//! mpv daemon (mpvd) process management.

use std::fs::{self, File};
use std::io;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Find the pid of the mpv daemon listening on `sock` by scanning /proc
/// (equivalent of `pgrep -f "input-ipc-server=<sock>"`).
pub fn find_pid(sock: &Path) -> Option<u32> {
	let needle = format!("input-ipc-server={}", sock.display());
	let entries = fs::read_dir("/proc").ok()?;
	for entry in entries.flatten() {
		let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
			continue;
		};
		let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
			continue;
		};
		let found = cmdline
			.split(|b| *b == 0)
			.filter_map(|arg| std::str::from_utf8(arg).ok())
			.any(|arg| arg.contains(&needle));
		if found {
			return Some(pid);
		}
	}
	None
}

/// Start the mpv daemon if it is not already running.
pub fn start(sock: &Path, playlist_file: &Path, log_file: &Path) -> io::Result<()> {
	if !playlist_file.try_exists()? {
		if let Some(dir) = playlist_file.parent() {
			fs::create_dir_all(dir)?;
		}
		File::create(playlist_file)?;
	}
	if find_pid(sock).is_some() {
		return Ok(());
	}

	let log = File::create(log_file)?;
	Command::new("mpv")
		.arg("--loop-playlist")
		.arg("--cache=yes")
		.arg("--cache-secs=60")
		.arg("--demuxer-max-bytes=256MiB")
		.arg("--demuxer-readahead-secs=30")
		.arg("--no-video")
		.arg("--idle=yes")
		.arg(format!("--playlist={}", playlist_file.display()))
		.arg(format!("--input-ipc-server={}", sock.display()))
		.stdin(Stdio::null())
		.stdout(log.try_clone()?)
		.stderr(log)
		.process_group(0)
		.spawn()?;
	Ok(())
}

/// Kill the mpv daemon if it is running.
pub fn stop(sock: &Path) {
	if let Some(pid) = find_pid(sock) {
		#[allow(clippy::cast_possible_wrap)]
		let pid = pid as i32;
		unsafe {
			libc::kill(pid, libc::SIGKILL);
		}
	}
}

/// Make sure the mpv daemon is running, starting (and waiting for) it if needed.
pub fn recheck(sock: &Path, playlist_file: &Path, log_file: &Path) {
	if find_pid(sock).is_some() {
		return;
	}
	println!("mpv server is down");
	if let Err(e) = start(sock, playlist_file, log_file) {
		eprintln!("Error: {e}");
		std::process::exit(1);
	}
	println!("starting mpv server ...");
	// wait until the process exists and its IPC socket accepts connections
	while find_pid(sock).is_none() || UnixStream::connect(sock).is_err() {
		thread::sleep(Duration::from_millis(100));
	}
}
