//! Runner functions for the run subcommand

#![allow(unsafe_code)]

use std::{
	ffi::{OsStr, OsString},
	os::unix::{net::UnixStream, process::CommandExt},
	path::Path,
	process::{self, Command, ExitStatus},
	sync::mpsc,
	thread,
	time::Duration,
};

use ipc::{IpcError, IpcMessage, IpcRx};
use qmp_client::{QmpClient, QmpCommand, QmpError, QmpEvent};

use crate::{
	cmd::Cmd,
	run::{control::ControllerMsg, session::S2EConfig},
	SessionConfig,
};

pub fn prepare_run(cmd: &mut Cmd, config: &SessionConfig) -> Result<(), ()> {
	fn data_dir_has_been_initialized(cmd: &mut Cmd, data_dir: &Path) -> bool {
		let version_file = &data_dir.join("version.txt");
		let version = version_file
			.exists()
			.then(|| String::from_utf8(cmd.read(version_file)).unwrap());
		let initialized = version.is_some() && !version.unwrap().is_empty();
		if !initialized {
			tracing::error!("$AMBA_DATA_DIR/version.txt is missing or empty");
		}
		initialized
	}

	if !data_dir_has_been_initialized(cmd, &config.base.data_dir) {
		tracing::error!(
			?config.base.data_dir,
			"AMBA_DATA_DIR has not been initialized"
		);
		return Err(());
	}

	if config.session_dir.exists() {
		tracing::error!(
			?config.session_dir,
			"session_dir already exists, are multiple amba instances running concurrently?"
		);
		return Err(());
	}
	if config.temp_dir.exists() {
		tracing::error!(
			?config.temp_dir,
			"temp_dir already exists. How did this happen?!"
		);
		return Err(());
	}
	cmd.create_dir_all(&config.session_dir);
	cmd.create_dir_all(&config.temp_dir);

	// Populate the `session_dir`
	S2EConfig::new(
		cmd,
		&config.session_dir,
		&config.recipe_path,
		&config.recipe,
	)
	.save_to(
		cmd,
		&config.base.dependencies_dir,
		&config.session_dir,
	);

	Ok(())
}

pub fn run_ipc(mut ipc_rx: IpcRx, controller_tx: mpsc::Sender<ControllerMsg>) -> Result<(), ()> {
	loop {
		match ipc_rx.blocking_receive() {
			Ok(IpcMessage::NewEdges {
				state_edges,
				block_edges,
			}) => {
				controller_tx
					.send(ControllerMsg::UpdateEdges {
						state_edges,
						block_edges,
					})
					.unwrap_or_else(|mpsc::SendError(_)| {
						tracing::warn!("ipc failed messaging controller: already shut down");
					});
			}
			Ok(msg) => tracing::info!(?msg),
			Err(IpcError::EndOfFile) => break,
			Err(other) => panic!("ipc error: {other:?}"),
		}
	}
	Ok(())
}

pub fn run_qemu(
	cmd: &mut Cmd,
	config: &SessionConfig,
	qmp_socket: &Path,
	controller_tx: mpsc::Sender<ControllerMsg>,
) -> Result<(), ()> {
	// supporting single- vs multi-path
	let s2e_mode = match true {
		true => "s2e",
		false => "s2e_sp",
	};
	let arch = "x86_64";

	let qemu = &config
		.base
		.dependencies_dir
		.join(format!("bin/qemu-system-{arch}"));
	let libs2e_dir = &config.base.dependencies_dir.join("share/libs2e");
	let libs2e = &libs2e_dir.join(format!("libs2e-{arch}-{s2e_mode}.so"));
	let s2e_config = &config.session_dir.join("s2e-config.lua");
	let max_processes = 1;
	let image = &config
		.base
		.data_dir
		.join("images/ubuntu-22.04-x86_64/image.raw.s2e");

	let status = run_qemu_inner(
		cmd,
		config.sigstop_before_qemu_exec,
		qemu,
		&config.temp_dir,
		libs2e,
		libs2e_dir,
		s2e_config,
		max_processes,
		image,
		qmp_socket,
		|pid| controller_tx.send(ControllerMsg::TellQemuPid(pid)).unwrap(),
	);
	match status.success() {
		true => Ok(()),
		false => {
			tracing::error!(?status, "qemu exited with error code");
			Err(())
		}
	}
}

pub fn run_qemu_inner(
	cmd: &mut Cmd,
	sigstop_qemu_on_fork: bool,
	qemu: &Path,
	temp_dir: &Path,
	libs2e: &Path,
	libs2e_dir: &Path,
	s2e_config: &Path,
	max_processes: u16,
	image: &Path,
	qmp_socket: &Path,
	with_pid: impl FnOnce(u32),
) -> ExitStatus {
	assert!(qemu.exists());
	assert!(libs2e.exists());
	assert!(s2e_config.exists());
	assert!(libs2e_dir.exists());

	let mut command = Command::new(qemu);
	command
		.current_dir(temp_dir)
		.env("LD_PRELOAD", libs2e)
		.env("S2E_CONFIG", s2e_config)
		.env("S2E_SHARED_DIR", libs2e_dir)
		.env("S2E_MAX_PROCESSES", max_processes.to_string())
		.env("S2E_UNBUFFERED_STREAM", "1");

	if sigstop_qemu_on_fork {
		// Before exec, and hence actually starting QEMU, the child process sends
		// SIGSTOP to itself. We can then start debugging by attaching to the QEMU pid
		// and sending SIGCONT
		// SAFETY: `raise` and `write` are async-safe. We do not allocate memory.
		unsafe {
			let mut buf = String::with_capacity(256);
			command.pre_exec(move || {
				use std::fmt::Write;

				let _ = writeln!(
					buf,
					"[pre-exec before SIGSTOP] stopped with pid={}",
					process::id()
				);

				nix::unistd::write(2, buf.as_ref())?;
				nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP)?;
				nix::unistd::write(2, b"[pre-exec after SIGSTOP] resuming!\n")?;

				Ok(())
			});
		}
	}
	if max_processes > 1 {
		command.arg("-nographic");
	}
	command
		.arg("-qmp")
		.arg({
			let mut line = OsString::new();
			line.push("unix:");
			line.push(qmp_socket);
			line.push(",server,nowait");
			line
		})
		.arg("-drive")
		.arg({
			let mut line = OsString::new();
			line.push("file=");
			line.push(image);
			{
				let bytes = <OsStr as std::os::unix::ffi::OsStrExt>::as_bytes(image.as_ref());
				assert!(!bytes.contains(&b','));
			}
			line.push(",format=s2e,cache=writeback");
			line
		})
		.args([
			"-k",
			"en-us",
			"-monitor",
			"null",
			"-m",
			"256M",
			"-enable-kvm",
			"-serial",
			"file:/dev/stdout",
			"-net",
			"none",
			"-net",
			"nic,model=e1000",
			"-loadvm",
			"ready",
		]);
	cmd.command_spawn_wait_with_pid(&mut command, with_pid)
}

pub fn run_qmp(socket: &Path, controller_tx: mpsc::Sender<ControllerMsg>) -> Result<(), ()> {
	let stream = 'outer: {
		let mut attempt = 0;
		let result = loop {
			attempt += 1;
			match UnixStream::connect(socket) {
				Ok(stream) => break 'outer stream,
				Err(err) if attempt > 10 => break err,
				Err(_) => {}
			}
			thread::sleep(Duration::from_millis(50));
		};
		tracing::error!(?result, ?socket, "failed to connect to socket");
		return Err(());
	};

	let mut qmp = QmpClient::new(&stream);

	let event_handler = |event @ QmpEvent { .. }| {
		tracing::info!(?event, "QMP");
	};

	let greeting = qmp.blocking_receive().expect("greeting");
	tracing::info!(?greeting, "QMP");

	let negotiated = qmp
		.blocking_request(&QmpCommand::QmpCapabilities, event_handler)
		.unwrap();
	tracing::info!(?negotiated, "QMP");

	let status = qmp
		.blocking_request(&QmpCommand::QueryStatus, event_handler)
		.unwrap();
	tracing::info!(?status, "QMP");

	loop {
		match qmp.blocking_receive() {
			Ok(response) => {
				tracing::info!(?response, "QMP");
				if let qmp_client::QmpResponse::Event(QmpEvent { event, .. }) = response {
					if event == "SHUTDOWN" {
						controller_tx.send(ControllerMsg::QemuShutdown).unwrap();
						return Ok(());
					}
				}
			}
			Err(QmpError::EndOfFile) => return Err(()),
			Err(err) => unreachable!("{:?}", err),
		}
	}
}
