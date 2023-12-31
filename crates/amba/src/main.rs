use std::{
	env,
	path::PathBuf,
	process::ExitCode,
	sync::{mpsc, Arc},
	time::Instant,
};

use chrono::offset::Local;
use model::Model;
use rand::{distributions::Alphanumeric, Rng};
use recipe::{FileSource, Recipe, RecipeError};
use tracing_subscriber::{filter::targets::Targets, fmt, layer::Layer};

use crate::cmd::Cmd;

mod cmd;
mod gui;
mod init;
mod run;

/// The executable component of amba that runs QEMU+S2E+libamba as a subprocess
///
/// Set `AMBA_DATA_DIR` to a directory where amba should read and write
/// run time artifacts such as disk images. The default is `$XDG_DATA_HOME/amba`
/// or `$HOME/.local/share/amba`.
#[derive(clap::Parser, Debug)]
#[command(about, verbatim_doc_comment)]
enum Args {
	Init(InitArgs),
	Run(RunArgs),
}

/// Initialize `$AMBA_DATA_DIR`
#[derive(clap::Args, Debug)]
pub struct InitArgs {
	/// Perform initialization even if amba has already been initialized.
	#[arg(short, long)]
	force: bool,
	/// Download guest images built by the S2E team from Google Drive rather than
	/// building them locally.
	#[arg(short, long)]
	download: bool,
}

/// Run QEMU+S2E+libamba
#[derive(clap::Args, Debug)]
pub struct RunArgs {
	/// Path to a recipe file specifying the run
	recipe_path: PathBuf,
	/// Start QEMU in a paused state, to attach a debugger or profiler
	#[arg(long)]
	debugger: bool,
	/// Do not open the graphical user interface
	#[arg(long)]
	no_gui: bool,
}

/// The nix store path of the script that builds guest images.
const AMBA_BUILD_GUEST_IMAGES_SCRIPT: &str = env!("AMBA_BUILD_GUEST_IMAGES_SCRIPT");

fn main() -> ExitCode {
	// Pinned version of winit crashes on wayland
	std::env::remove_var("WAYLAND_DISPLAY");

	tracing::subscriber::set_global_default(
		Targets::new()
			.with_target("h2", tracing::Level::INFO)
			.with_target(
				"tokio_util::codec::framed_impl",
				tracing::Level::DEBUG,
			)
			.with_target("eframe::native::run", tracing::Level::DEBUG)
			.with_default(tracing::Level::TRACE)
			.with_subscriber(
				tracing_subscriber::FmtSubscriber::builder()
					.with_max_level(tracing::Level::TRACE)
					.with_timer(UptimeHourMinuteSeconds::default())
					.with_thread_names(true)
					.finish(),
			),
	)
	.expect("enabling global logger");

	std::panic::set_hook(Box::new(|info| {
		let payload = info.payload();
		let msg = Option::or(
			payload.downcast_ref::<&str>().copied(),
			payload.downcast_ref::<String>().map(|s| &**s),
		)
		.unwrap_or("<no message>");
		let location = info
			.location()
			.map_or("<unknown location>".to_owned(), |loc| {
				format!("file {} at line {}", loc.file(), loc.line())
			});
		tracing::error!(location, msg, "panicked!");
		cmd::ctrlc_handler();
	}));

	let args: Args = clap::Parser::parse();

	let base = Box::leak(Box::new(BaseConfig {
		data_dir: match env::var_os("AMBA_DATA_DIR") {
			Some(dir) => PathBuf::from(dir),
			None => dirs::data_dir().unwrap().join("amba"),
		},
		dependencies_dir: match env::var_os("RUN_TIME_AMBA_DEPENDENCIES_DIR") {
			Some(dir) => PathBuf::from(dir),
			None => PathBuf::from(env!("COMPILE_TIME_AMBA_DEPENDENCIES_DIR")),
		},
	}));

	tracing::info!(debug_assertions = cfg!(debug_assertions));
	tracing::info!(AMBA_DEPENDENCIES_DIR = ?base.dependencies_dir);
	tracing::info!(AMBA_BUILD_GUEST_IMAGES_SCRIPT);
	tracing::info!(AMBA_DATA_DIR = ?base.data_dir);
	tracing::info!(?args);

	let cmd = Cmd::get();
	let res = match args {
		Args::Init(args) => init::init(cmd, base, args),
		Args::Run(args) => {
			if args.no_gui {
				let (tx, rx) = mpsc::channel();
				SessionConfig::new(cmd, base, &args).and_then(|config| {
					(run::control::Controller {
						tx,
						rx,
						gui_context: None,
						qemu_pid: None,
						embedder_tx: None,
					})
					.run(cmd, &config, Arc::new(Model::new()))
				})
			} else {
				SessionConfig::new(cmd, base, &args).and_then(|config| gui::run_gui(cmd, config))
			}
		}
	};
	match res {
		Ok(()) => ExitCode::SUCCESS,
		Err(()) => ExitCode::FAILURE,
	}
}

pub struct BaseConfig {
	dependencies_dir: PathBuf,
	data_dir: PathBuf,
}

pub struct SessionConfig {
	base: &'static BaseConfig,
	session_dir: PathBuf,
	temp_dir: PathBuf,
	recipe_path: PathBuf,
	recipe: Recipe,
	sigstop_before_qemu_exec: bool,
}

impl SessionConfig {
	pub fn new(cmd: &mut Cmd, base: &'static BaseConfig, run_args: &RunArgs) -> Result<Self, ()> {
		let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S");
		let mut rng = rand::thread_rng();
		let random: String = (0..6).map(|_| rng.sample(Alphanumeric) as char).collect();

		let recipe_path = run_args.recipe_path.clone();
		let recipe = match Recipe::deserialize_from(&cmd.read(&recipe_path)) {
			Ok(recipe) => recipe,
			Err(err) => {
				match err {
					RecipeError::NotSemanticRecipe(err) => {
						tracing::error!(
							?recipe_path,
							?err,
							"Not a semantically valid Recipe"
						);
					}
					RecipeError::NotSyntacticRecipe(err) => {
						tracing::error!(
							?recipe_path,
							?err,
							"Not a syntactically valid Recipe"
						);
					}
					RecipeError::NotJson(err) => {
						tracing::error!(?recipe_path, ?err, "Not a valid JSON");
					}
					RecipeError::NotUtf8(err) => {
						tracing::error!(?recipe_path, ?err, "Not valid UTF8");
					}
				}
				return Err(());
			}
		};

		Ok(Self {
			base,
			session_dir: base.data_dir.join(timestamp.to_string()),
			temp_dir: env::temp_dir().join(format!("amba-{timestamp}-{random}")),
			recipe_path,
			recipe,
			sigstop_before_qemu_exec: run_args.debugger,
		})
	}

	pub fn executable_host_path(&self) -> Option<PathBuf> {
		// NOTE: `fs::canonicalize` and similar are inappropriate here since we are
		// operating on a *guest* path.
		fn remove_executable_dotslash(mut guest_path: &str) -> &str {
			while let Some(stripped) = guest_path.strip_prefix("./") {
				assert_ne!(stripped.chars().next(), Some('/'));
				guest_path = stripped;
			}
			assert!(!guest_path.is_empty());
			guest_path
		}
		// Absolute guest paths are interpreted as already existing within the
		// guest and therefore having no host path to copy from.
		if self.recipe.executable_path.starts_with("/") {
			return None;
		}
		let guest_path: &str = remove_executable_dotslash(&self.recipe.executable_path);

		match self.recipe.files.get(guest_path) {
			None => panic!(
				"invalid recipe: guest path '{guest_path}' matches no guest file: {:?}",
				self.recipe.files
			),
			Some(FileSource::Host(host_path)) => {
				Some(self.recipe_path.parent().unwrap().join(host_path))
			}
			Some(
				symbolic @ (FileSource::SymbolicContent { .. } | FileSource::SymbolicHost { .. }),
			) => panic!("invalid recipe: executable file cannot be symbolic: {symbolic:?}"),
		}
	}
}

/// A timer to add `{h}h{m}m{s}s` to logs.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct UptimeHourMinuteSeconds {
	epoch: Instant,
}

impl Default for UptimeHourMinuteSeconds {
	fn default() -> Self {
		UptimeHourMinuteSeconds {
			epoch: Instant::now(),
		}
	}
}

impl fmt::time::FormatTime for UptimeHourMinuteSeconds {
	fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
		let seconds = self.epoch.elapsed().as_secs();
		let h = seconds / (60 * 60);
		let m = (seconds / 60) % (60 * 60);
		let s = seconds % 60;
		write!(w, "{h}h{m}m{s}s")
	}
}
