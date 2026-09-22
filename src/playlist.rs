//! Playlist file handling (the m3u file the daemon was started with).

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde_json::Value;

use crate::ipc;

/// Get the filenames of the current mpv playlist.
pub fn filenames(sock: &Path) -> io::Result<Vec<String>> {
	let data = ipc::property_data(sock, "playlist")?;
	Ok(data
		.as_array()
		.map(|entries| {
			entries
				.iter()
				.filter_map(|e| e.get("filename").and_then(Value::as_str))
				.map(str::to_owned)
				.collect()
		})
		.unwrap_or_default())
}

/// Write the current mpv playlist filenames to `file`.
pub fn refresh_from_mpv(sock: &Path, file: &Path) -> io::Result<()> {
	write_lines(file, &filenames(sock)?)
}

/// Append paths to the playlist file.
pub fn append(file: &Path, paths: &[String]) -> io::Result<()> {
	let mut out = fs::OpenOptions::new().create(true).append(true).open(file)?;
	for path in paths {
		writeln!(out, "{path}")?;
	}
	Ok(())
}

/// Write the playlist file, one path per line.
pub fn write_lines(file: &Path, lines: &[String]) -> io::Result<()> {
	if let Some(dir) = file.parent() {
		fs::create_dir_all(dir)?;
	}
	let mut out = fs::File::create(file)?;
	for line in lines {
		writeln!(out, "{line}")?;
	}
	Ok(())
}

/// Copy a playlist file over the daemon playlist file.
pub fn load(src: &Path, playlist_file: &Path) -> io::Result<()> {
	let content = fs::read(src)?;
	if let Some(dir) = playlist_file.parent() {
		fs::create_dir_all(dir)?;
	}
	fs::write(playlist_file, content)
}
