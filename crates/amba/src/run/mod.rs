use std::{
	path::Path,
	process::{Command, ExitStatus},
};

use chrono::offset::Local;

use crate::{cmd::Cmd, run::session::S2EConfig, RunArgs, AMBA_DEPENDENCIES_DIR};

mod session;

// See also
// https://github.com/S2E/s2e-env/blob/master/s2e_env/templates/launch-s2e.sh
// TODO cross-platform
pub fn run(
	cmd: &mut Cmd,
	data_dir: &Path,
	RunArgs {
		host_path_to_executable,
	}: RunArgs,
) -> Result<(), ()> {
	if !data_dir_has_been_initialized(cmd, data_dir) {
		tracing::error!(
			?data_dir,
			"AMBA_DATA_DIR has not been initialized"
		);
		return Err(());
	}

	let session_dir = &data_dir.join(format!(
		"analysis-{}",
		Local::now().format("%Y-%m-%dT%H:%M:%S")
	));
	if session_dir.exists() {
		tracing::error!(
			?session_dir,
			"session_dir already exists, are multiple amba instances running concurrently?"
		);
		return Err(());
	}
	cmd.create_dir_all(session_dir);
	{
		let executable_name = host_path_to_executable
			.file_name()
			.unwrap()
			.to_str()
			.unwrap();
		cmd.copy(
			&host_path_to_executable,
			session_dir.join(executable_name),
		);
		S2EConfig::new(session_dir, &executable_name).save_to(cmd, session_dir);
	}

	// supporting single- vs multi-path
	let s2e_mode = match true {
		true => "s2e",
		false => "s2e_sp",
	};
	let arch = "x86_64";

	let qemu = &Path::new(AMBA_DEPENDENCIES_DIR).join(format!("bin/qemu-system-{arch}"));
	let libs2e_dir = &Path::new(AMBA_DEPENDENCIES_DIR).join("share/libs2e");
	let libs2e = &libs2e_dir.join(format!("libs2e-{arch}-{s2e_mode}.so"));
	let s2e_config = &session_dir.join("s2e-config.lua");
	let max_processes = 1;
	let image_name = "ubuntu-22.04-x86_64";
	let serial_out = &session_dir.join("serial.txt");

	let status = run_qemu(
		cmd,
		qemu,
		libs2e,
		libs2e_dir,
		s2e_config,
		max_processes,
		image_name,
		serial_out,
	);
	if status.success() {
		Ok(())
	} else {
		tracing::error!(?status, "qemu exited with error code");
		Err(())
	}
}
fn run_qemu(
	cmd: &mut Cmd,
	qemu: &Path,
	libs2e: &Path,
	libs2e_dir: &Path,
	s2e_config: &Path,
	max_processes: u16,
	image_name: &str,
	serial_out: &Path,
) -> ExitStatus {
	assert!(qemu.exists());
	assert!(libs2e.exists());
	assert!(s2e_config.exists());
	assert!(libs2e_dir.exists());

	let mut command = Command::new(qemu);
	command
		.env("LD_PRELOAD", libs2e)
		.env("S2E_CONFIG", s2e_config)
		.env("S2E_SHARED_DIR", libs2e_dir)
		.env("S2E_MAX_PROCESSES", max_processes.to_string())
		.env("S2E_UNBUFFERED_STREAM", "1");
	if max_processes > 1 {
		command.arg("-nographic");
	}
	cmd.command_spawn_wait(command.args([
		"-drive",
		&format!("file={image_name},format=s2e,cache=writeback",),
		"-k",
		"en-us",
		"-monitor",
		"null",
		"-m",
		"256M",
		"-enable-kvm",
		"-serial",
		&format!("file:{}", serial_out.to_str().unwrap()),
		"-net",
		"none",
		"-net",
		"nix,model=e1000",
		"-loadvm",
		"ready",
	]))
}
fn data_dir_has_been_initialized(cmd: &mut Cmd, data_dir: &Path) -> bool {
	let mut count = 0;
	let images = data_dir.join("images");
	if !images.exists() {
		return false;
	}
	for entry in cmd.read_dir(images) {
		let _ = entry.unwrap();
		count += 1;
	}
	match count {
		0 => {
			tracing::error!("$AMBA_DATA_DIR/images is empty");
			false
		}
		_ => true,
	}
}