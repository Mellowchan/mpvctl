//! Interactive TUI for controlling the mpv daemon.
//!
//! Started by running `mpvctl` without arguments in a terminal. It keeps a
//! persistent IPC connection with observed properties, so the display
//! updates in realtime while it is open.

use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use serde_json::{Value, json};

use crate::Ctx;
use crate::config::{Action, Keybindings, Theme, key_label};
use crate::daemon;
use crate::ipc::{self, IpcMsg, Observer};
use crate::playlist;

/// Observed properties and their ids on the persistent connection.
const PROPERTIES: [(&str, u64); 4] = [("playlist", 1), ("pause", 2), ("playback-time", 3), ("duration", 4)];

const TICK: Duration = Duration::from_millis(100);
const RECONNECT: Duration = Duration::from_secs(1);
const MESSAGE_TTL: Duration = Duration::from_secs(3);
/// Seconds added/removed by the seek keys.
const SEEK_STEP: i64 = 5;
/// Log level requested from mpv while the log view is open.
const LOG_LEVEL: &str = "info";
/// Maximum number of buffered log lines.
const LOG_BUFFER: usize = 2000;

struct Entry {
	filename: String,
	current: bool,
}

#[derive(Default)]
enum Mode {
	#[default]
	Normal,
	/// visual selection from an anchor entry to the cursor
	Visual {
		anchor: usize,
	},
	Command(String),
}

/// Which list the main area shows.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum View {
	#[default]
	Playlist,
	Log,
	Playlists,
}

struct Message {
	text: String,
	error: bool,
	at: Instant,
}

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct App {
	entries: Vec<Entry>,
	paused: bool,
	playback_time: Option<f64>,
	duration: Option<f64>,
	selected: usize,
	mode: Mode,
	message: Option<Message>,
	server_up: bool,
	list_state: ListState,
	/// playlists browser state
	pls_names: Vec<String>,
	pls_state: ListState,
	pls_selected: usize,
	pls_marked: std::collections::BTreeSet<usize>,
	view: View,
	/// ring buffer of recent daemon log lines
	log_lines: Vec<String>,
	/// follow the log tail (false once the user scrolled up)
	log_follow: bool,
	/// lines scrolled up from the tail when not following
	log_offset: usize,
	show_help: bool,
	/// descriptions of commands awaiting a response, by request id
	waiting: HashMap<u64, String>,
}

impl App {
	fn new(server_up: bool) -> Self {
		Self {
			server_up,
			..Self::default()
		}
	}

	fn message(&mut self, text: impl Into<String>, error: bool) {
		self.message = Some(Message {
			text: text.into(),
			error,
			at: Instant::now(),
		});
	}

	fn current_index(&self) -> Option<usize> {
		self.entries.iter().position(|e| e.current)
	}

	/// The selected range in visual mode (anchor..=cursor), if active.
	fn visual_range(&self) -> Option<(usize, usize)> {
		match self.mode {
			Mode::Visual { anchor } => Some((anchor.min(self.selected), anchor.max(self.selected))),
			_ => None,
		}
	}

	/// Move the selection by `delta` entries, clamped to the list.
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
	fn select(&mut self, delta: i64) {
		if self.entries.is_empty() {
			self.selected = 0;
		} else {
			let last = self.entries.len() as i64 - 1;
			self.selected = (self.selected as i64 + delta).clamp(0, last) as usize;
		}
	}

	/// Send a command through the observer; errors show up once the
	/// response arrives (matched by request id).
	fn command(&mut self, observer: Option<&mut Observer>, cmd: &Value, desc: &str) {
		let Some(observer) = observer else {
			self.message("mpv server is down", true);
			return;
		};
		match observer.send_tracked(cmd) {
			Ok(id) => {
				self.waiting.insert(id, desc.to_owned());
			}
			Err(e) => self.message(format!("{desc}: {e}"), true),
		}
	}

	/// Apply a JSON message received from mpv.
	fn apply(&mut self, v: &Value) {
		if v.get("event").and_then(Value::as_str) == Some("log-message") {
			if let Some(text) = v.get("text").and_then(Value::as_str) {
				self.log_lines.push(text.trim_end().to_owned());
				if self.log_lines.len() > LOG_BUFFER {
					self.log_lines.drain(..self.log_lines.len() - LOG_BUFFER);
				}
			}
			return;
		}
		if v.get("event").and_then(Value::as_str) == Some("property-change") {
			self.server_up = true;
			let name = v.get("name").and_then(Value::as_str).unwrap_or_default();
			let data = v.get("data");
			match name {
				"playlist" => {
					self.entries = entries_from(data);
					let last = self.entries.len().saturating_sub(1);
					self.selected = self.selected.min(last);
					if let Mode::Visual { anchor } = &mut self.mode {
						*anchor = (*anchor).min(last);
					}
					if self.entries.is_empty() {
						self.mode = Mode::Normal;
					}
				}
				"pause" => self.paused = data.and_then(Value::as_bool).unwrap_or(false),
				"playback-time" => self.playback_time = data.and_then(Value::as_f64),
				"duration" => self.duration = data.and_then(Value::as_f64),
				_ => {}
			}
			return;
		}
		if let Some(id) = v.get("request_id").and_then(Value::as_u64)
			&& let Some(desc) = self.waiting.remove(&id)
		{
			let error = v.get("error").and_then(Value::as_str).unwrap_or_default();
			if error != ipc::SUCCESS {
				self.message(format!("{desc}: {error}"), true);
			}
		}
	}
}

fn entries_from(data: Option<&Value>) -> Vec<Entry> {
	data.and_then(Value::as_array)
		.map(|entries| {
			entries
				.iter()
				.map(|e| Entry {
					filename: e.get("filename").and_then(Value::as_str).unwrap_or_default().to_owned(),
					current: e.get("current").and_then(Value::as_bool).unwrap_or(false),
				})
				.collect()
		})
		.unwrap_or_default()
}

/// Run the TUI until the user quits.
pub fn run(ctx: &Ctx, bindings: &Keybindings, theme: &Theme) -> Result<(), Box<dyn Error>> {
	let mut terminal = ratatui::try_init()?;
	let result = event_loop(&mut terminal, ctx, bindings, theme);
	ratatui::try_restore()?;
	result
}

fn connect(ctx: &Ctx) -> Option<Observer> {
	let _ = daemon::ensure_running(&ctx.sock, &ctx.playlist_file, &ctx.log_file);
	Observer::connect(&ctx.sock, &PROPERTIES).ok()
}

fn event_loop(
	terminal: &mut DefaultTerminal,
	ctx: &Ctx,
	bindings: &Keybindings,
	theme: &Theme,
) -> Result<(), Box<dyn Error>> {
	let mut observer = connect(ctx);
	let mut app = App::new(observer.is_some());
	let mut next_try = Instant::now();

	loop {
		app.list_state.select(Some(app.selected));
		terminal.draw(|f| draw(f, &mut app, ctx, bindings, theme))?;

		if event::poll(TICK)?
			&& let Event::Key(key) = event::read()?
			&& key.kind == KeyEventKind::Press
			&& handle_key(key, &mut app, ctx, &mut observer, bindings)
		{
			return Ok(());
		}

		let mut closed = false;
		if let Some(observer) = observer.as_ref() {
			while let Some(msg) = observer.poll() {
				match msg {
					IpcMsg::Json(v) => app.apply(&v),
					IpcMsg::Closed => {
						closed = true;
						break;
					}
				}
			}
		}
		if closed {
			observer = None;
			app.server_up = false;
			app.waiting.clear();
			app.message("mpv server is down", true);
		} else if observer.is_none() && Instant::now() >= next_try {
			next_try = Instant::now() + RECONNECT;
			if let Ok(new_observer) = Observer::connect(&ctx.sock, &PROPERTIES) {
				app.server_up = true;
				// resubscribe to log messages if the log view is open
				if app.view == View::Log {
					let _ = new_observer.send(&json!({"command": ["request_log_messages", LOG_LEVEL]}));
				}
				observer = Some(new_observer);
			}
		}
	}
}

/// Handle a key press. Returns true to quit.
fn handle_key(
	key: KeyEvent,
	app: &mut App,
	ctx: &Ctx,
	observer: &mut Option<Observer>,
	bindings: &Keybindings,
) -> bool {
	if matches!(app.mode, Mode::Command(_)) {
		return handle_command_key(key, app, ctx, observer);
	}

	if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
		return true;
	}
	let action = bindings.action_for(key);

	if app.view == View::Log {
		return handle_log_key(key, action, app, observer);
	}
	if app.view == View::Playlists {
		return handle_playlists_key(key, action, app, ctx);
	}

	// while the keymap is open, any of the close keys just closes it
	if app.show_help {
		if action == Some(Action::Help) || matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
			app.show_help = false;
		}
		return false;
	}

	if matches!(key.code, KeyCode::Esc) && app.visual_range().is_some() {
		app.mode = Mode::Normal;
		return false;
	}

	let Some(action) = action else {
		return false;
	};
	handle_playlist_key(action, app, ctx, observer)
}

/// Key handling for the playlist view (normal and visual mode).
fn handle_playlist_key(action: Action, app: &mut App, ctx: &Ctx, observer: &mut Option<Observer>) -> bool {
	let selected = app.selected;
	let len = app.entries.len();
	let obs = observer.as_mut();
	// range the destructive operations apply to: the visual selection or
	// just the cursor
	let (a, b) = app.visual_range().unwrap_or((selected, selected));
	match action {
		Action::Up => app.select(-1),
		Action::Down => app.select(1),
		Action::Top => app.selected = 0,
		Action::Bottom => app.selected = len.saturating_sub(1),
		Action::Jump if !app.entries.is_empty() => {
			app.command(obs, &json!(["set_property", "playlist-pos", selected]), "jump");
		}
		Action::Toggle => app.command(obs, &json!(["set_property", "pause", !app.paused]), "pause"),
		Action::Play => app.command(obs, &json!(["set_property", "pause", false]), "play"),
		Action::Pause => app.command(obs, &json!(["set_property", "pause", true]), "pause"),
		Action::Prev => app.command(obs, &json!(["playlist-prev"]), "prev"),
		Action::Next => app.command(obs, &json!(["playlist-next"]), "next"),
		Action::SeekBack => app.command(obs, &json!(["add", "time-pos", -SEEK_STEP]), "seek"),
		Action::SeekFwd => app.command(obs, &json!(["add", "time-pos", SEEK_STEP]), "seek"),
		Action::Visual => {
			app.mode = if app.visual_range().is_some() {
				Mode::Normal
			} else {
				Mode::Visual { anchor: app.selected }
			};
		}
		Action::Delete if b < len => {
			let cmds: Vec<Value> = (0..=b - a).map(|_| json!(["playlist-remove", a])).collect();
			if mutate(app, ctx, &cmds, "delete") {
				app.mode = Mode::Normal;
				app.selected = a;
			}
		}
		Action::MoveUp if a > 0 => match playlist::move_range_commands(len, a, b, Some(a - 1)) {
			Ok(cmds) => {
				if mutate(app, ctx, &cmds, "move") {
					app.selected = selected.saturating_sub(1);
					if let Mode::Visual { anchor } = &mut app.mode {
						*anchor -= 1;
					}
				}
			}
			Err(e) => app.message(format!("move: {e}"), true),
		},
		Action::MoveDown if b + 1 < len => {
			let before = if b + 2 < len { Some(b + 2) } else { None };
			match playlist::move_range_commands(len, a, b, before) {
				Ok(cmds) => {
					if mutate(app, ctx, &cmds, "move") {
						app.selected = selected + 1;
						if let Mode::Visual { anchor } = &mut app.mode {
							*anchor += 1;
						}
					}
				}
				Err(e) => app.message(format!("move: {e}"), true),
			}
		}
		Action::Shuffle => {
			mutate(app, ctx, &[json!(["playlist-shuffle"])], "shuffle");
		}
		Action::Clear => {
			if mutate(app, ctx, &[json!(["stop"])], "clear") {
				app.selected = 0;
				app.mode = Mode::Normal;
			}
		}
		Action::LogView => open_log(app, observer),
		Action::Browser => open_playlists(app, ctx),
		Action::Command => app.mode = Mode::Command(String::new()),
		Action::Help => app.show_help = true,
		Action::Quit => return true,
		_ => {}
	}
	false
}

/// Key handling for the ':' prompt.
fn handle_command_key(key: KeyEvent, app: &mut App, ctx: &Ctx, observer: &mut Option<Observer>) -> bool {
	let Mode::Command(ref mut input) = app.mode else {
		return false;
	};
	match key.code {
		KeyCode::Esc => app.mode = Mode::Normal,
		KeyCode::Enter => {
			let line = std::mem::take(input);
			app.mode = Mode::Normal;
			if run_command(&line, app, ctx, observer) {
				return true;
			}
		}
		KeyCode::Backspace => {
			input.pop();
		}
		KeyCode::Char(c) => input.push(c),
		_ => {}
	}
	false
}

/// Key handling for the log view: scroll with the movement keys, close with
/// the log key or esc.
fn handle_log_key(key: KeyEvent, action: Option<Action>, app: &mut App, observer: &mut Option<Observer>) -> bool {
	match action {
		Some(Action::Quit) => true,
		Some(Action::Command) => {
			app.mode = Mode::Command(String::new());
			false
		}
		Some(Action::LogView) => {
			close_log(app, observer);
			false
		}
		Some(Action::Up) => {
			app.log_follow = false;
			app.log_offset = (app.log_offset + 1).min(app.log_lines.len());
			false
		}
		Some(Action::Down) => {
			app.log_offset = app.log_offset.saturating_sub(1);
			app.log_follow = app.log_offset == 0;
			false
		}
		Some(Action::Top) => {
			app.log_follow = false;
			app.log_offset = app.log_lines.len();
			false
		}
		Some(Action::Bottom) => {
			app.log_follow = true;
			app.log_offset = 0;
			false
		}
		_ if matches!(key.code, KeyCode::Esc) => {
			close_log(app, observer);
			false
		}
		_ => false,
	}
}

/// Open the playlists browser and (re)load its listing.
fn open_playlists(app: &mut App, ctx: &Ctx) {
	app.view = View::Playlists;
	reload_playlists(app, ctx);
}

fn reload_playlists(app: &mut App, ctx: &Ctx) {
	app.pls_names = playlist::dir_entries(&ctx.playlists_dir).unwrap_or_default();
	app.pls_selected = 0;
	app.pls_marked.clear();
}

/// The indexes to load: the marked entries, or just the cursor.
fn playlists_to_load(app: &App) -> Vec<usize> {
	if app.pls_marked.is_empty() {
		vec![app.pls_selected]
	} else {
		app.pls_marked.iter().copied().collect()
	}
}

/// Load the chosen playlists into the main playlist: appended, or replacing
/// everything when `replace` is set. Returns an error message on failure.
fn load_playlists(app: &mut App, ctx: &Ctx, replace: bool) -> Result<String, String> {
	let indexes = playlists_to_load(app);
	let mut names = Vec::new();
	let mut cmds = Vec::new();
	for (n, &i) in indexes.iter().enumerate() {
		let Some(name) = app.pls_names.get(i) else { continue };
		let path = ctx.playlists_dir.join(name);
		let mode = if replace && n == 0 { "replace" } else { "append-play" };
		cmds.push(json!(["loadlist", path.display().to_string(), mode]));
		names.push(name.clone());
	}
	if cmds.is_empty() {
		return Err("no playlists to load".to_owned());
	}
	let count = cmds.len();
	playlist::run_commands_refresh(&ctx.sock, &cmds, &ctx.playlist_file).map_err(|e| e.to_string())?;
	app.view = View::Playlist;
	Ok(if replace {
		format!("loaded {count} playlist(s) (replaced)")
	} else {
		format!("appended {count} playlist(s)")
	})
}

/// Key handling for the playlists browser: cursor + marks, a/enter appends,
/// o overwrites, r reloads, b/esc closes.
#[allow(clippy::too_many_lines)]
fn handle_playlists_key(key: KeyEvent, action: Option<Action>, app: &mut App, ctx: &Ctx) -> bool {
	let len = app.pls_names.len();
	match action {
		Some(Action::Quit) => true,
		Some(Action::Command) => {
			app.mode = Mode::Command(String::new());
			false
		}
		Some(Action::Browser) | None if matches!(key.code, KeyCode::Esc) => {
			app.view = View::Playlist;
			false
		}
		Some(Action::Browser) => {
			app.view = View::Playlist;
			false
		}
		Some(Action::Up) => {
			app.pls_selected = app.pls_selected.saturating_sub(1);
			false
		}
		Some(Action::Down) if len > 0 => {
			app.pls_selected = (app.pls_selected + 1).min(len - 1);
			false
		}
		Some(Action::Top) => {
			app.pls_selected = 0;
			false
		}
		Some(Action::Bottom) if len > 0 => {
			app.pls_selected = len - 1;
			false
		}
		Some(Action::Mark) if len > 0 => {
			if !app.pls_marked.insert(app.pls_selected) {
				app.pls_marked.remove(&app.pls_selected);
			}
			false
		}
		Some(Action::Refresh) => {
			reload_playlists(app, ctx);
			false
		}
		Some(Action::Append | Action::Jump) if len > 0 => {
			match load_playlists(app, ctx, false) {
				Ok(msg) => {
					app.message(msg, false);
				}
				Err(e) => app.message(e, true),
			}
			false
		}
		Some(Action::Overwrite) if len > 0 => {
			match load_playlists(app, ctx, true) {
				Ok(msg) => {
					app.message(msg, false);
				}
				Err(e) => app.message(e, true),
			}
			false
		}
		_ => false,
	}
}

/// Open the log view and subscribe to daemon log messages.
fn open_log(app: &mut App, observer: &mut Option<Observer>) {
	app.view = View::Log;
	app.log_follow = true;
	app.log_offset = 0;
	if let Some(observer) = observer.as_ref() {
		let _ = observer.send(&json!({"command": ["request_log_messages", LOG_LEVEL]}));
	}
}

/// Close the log view and stop the log message stream.
fn close_log(app: &mut App, observer: &mut Option<Observer>) {
	app.view = View::Playlist;
	if let Some(observer) = observer.as_ref() {
		let _ = observer.send(&json!({"command": ["request_log_messages", "no"]}));
	}
}

/// Run commands that change the playlist and refresh the playlist file in
/// one round trip. Returns true on success.
fn mutate(app: &mut App, ctx: &Ctx, cmds: &[Value], desc: &str) -> bool {
	if let Err(e) = playlist::run_commands_refresh(&ctx.sock, cmds, &ctx.playlist_file) {
		app.message(format!("{desc}: {e}"), true);
		return false;
	}
	true
}

/// ok with an optional status message, or an error text to display
type CmdResult = Result<Option<String>, String>;

/// Check a one-shot command response, mapping transport and mpv errors to
/// a message.
fn checked(desc: &str, resp: io::Result<Value>) -> CmdResult {
	match resp {
		Ok(v) if v.get("error").and_then(Value::as_str) == Some(ipc::SUCCESS) => Ok(None),
		Ok(v) => Err(format!(
			"{desc}: {}",
			v.get("error").and_then(Value::as_str).unwrap_or("unknown error")
		)),
		Err(e) => Err(format!("{desc}: {e}")),
	}
}

/// Execute a ':' command line. Returns true to quit.
fn run_command(line: &str, app: &mut App, ctx: &Ctx, observer: &mut Option<Observer>) -> bool {
	let mut args = line.split_whitespace();
	let Some(name) = args.next() else {
		return false;
	};
	let rest: Vec<String> = args.map(str::to_owned).collect();

	let result: CmdResult = match name {
		"q" | "quit" => return true,
		"restart" => {
			daemon::stop(&ctx.sock);
			match daemon::ensure_running(&ctx.sock, &ctx.playlist_file, &ctx.log_file)
				.and_then(|()| Observer::connect(&ctx.sock, &PROPERTIES).map_err(io::Error::other))
			{
				Ok(new_observer) => {
					app.server_up = true;
					app.waiting.clear();
					*observer = Some(new_observer);
					Ok(Some("mpv server restarted".to_owned()))
				}
				Err(e) => Err(format!("restart: {e}")),
			}
		}
		"seek" => rest
			.first()
			.ok_or_else(|| "seek needs seconds".to_owned())
			.and_then(|v| v.parse::<f64>().map_err(|_| "invalid seconds".to_owned()))
			.and_then(|n| checked("seek", ipc::command(&ctx.sock, &json!(["add", "time-pos", n])))),
		"time" => rest
			.first()
			.ok_or_else(|| "time needs seconds".to_owned())
			.and_then(|v| v.parse::<f64>().map_err(|_| "invalid seconds".to_owned()))
			.and_then(|n| {
				checked("time", ipc::command(&ctx.sock, &json!(["set_property", "time-pos", n])))
			}),
		"jump" => rest
			.first()
			.ok_or_else(|| "jump needs an index".to_owned())
			.and_then(|v| v.parse::<i64>().map_err(|_| "invalid index".to_owned()))
			.and_then(|i| {
				checked(
					"jump",
					ipc::command(&ctx.sock, &json!(["set_property", "playlist-pos", i])),
				)
			}),
		"del" => crate::delete(ctx, &rest)
			.map_err(|e| format!("del: {e}"))
			.map(|()| None),
		"move" => crate::r#move(ctx, &rest)
			.map_err(|e| format!("move: {e}"))
			.map(|()| None),
		"load" => match rest.first() {
			Some(name) => {
				let file = crate::resolve_playlist(ctx, name);
				playlist::load(&file, &ctx.playlist_file)
					.map_err(|e| format!("load: {}: {e}", file.display()))
					.map(|()| Some(format!("loaded {}", file.display())))
			}
			None => Err("load needs a playlist".to_owned()),
		},
		"save" => match rest.first() {
			Some(file) => playlist::filenames(&ctx.sock)
				.and_then(|names| {
					playlist::write_lines(std::path::Path::new(file), &names).map(|()| names.len())
				})
				.map_err(|e| format!("save: {e}"))
				.map(|count| Some(format!("saved {count} tracks"))),
			None => Err("save needs a file".to_owned()),
		},
		"prop" => match rest.first() {
			Some(prop) => match ipc::get_property(&ctx.sock, prop) {
				Ok(v) if v.get("error").and_then(Value::as_str) == Some(ipc::SUCCESS) => {
					Ok(Some(format!("{prop} = {}", v.get("data").unwrap_or(&Value::Null))))
				}
				Ok(v) => Err(format!(
					"prop: {}",
					v.get("error").and_then(Value::as_str).unwrap_or("unknown error")
				)),
				Err(e) => Err(format!("prop: {e}")),
			},
			None => Err("prop needs a property".to_owned()),
		},
		"cmd" => match rest.first() {
			Some(cmd_name) => checked(
				"cmd",
				ipc::command(&ctx.sock, &ipc::build_command(cmd_name, &rest[1..])),
			),
			None => Err("cmd needs a command".to_owned()),
		},
		"help" => Ok(Some(
			"commands: seek <s> | time <s> | jump <i> | del <i> [j] | move <i> <j> | load <name> | save <file> | prop <name> | cmd <name> [args] | restart | quit"
				.to_owned(),
		)),
		_ => Err(format!("unknown command: {name}")),
	};

	match result {
		Ok(Some(msg)) => app.message(msg, false),
		Ok(None) => {}
		Err(e) => app.message(e, true),
	}
	false
}

fn draw(f: &mut Frame, app: &mut App, ctx: &Ctx, bindings: &Keybindings, theme: &Theme) {
	let area = f.area();
	if area.height < 5 || area.width < 8 {
		return;
	}
	let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);

	match app.view {
		View::Log => draw_log(f, rows[0], app, theme),
		View::Playlists => draw_playlists(f, rows[0], app, ctx, theme),
		View::Playlist => draw_playlist(f, rows[0], app, theme),
	}

	// status bar (or command prompt)
	let bar_style = Style::new().fg(theme.status_fg).bg(theme.status_bg);
	let bar = match &app.mode {
		Mode::Command(input) => Line::from(format!(":{input}▌")),
		Mode::Normal | Mode::Visual { .. } => status_line(app, theme, bindings),
	};
	f.render_widget(Paragraph::new(bar).style(bar_style), rows[1]);

	if app.show_help {
		draw_help(f, area, bindings, theme);
	}
}

/// The playlist view.
fn draw_playlist(f: &mut Frame, area: ratatui::layout::Rect, app: &mut App, theme: &Theme) {
	let title = if app.entries.is_empty() {
		" mpvctl ".to_owned()
	} else {
		format!(" mpvctl ─ {} tracks ", app.entries.len())
	};
	let range = app.visual_range();
	let items: Vec<ListItem> = app
		.entries
		.iter()
		.enumerate()
		.map(|(i, entry)| {
			let name = crate::display_name(&entry.filename);
			let text = if entry.current {
				format!("[{i}] {name} [*]")
			} else {
				format!("[{i}] {name}")
			};
			// visual range members (except the cursor row, which the list
			// highlight styles) get the selection background
			let style = if entry.current {
				Style::new().fg(theme.current)
			} else {
				Style::new()
			};
			let style = if let Some((a, b)) = range
				&& i >= a && i <= b
				&& i != app.selected
			{
				style.bg(theme.selected_bg)
			} else {
				style
			};
			ListItem::new(text).style(style)
		})
		.collect();
	let list = List::new(items)
		.block(
			Block::bordered()
				.title(title)
				.border_style(Style::new().fg(theme.border)),
		)
		.highlight_style(Style::new().fg(theme.selected_fg).bg(theme.selected_bg));
	f.render_stateful_widget(list, area, &mut app.list_state);
}

/// The playlists browser view.
fn draw_playlists(f: &mut Frame, area: ratatui::layout::Rect, app: &mut App, ctx: &Ctx, theme: &Theme) {
	let title = format!(" playlists ─ {} ", ctx.playlists_dir.display());
	let items: Vec<ListItem> = app
		.pls_names
		.iter()
		.enumerate()
		.map(|(i, name)| {
			let mark = if app.pls_marked.contains(&i) { "*" } else { " " };
			ListItem::new(format!("{mark} {name}"))
		})
		.collect();
	let list = List::new(items)
		.block(
			Block::bordered()
				.title(title)
				.border_style(Style::new().fg(theme.border)),
		)
		.highlight_style(Style::new().fg(theme.selected_fg).bg(theme.selected_bg));
	app.pls_state.select(Some(app.pls_selected));
	f.render_stateful_widget(list, area, &mut app.pls_state);
}

/// The daemon log view (follows the tail unless the user scrolled up).
fn draw_log(f: &mut Frame, area: ratatui::layout::Rect, app: &App, theme: &Theme) {
	let height = area.height.saturating_sub(2) as usize;
	let total = app.log_lines.len();
	let end = if app.log_follow {
		total
	} else {
		total.saturating_sub(app.log_offset).max(height.min(total))
	};
	let end = end.min(total);
	let start = end.saturating_sub(height);
	let mut lines: Vec<Line> = app.log_lines[start..end]
		.iter()
		.map(|l| Line::from(l.as_str()))
		.collect();
	while lines.len() < height {
		lines.push(Line::from(""));
	}
	let follow = if app.log_follow { "following" } else { "paused" };
	let title = format!(" mpv log ─ {follow} ");
	let block = Block::bordered()
		.title(title)
		.border_style(Style::new().fg(theme.border));
	f.render_widget(Paragraph::new(lines).block(block), area);
}

/// Centered popup with the keymap, shown with the help key.
fn draw_help(f: &mut Frame, area: ratatui::layout::Rect, bindings: &Keybindings, theme: &Theme) {
	let entries = help_entries(bindings);
	let half = entries.len().div_ceil(2);
	let key_w = 8;
	let desc_w = 24;
	let body_w = (key_w + desc_w) * 2;
	#[allow(clippy::cast_possible_truncation)]
	let width = ((body_w + 2) as u16).min(area.width.saturating_sub(2)).max(10);
	#[allow(clippy::cast_possible_truncation)]
	let height = (half as u16 + 3).min(area.height.saturating_sub(2));

	// keys are highlighted, descriptions dim, on a dark background
	let key_style = Style::new().fg(theme.help_key).bg(theme.help_bg).bold();
	let desc_style = Style::new().fg(theme.help_fg).bg(theme.help_bg);
	let pair = |(k, d): &(String, String), kw: usize, dw: usize| {
		vec![
			Span::styled(format!("{k:<kw$}"), key_style),
			Span::styled(format!("{d:<dw$}"), desc_style),
		]
	};
	let mut lines = Vec::new();
	for row in 0..half {
		let mut spans = pair(&entries[row], key_w, desc_w);
		spans.push(Span::raw(" "));
		if let Some(entry) = entries.get(row + half) {
			spans.extend(pair(entry, key_w, desc_w));
		}
		lines.push(Line::from(spans).style(desc_style));
	}
	lines.push(Line::from(format!(
		" {} prompt: seek time jump del move load save prop cmd restart ",
		key_label(bindings.command)
	)));

	let popup = centered_rect(area, width, height);
	f.render_widget(ratatui::widgets::Clear, popup);
	let block = Block::bordered()
		.title(" keymap ")
		.title_bottom(format!(" {} / esc close ", key_label(bindings.help)))
		.border_style(Style::new().fg(theme.help_key).bg(theme.help_bg))
		.style(Style::new().bg(theme.help_bg));
	f.render_widget(Paragraph::new(lines).block(block), popup);
}

/// (key, description) pairs shown in the keymap popup.
fn help_entries(bindings: &Keybindings) -> Vec<(String, String)> {
	let b = bindings;
	[
		(b.up, "select previous"),
		(b.down, "select next"),
		(b.top, "select first"),
		(b.bottom, "select last"),
		(b.jump, "play selected track"),
		(b.toggle, "toggle pause"),
		(b.play, "play"),
		(b.pause, "pause"),
		(b.prev, "previous track"),
		(b.next, "next track"),
		(b.seek_back, "seek 5s back"),
		(b.seek_fwd, "seek 5s forward"),
		(b.delete, "delete entry"),
		(b.move_up, "move entry up"),
		(b.move_down, "move entry down"),
		(b.shuffle, "shuffle playlist"),
		(b.clear, "clear playlist"),
		(b.log, "daemon log view"),
		(b.visual, "visual selection (d/J/K on range)"),
		(b.browser, "playlists browser (m mark, a append, o overwrite)"),
		(b.command, "command prompt"),
		(b.help, "toggle this keymap"),
		(b.quit, "quit"),
	]
	.iter()
	.map(|(k, d)| (key_label(*k), (*d).to_owned()))
	.collect()
}

/// A centered rectangle inside `area` with the given size.
fn centered_rect(area: ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
	let x = area.x + area.width.saturating_sub(width) / 2;
	let y = area.y + area.height.saturating_sub(height) / 2;
	ratatui::layout::Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn status_line(app: &App, theme: &Theme, bindings: &Keybindings) -> Line<'static> {
	if let Some(message) = &app.message
		&& message.at.elapsed() < MESSAGE_TTL
	{
		let fg = if message.error { Color::Red } else { theme.status_fg };
		return Line::from(format!(" {} ", message.text)).style(Style::new().fg(fg).bg(theme.status_bg));
	}
	if !app.server_up {
		return Line::from(" mpv server is down (waiting for it to come up) ")
			.style(Style::new().fg(Color::Red).bg(theme.status_bg));
	}

	let (glyph, state) = if app.entries.is_empty() {
		("■", "idle")
	} else if app.paused {
		("⏸", "paused")
	} else {
		("▶", "playing")
	};
	let time = format!("{}/{}", fmt_secs(app.playback_time), fmt_secs(app.duration));
	let position = app
		.current_index()
		.map_or_else(|| "-".to_owned(), |i| (i + 1).to_string());
	let name = app
		.entries
		.iter()
		.find(|e| e.current)
		.map(|e| crate::display_name(&e.filename).to_owned())
		.unwrap_or_default();
	let help = key_label(bindings.help);
	let visual = if app.visual_range().is_some() {
		Span::raw("VISUAL ")
	} else {
		Span::raw("")
	};
	Line::from(vec![
		Span::raw(format!(" {glyph} {state} ")),
		visual,
		Span::raw(format!("{time} ")),
		Span::raw(format!("{position}/{} ", app.entries.len())),
		Span::raw(name),
		Span::raw(format!(" [{help} help] ")),
	])
}

fn fmt_secs(time: Option<f64>) -> String {
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	fn fmt(v: f64) -> String {
		let s = v.max(0.0).floor() as u64;
		let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
		if h > 0 {
			format!("{h}:{m:02}:{sec:02}")
		} else {
			format!("{m}:{sec:02}")
		}
	}
	time.map_or_else(|| "0:00".to_owned(), fmt)
}
