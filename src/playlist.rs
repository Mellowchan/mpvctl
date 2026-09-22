//! Playlist file handling (the m3u file the daemon was started with).

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::ipc;

/// Get the filenames of the current mpv playlist.
pub fn filenames(sock: &Path) -> io::Result<Vec<String>> {
	let data = ipc::property_data(sock, "playlist")?;
	Ok(filenames_from(&data))
}

/// Extract the filenames from a playlist property value.
fn filenames_from(data: &Value) -> Vec<String> {
	data.as_array()
		.map(|entries| {
			entries
				.iter()
				.filter_map(|e| e.get("filename").and_then(Value::as_str))
				.map(str::to_owned)
				.collect()
		})
		.unwrap_or_default()
}

const REFRESH_ID: u64 = 999;

/// Run commands on a fresh connection and write the resulting playlist to
/// `file` (single round trip, so commands and read-back cannot race).
pub fn run_commands_refresh(sock: &Path, cmds: &[Value], file: &Path) -> io::Result<()> {
	let mut requests: Vec<Value> = cmds
		.iter()
		.enumerate()
		.map(|(i, c)| json!({"request_id": i as u64 + 1, "command": c}))
		.collect();
	requests.push(json!({"request_id": REFRESH_ID, "command": ["get_property", "playlist"]}));
	let lines = ipc::Client::connect(sock)?.send(&requests)?;
	for line in lines {
		let Ok(v) = serde_json::from_str::<Value>(&line) else {
			continue;
		};
		if v.get("request_id").and_then(Value::as_u64) == Some(REFRESH_ID) {
			let error = v.get("error").and_then(Value::as_str).unwrap_or_default();
			if error != ipc::SUCCESS {
				return Err(io::Error::other(format!("could not read playlist: {error}")));
			}
			return write_lines(file, &filenames_from(v.get("data").unwrap_or(&Value::Null)));
		}
	}
	Err(io::Error::other("no playlist response from mpv"))
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
