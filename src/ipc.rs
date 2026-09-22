//! mpv JSON IPC protocol client over a unix socket.

use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use serde_json::{Value, json};

pub const SUCCESS: &str = "success";

/// Connection to the mpv IPC socket.
pub struct Client {
	stream: UnixStream,
}

impl Client {
	pub fn connect(sock: &Path) -> io::Result<Self> {
		UnixStream::connect(sock).map(|stream| Self { stream })
	}

	/// Send the given JSON requests and read raw response lines until mpv
	/// closes the connection (events and responses are all returned).
	pub fn send(&mut self, requests: &[Value]) -> io::Result<Vec<String>> {
		for req in requests {
			writeln!(self.stream, "{req}")?;
		}
		self.stream.flush()?;
		self.stream.shutdown(Shutdown::Write)?;
		let reader = BufReader::new(&self.stream);
		let mut lines = Vec::new();
		for line in reader.lines() {
			lines.push(line?);
		}
		Ok(lines)
	}
}

/// Send a single command and return its response object.
pub fn command(sock: &Path, cmd: &Value) -> io::Result<Value> {
	let request = json!({"request_id": 1, "command": cmd});
	let lines = Client::connect(sock)?.send(&[request])?;
	for line in lines {
		if let Ok(v) = serde_json::from_str::<Value>(&line)
			&& v.get("request_id").and_then(Value::as_u64) == Some(1)
		{
			return Ok(v);
		}
	}
	Err(io::Error::new(io::ErrorKind::InvalidData, "no response from mpv"))
}

/// Query a property, returning the full response object.
pub fn get_property(sock: &Path, name: &str) -> io::Result<Value> {
	command(sock, &json!(["get_property", name]))
}

/// Query a property, returning its data value.
pub fn property_data(sock: &Path, name: &str) -> io::Result<Value> {
	let resp = get_property(sock, name)?;
	if resp.get("error").and_then(Value::as_str) == Some(SUCCESS) {
		Ok(resp.get("data").cloned().unwrap_or(Value::Null))
	} else {
		Err(io::Error::other(
			resp.get("error")
				.and_then(Value::as_str)
				.unwrap_or("unknown error")
				.to_string(),
		))
	}
}

/// Convert a command line argument to a JSON value the way the shell
/// version did: booleans and numbers are passed through, the rest as strings.
pub fn coerce_arg(arg: &str) -> Value {
	if arg == "true" {
		json!(true)
	} else if arg == "false" {
		json!(false)
	} else if let Ok(n) = arg.parse::<i64>() {
		json!(n)
	} else if let Ok(f) = arg.parse::<f64>() {
		json!(f)
	} else {
		json!(arg)
	}
}

/// Build a `["name", args...]` command array from string arguments.
pub fn build_command(name: &str, args: &[String]) -> Value {
	let mut cmd = vec![json!(name)];
	cmd.extend(args.iter().map(|a| coerce_arg(a)));
	Value::Array(cmd)
}

/// Messages pushed by the observer's reader thread.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcMsg {
	/// A JSON message (event or command response) sent by mpv.
	Json(Value),
	/// The connection to mpv was closed.
	Closed,
}

/// Persistent IPC connection that observes mpv properties and receives all
/// events and responses pushed by mpv.
pub struct Observer {
	writer: UnixStream,
	rx: Receiver<IpcMsg>,
	next_id: AtomicU64,
}

impl Observer {
	/// Connect and start observing the given `(name, id)` properties.
	pub fn connect(sock: &Path, props: &[(&str, u64)]) -> io::Result<Self> {
		let stream = UnixStream::connect(sock)?;
		let writer = stream.try_clone()?;
		let (tx, rx) = channel();
		thread::spawn(move || {
			let reader = BufReader::new(stream);
			for line in reader.lines() {
				match line {
					Ok(line) => {
						// empty or non-JSON lines simply fail to parse and are skipped
						if let Ok(v) = serde_json::from_str::<Value>(&line)
							&& tx.send(IpcMsg::Json(v)).is_err()
						{
							return;
						}
					}
					Err(_) => break,
				}
			}
			let _ = tx.send(IpcMsg::Closed);
		});
		let observer = Self {
			writer,
			rx,
			next_id: AtomicU64::new(1000),
		};
		for (name, id) in props {
			observer.send(&json!({"command": ["observe_property", id, name]}))?;
		}
		Ok(observer)
	}

	/// Send a raw JSON request.
	pub fn send(&self, request: &Value) -> io::Result<()> {
		let mut writer = &self.writer;
		writeln!(writer, "{request}")?;
		writer.flush()
	}

	/// Send a command with a fresh request id so its response (and errors)
	/// can be matched later.
	pub fn send_tracked(&self, cmd: &Value) -> io::Result<u64> {
		let id = self.next_id.fetch_add(1, Ordering::Relaxed);
		self.send(&json!({"request_id": id, "command": cmd}))?;
		Ok(id)
	}

	/// Fetch the next pending message, if any.
	pub fn poll(&self) -> Option<IpcMsg> {
		match self.rx.try_recv() {
			Ok(msg) => Some(msg),
			Err(TryRecvError::Empty) => None,
			Err(TryRecvError::Disconnected) => Some(IpcMsg::Closed),
		}
	}
}
