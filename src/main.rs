use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::exit;

use serde_json::{Value, json};

mod config;
mod daemon;
mod ipc;
mod playlist;
mod tui;

use config::{Keybindings, Theme};

const BOLD: &str = "\x1b[1m";
/// reset sequence as emitted by `tput sgr0` on xterm terminals
const NORMAL: &str = "\x1b(B\x1b[m";

struct Ctx {
	progname: String,
	sock: PathBuf,
	playlist_file: PathBuf,
	playlists_dir: PathBuf,
	log_file: PathBuf,
}

fn usage(progname: &str) {
	print!(
		r"{progname} <cmd> [ARGS...]

COMMANDS:
  p | play		- start playing
  s | pause		- pause current file
  T | toggle		- toggle current file
  N | next		- play next file
  P | prev		- play previous file
  l | ls		- list the playlist
  c | clear		- clear the playlist
  S | shuffle		- shuffle the playlist
  j | jump [i]		- jump to index in playlist
  e | seek [i]		- seek in seconds
  t | time [i]		- jump to time in seconds
  O | prop [...]	- get property
  C | cmd [...]		- send custom command
  a | add [...]		- add parameters to playlist
  d | del [i] [i]	- delete item or range
  m | move [r] [j]	- move item or range r (e.g. 2 or 10-15) in front of item j
  save [file]		- save current playlist to file
  load [name]		- load playlist (name from the playlists dir or a path)
  pl | playlists	- list playlists in the playlists dir
  start			- start mpv server
  stop			- stop mpv server
  restart		- restart mpv server
  status		- print mpv server status
  log			- list the mpv server log
  h | help		- print usage

PIPE:
  find ~/music/ -type f | {progname}

TUI:
  run {progname} without arguments in a terminal

"
	);
}

pub(crate) fn home() -> PathBuf {
	let home = env::var("HOME").map_err(|_| "HOME is not set").unwrap_or_else(|e| {
		eprintln!("Error: {e}");
		exit(1);
	});
	PathBuf::from(home)
}

fn xdg_runtime_dir() -> PathBuf {
	PathBuf::from(env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()))
}

fn new_ctx(config: &config::Config) -> Ctx {
	let progname = env::args().next().unwrap_or_else(|| "mpvctl".into());
	Ctx {
		progname,
		sock: xdg_runtime_dir().join("mpvd"),
		playlist_file: home().join(".local/share/mpvd_playlist.m3u"),
		playlists_dir: config::playlists_dir(config),
		log_file: PathBuf::from("/tmp/mpvd.log"),
	}
}

fn strip_ext(name: &str) -> &str {
	match name.rfind('.') {
		Some(pos) if pos > 0 => &name[..pos],
		_ => name,
	}
}

fn basename(path: &str) -> &str {
	path.rsplit('/').next().unwrap_or(path)
}

pub(crate) fn display_name(filename: &str) -> &str {
	strip_ext(basename(filename))
}

fn have_terminal() -> bool {
	matches!(env::var("TERM"), Ok(t) if !t.is_empty() && t != "dumb")
}

/// Floor playback/duration seconds to whole seconds for display.
fn playlist_times(playback: Option<f64>, duration: Option<f64>) -> (i64, i64) {
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	fn secs(v: Option<f64>) -> i64 {
		v.unwrap_or(0.0).floor() as i64
	}
	(secs(playback), secs(duration))
}

/// Send a command built from args and print the raw responses.
fn cmd(ctx: &Ctx, name: &str, args: &[String]) -> Result<(), Box<dyn Error>> {
	let command = ipc::build_command(name, args);
	let lines = ipc::Client::connect(&ctx.sock)?.send(&[json!({"command": command})])?;
	for line in lines {
		println!("{line}");
	}
	Ok(())
}

/// Send commands without caring about their responses.
fn cmd_quiet(sock: &Path, commands: &[Value]) -> Result<(), Box<dyn Error>> {
	let requests: Vec<Value> = commands
		.iter()
		.enumerate()
		.map(|(i, c)| json!({"request_id": i + 1, "command": c}))
		.collect();
	ipc::Client::connect(sock)?.send(&requests)?;
	Ok(())
}

fn parse_index(arg: &str) -> Result<i64, Box<dyn Error>> {
	arg.parse::<i64>().map_err(|_| format!("invalid index: {arg}").into())
}

fn toggle(ctx: &Ctx) -> Result<(), Box<dyn Error>> {
	let paused = ipc::property_data(&ctx.sock, "pause")?;
	let set = !paused.as_bool().unwrap_or(false);
	cmd(
		ctx,
		"set_property",
		&["pause".into(), if set { "true" } else { "false" }.into()],
	)?;
	Ok(())
}

fn append(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let stdin_paths: Vec<String> = if io::stdin().is_terminal() {
		args.to_vec()
	} else {
		let mut buf = String::new();
		io::stdin().read_to_string(&mut buf)?;
		buf.lines().map(str::to_owned).collect()
	};

	let mut fullpaths = Vec::new();
	for path in stdin_paths {
		match fs::canonicalize(&path) {
			Ok(full) => fullpaths.push(full.display().to_string()),
			Err(e) => eprintln!("realpath: {path}: {e}"),
		}
	}

	let commands: Vec<Value> = fullpaths
		.iter()
		.map(|p| json!(["loadfile", p, "append-play"]))
		.collect();
	if !commands.is_empty() {
		cmd_quiet(&ctx.sock, &commands)?;
	}
	playlist::append(&ctx.playlist_file, &fullpaths)?;
	Ok(())
}

pub(crate) fn delete(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let from = parse_index(
		args.first()
			.ok_or_else(|| "del needs an index or a range".to_string())?,
	)?;
	let removes = match args.get(1) {
		Some(end) => {
			let end = parse_index(end)?;
			let count = end - from + 1;
			if count < 1 {
				return Err(format!("invalid range: {from}..{end}").into());
			}
			count
		}
		None => 1,
	};
	let commands: Vec<Value> = (0..removes).map(|_| json!(["playlist-remove", from])).collect();
	playlist::run_commands_refresh(&ctx.sock, &commands, &ctx.playlist_file).map_err(|e| -> Box<dyn Error> { e.into() })
}

fn shuffle(ctx: &Ctx) -> Result<(), Box<dyn Error>> {
	playlist::run_commands_refresh(&ctx.sock, &[json!(["playlist-shuffle"])], &ctx.playlist_file)
		.map_err(|e| -> Box<dyn Error> { e.into() })
}

pub(crate) fn r#move(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let first = args
		.first()
		.ok_or_else(|| "move needs a range and an index".to_string())?;
	let target = parse_index(
		args.get(1)
			.ok_or_else(|| "move needs a range and an index".to_string())?,
	)?;
	// first argument: single index or an inclusive "a-b" range
	let range = first.split_once('-').and_then(|(x, y)| {
		let (x, y) = (x.parse::<i64>().ok()?, y.parse::<i64>().ok()?);
		Some((x, y))
	});
	let (a, b) = if let Some((a, b)) = range {
		(a, b)
	} else {
		let i = parse_index(first)?;
		(i, i)
	};
	if a < 0 || b < 0 || target < 0 {
		return Err("move indexes must be non-negative".into());
	}
	if a > b {
		return Err(format!("invalid range: {a}-{b}").into());
	}
	let len = playlist::filenames(&ctx.sock)?.len();
	#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
	let (a, b, target) = (a as usize, b as usize, target as usize);
	let cmds = playlist::move_range_commands(len, a, b, Some(target))?;
	playlist::run_commands_refresh(&ctx.sock, &cmds, &ctx.playlist_file).map_err(|e| -> Box<dyn Error> { e.into() })
}

fn save(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let file = args.first().ok_or_else(|| "save needs a file".to_string())?;
	let filenames = playlist::filenames(&ctx.sock)?;
	playlist::write_lines(Path::new(file), &filenames)?;
	Ok(())
}

fn load(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let name = args.first().ok_or_else(|| "load needs a playlist".to_string())?;
	let file = resolve_playlist(ctx, name);
	playlist::load(&file, &ctx.playlist_file).map_err(|e| format!("{}: {e}", file.display()).into())
}

/// Resolve a playlist argument: names without a slash are looked up in the
/// playlists directory (with or without the .m3u extension), anything else
/// is used as a path so playlists outside the directory keep working.
pub(crate) fn resolve_playlist(ctx: &Ctx, name: &str) -> PathBuf {
	if name.contains('/') {
		return PathBuf::from(name);
	}
	let dir = &ctx.playlists_dir;
	let direct = dir.join(name);
	if direct.is_file() {
		return direct;
	}
	let with_ext = dir.join(format!("{name}.m3u"));
	if with_ext.is_file() {
		return with_ext;
	}
	if let Some(stem) = name.strip_suffix(".m3u") {
		let stripped = dir.join(stem);
		if stripped.is_file() {
			return stripped;
		}
	}
	PathBuf::from(name)
}

/// List the playlist files available in the playlists directory.
fn list_playlists(ctx: &Ctx) -> Result<(), Box<dyn Error>> {
	if !ctx.playlists_dir.is_dir() {
		return Ok(());
	}
	for name in playlist::dir_entries(&ctx.playlists_dir)? {
		println!("{name}");
	}
	Ok(())
}

fn list(ctx: &Ctx) -> Result<(), Box<dyn Error>> {
	let requests = [
		json!({"request_id": 1, "command": ["get_property", "playlist"]}),
		json!({"request_id": 2, "command": ["get_property", "pause"]}),
		json!({"request_id": 3, "command": ["get_property", "playback-time"]}),
		json!({"request_id": 4, "command": ["get_property", "duration"]}),
	];
	let lines = ipc::Client::connect(&ctx.sock)?.send(&requests)?;

	let mut playlist_entries = Vec::new();
	let mut paused = None;
	let mut playback: Option<f64> = None;
	let mut duration: Option<f64> = None;
	for line in lines {
		let Ok(v) = serde_json::from_str::<Value>(&line) else {
			continue;
		};
		match v.get("request_id").and_then(Value::as_u64) {
			Some(1) if v.get("error").and_then(Value::as_str) == Some(ipc::SUCCESS) => {
				playlist_entries = v.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
			}
			Some(2) => paused = v.get("data").and_then(Value::as_bool),
			Some(3) => playback = v.get("data").and_then(Value::as_f64),
			Some(4) => duration = v.get("data").and_then(Value::as_f64),
			_ => {}
		}
	}

	let (bold, normal) = if have_terminal() { (BOLD, NORMAL) } else { ("", "") };

	println!("Playlist:");
	for (i, entry) in playlist_entries.iter().enumerate() {
		let name = entry.get("filename").and_then(Value::as_str).map_or("", display_name);
		if entry.get("current").and_then(Value::as_bool) == Some(true) {
			println!("{bold}[{i}] {name} [*]{normal}");
		} else {
			println!("[{i}] {name}");
		}
	}

	let current = playlist_entries
		.iter()
		.find(|e| e.get("current").and_then(Value::as_bool) == Some(true))
		.map_or("", |e| {
			e.get("filename").and_then(Value::as_str).map_or("", display_name)
		});

	let is_paused = paused.unwrap_or(false) || playlist_entries.is_empty();
	let (pb, du) = playlist_times(playback, duration);
	println!(
		"\nstatus: {}, time: {}:{}:{}/{}:{}:{}, file: {}",
		if is_paused { "paused" } else { "playing" },
		pb / 3600,
		pb % 3600 / 60,
		pb % 60,
		du / 3600,
		du % 3600 / 60,
		du % 60,
		current
	);
	Ok(())
}

fn log(ctx: &Ctx) -> Result<(), Box<dyn Error>> {
	let content = fs::read_to_string(&ctx.log_file).map_err(|e| format!("{}: {e}", ctx.log_file.display()))?;
	print!("{content}");
	Ok(())
}

fn status(ctx: &Ctx) {
	if let Some(pid) = daemon::find_pid(&ctx.sock) {
		println!("mpv server is running ({pid})");
	} else {
		println!("mpv server is down");
		exit(1);
	}
}

fn run(ctx: &Ctx, args: &[String]) -> Result<(), Box<dyn Error>> {
	let Some(cmd_arg) = args.first() else {
		return list(ctx);
	};
	let cmd_arg = cmd_arg.as_str();
	let rest = &args[1..];

	match cmd_arg {
		"O" | "prop" => cmd(ctx, "get_property", rest),
		"C" | "cmd" => match rest.first() {
			Some(name) => cmd(ctx, name, &rest[1..]),
			None => Err("cmd needs a command name".into()),
		},
		"p" | "play" => cmd(ctx, "set_property", &["pause".into(), "false".into()]),
		"s" | "pause" => cmd(ctx, "set_property", &["pause".into(), "true".into()]),
		"T" | "toggle" => toggle(ctx),
		"c" | "clear" => {
			cmd(ctx, "stop", &[])?;
			fs::File::create(&ctx.playlist_file)?;
			Ok(())
		}
		"N" | "next" => cmd(ctx, "playlist-next", &[]),
		"P" | "prev" => cmd(ctx, "playlist-prev", &[]),
		"j" | "jump" => {
			let idx = rest.first().ok_or_else(|| "jump needs an index".to_string())?;
			cmd(ctx, "set_property", &["playlist-pos".into(), idx.clone()])
		}
		"t" | "time" => {
			let pos = rest.first().ok_or_else(|| "time needs a position".to_string())?;
			cmd(ctx, "set_property", &["time-pos".into(), pos.clone()])
		}
		"e" | "seek" => {
			let amount = rest.first().ok_or_else(|| "seek needs an amount".to_string())?;
			cmd(ctx, "add", &["time-pos".into(), amount.clone()])
		}
		"a" | "add" => append(ctx, rest),
		"d" | "del" | "delete" => delete(ctx, rest),
		"m" | "move" => r#move(ctx, rest),
		"S" | "shuffle" => shuffle(ctx),
		"l" | "ls" => list(ctx),
		"save" => save(ctx, rest),
		"load" => load(ctx, rest),
		"pl" | "playlists" => list_playlists(ctx),
		"start" => {
			daemon::start(&ctx.sock, &ctx.playlist_file, &ctx.log_file).map_err(|e| -> Box<dyn Error> { e.into() })
		}
		"stop" => {
			daemon::stop(&ctx.sock);
			Ok(())
		}
		"restart" => {
			daemon::stop(&ctx.sock);
			daemon::start(&ctx.sock, &ctx.playlist_file, &ctx.log_file).map_err(|e| -> Box<dyn Error> { e.into() })
		}
		"status" => {
			status(ctx);
			Ok(())
		}
		"log" => log(ctx),
		"h" | "help" | "-h" | "--help" => {
			usage(&ctx.progname);
			Ok(())
		}
		_ => {
			usage(&ctx.progname);
			exit(1);
		}
	}
}

fn main() {
	let config = config::load();
	let ctx = new_ctx(&config);
	let args: Vec<String> = env::args().skip(1).collect();

	// no arguments in a terminal: run the TUI (pipes/scripts still get `list`)
	if args.is_empty() && io::stdin().is_terminal() && io::stdout().is_terminal() {
		let bindings = Keybindings::from_config(&config.keys);
		let theme = Theme::from_config(&config.colors);
		if let Err(e) = tui::run(&ctx, &bindings, &theme) {
			eprintln!("Error: {e}");
			exit(1);
		}
		exit(0);
	}

	// commands that need a running mpv server (starting it if it is down)
	match args.first().map(String::as_str) {
		Some(
			"O" | "prop" | "C" | "cmd" | "p" | "play" | "s" | "pause" | "T" | "toggle" | "c" | "clear" | "N" | "next"
			| "P" | "prev" | "j" | "jump" | "t" | "time" | "e" | "seek" | "a" | "add" | "d" | "del" | "delete" | "m"
			| "move" | "S" | "shuffle" | "l" | "ls" | "save" | "load" | "log",
		)
		| None => daemon::recheck(&ctx.sock, &ctx.playlist_file, &ctx.log_file),
		_ => {}
	}

	if let Err(e) = run(&ctx, &args) {
		eprintln!("Error: {e}");
		exit(1);
	}
}
