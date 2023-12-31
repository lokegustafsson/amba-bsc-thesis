//! The init subcommand

use std::{path::Path, process::Command};

use crate::{cmd::Cmd, BaseConfig, InitArgs};

mod build;
mod download;

/// Initialize amba by building/downloading the guest images to be run in
/// QEMU+S2E+libamba.
pub fn init(
	cmd: &mut Cmd,
	config: &BaseConfig,
	InitArgs { force, download }: InitArgs,
) -> Result<(), ()> {
	// Choose strategy.
	let initializer: Box<dyn InitStrategy> = match download {
		true => download::InitDownload::new(),
		false => build::InitBuild::new(),
	};
	// Already up to date?
	let new_version = initializer.version();
	let version_file = &config.data_dir.join("version.txt");
	{
		let old_version = version_file
			.exists()
			.then(|| String::from_utf8(cmd.read(version_file)).unwrap());

		if !force && old_version.as_ref() == Some(&new_version) {
			tracing::info!("guest images already up to date; force rebuild with --force");
			return Ok(());
		}
	}

	// Remove artifacts from old or unfinished builds.
	{
		version_file.exists().then(|| cmd.remove(version_file));
		let images = &config.data_dir.join("images");
		let images_build = &config.data_dir.join("images-build");
		if images.exists() {
			remove_images(cmd, images);
		}
		if images_build.exists() {
			build::remove_images_build(cmd, images_build);
		}
	}

	// Perform initialization, assert success.
	initializer.init(cmd, &config.data_dir)?;
	assert!(config
		.data_dir
		.join("images/ubuntu-22.04-x86_64/image.json")
		.exists());
	assert!(config
		.data_dir
		.join("images/ubuntu-22.04-x86_64/image.raw.s2e")
		.exists());
	assert!(config
		.data_dir
		.join("images/ubuntu-22.04-x86_64/image.raw.s2e.ready")
		.exists());

	cmd.write(version_file, new_version);
	Ok(())
}

/// A method for acquiring guest images.
trait InitStrategy {
	/// Construct a strategy.
	fn new() -> Box<Self>
	where
		Self: Sized;

	/// Get strategy version string, to check up-to-date:ness.
	fn version(&self) -> String;

	/// Perform initialization.
	fn init(self: Box<Self>, cmd: &mut Cmd, data_dir: &Path) -> Result<(), ()>;
}

fn remove_images(cmd: &mut Cmd, images: &Path) {
	let chmod_result =
		cmd.command_spawn_wait(Command::new("chmod").args(["-R", "u+w"]).arg(images));
	assert!(chmod_result.success());
	build::unmount_images_imagefs(cmd, images);
	// Recursively delete `$AMBA_DATA_DIR/images/`
	cmd.remove_dir_all(images);
}
