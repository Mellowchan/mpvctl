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

/// Sorted playlist file names in a playlists directory (regular files only).
pub fn dir_entries(dir: &Path) -> io::Result<Vec<String>> {
	let mut names: Vec<String> = fs::read_dir(dir)?
		.flatten()
		.filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
		.filter_map(|e| e.file_name().into_string().ok())
		.collect();
	names.sort();
	Ok(names)
}

const REFRESH_ID: u64 = 999;

/// Build the `playlist-move` commands that move entries `a..=b` in front of
/// the entry currently at index `before` (or to the end when `None`),
/// preserving the order inside the range.
///
/// mpv only moves single entries, so the commands are derived greedily from
/// the target order: fix each position left to right by moving the wanted
/// entry in front of it (its position is always to the right of the target,
/// which lands it exactly on the target index).
pub fn move_range_commands(len: usize, a: usize, b: usize, before: Option<usize>) -> Result<Vec<Value>, String> {
	if len == 0 || a >= len || b >= len || a > b {
		return Err(format!("invalid index or range: {a}-{b}"));
	}
	let block: Vec<usize> = (a..=b).collect();
	let mut rest: Vec<usize> = (0..len).filter(|i| !block.contains(i)).collect();
	let target: Vec<usize> = if let Some(j) = before {
		if j >= len {
			return Err(format!("index out of range: {j}"));
		}
		if block.contains(&j) {
			return Err(format!("target index {j} is inside the moved range"));
		}
		let pos = rest.iter().position(|&i| i == j).expect("j not in block");
		let mut target = rest.drain(..pos).collect::<Vec<_>>();
		target.extend_from_slice(&block);
		target.extend_from_slice(&rest);
		target
	} else {
		rest.extend_from_slice(&block);
		rest
	};

	let mut current: Vec<usize> = (0..len).collect();
	let mut cmds = Vec::new();
	for pos in 0..len {
		if current[pos] != target[pos] {
			let from = current.iter().position(|&i| i == target[pos]).expect("entry present");
			cmds.push(json!(["playlist-move", from, pos]));
			let entry = current.remove(from);
			current.insert(pos, entry);
		}
	}
	Ok(cmds)
}

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

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::move_range_commands;
	use serde_json::Value;

	/// Apply the produced playlist-move commands like mpv would.
	fn simulate(len: usize, cmds: &[Value]) -> Vec<usize> {
		let mut v: Vec<usize> = (0..len).collect();
		for cmd in cmds {
			let from = usize::try_from(cmd[1].as_u64().unwrap()).unwrap();
			let to = usize::try_from(cmd[2].as_u64().unwrap()).unwrap();
			let to = if to > from { to - 1 } else { to };
			let entry = v.remove(from);
			v.insert(to, entry);
		}
		v
	}

	#[test]
	fn single_moves() {
		// move 0 in front of 2 (mpv playlist-move 0 2 semantics)
		let cmds = move_range_commands(4, 0, 0, Some(2)).unwrap();
		assert_eq!(simulate(4, &cmds), vec![1, 0, 2, 3]);
		// move 3 in front of 1
		let cmds = move_range_commands(4, 3, 3, Some(1)).unwrap();
		assert_eq!(simulate(4, &cmds), vec![0, 3, 1, 2]);
		// move 0 in front of 1: already in front, no-op (like mpv)
		let cmds = move_range_commands(4, 0, 0, Some(1)).unwrap();
		assert_eq!(simulate(4, &cmds), vec![0, 1, 2, 3]);
		assert!(cmds.is_empty());
	}

	#[test]
	fn range_moves() {
		// m 10-15 2 with 20 tracks: entries 10..=15 land before entry 2
		let cmds = move_range_commands(20, 10, 15, Some(2)).unwrap();
		let result = simulate(20, &cmds);
		assert_eq!(&result[..8], &[0, 1, 10, 11, 12, 13, 14, 15]);
		assert_eq!(&result[8..], &[2, 3, 4, 5, 6, 7, 8, 9, 16, 17, 18, 19]);
		// move a range to the end
		let cmds = move_range_commands(5, 1, 2, None).unwrap();
		assert_eq!(simulate(5, &cmds), vec![0, 3, 4, 1, 2]);
		// move a range backwards
		let cmds = move_range_commands(5, 3, 4, Some(1)).unwrap();
		assert_eq!(simulate(5, &cmds), vec![0, 3, 4, 1, 2]);
	}

	#[test]
	fn invalid_moves() {
		assert!(move_range_commands(4, 2, 1, Some(0)).is_err()); // reversed range
		assert!(move_range_commands(4, 0, 9, Some(1)).is_err()); // out of bounds
		assert!(move_range_commands(4, 0, 2, Some(4)).is_err()); // target out of bounds
		assert!(move_range_commands(4, 0, 2, Some(1)).is_err()); // target inside range
		assert!(move_range_commands(0, 0, 0, None).is_err()); // empty playlist
	}
}
