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

struct Entry {
	filename: String,
	current: bool,
}

#[derive(Default)]
enum Mode {
	#[default]
	Normal,
	Command(String),
}

struct Message {
	text: String,
	error: bool,
	at: Instant,
}

#[derive(Default)]
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
		if v.get("event").and_then(Value::as_str) == Some("property-change") {
			self.server_up = true;
			let name = v.get("name").and_then(Value::as_str).unwrap_or_default();
			let data = v.get("data");
			match name {
				"playlist" => {
					self.entries = entries_from(data);
					self.selected = self.selected.min(self.entries.len().saturating_sub(1));
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
		terminal.draw(|f| draw(f, &mut app, bindings, theme))?;

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
	if let Mode::Command(input) = &mut app.mode {
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
		return false;
	}

	if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
		return true;
	}
	let action = bindings.action_for(key);

	// while the keymap is open, any of the close keys just closes it
	if app.show_help {
		if action == Some(Action::Help) || matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
			app.show_help = false;
		}
		return false;
	}

	let Some(action) = action else {
		return false;
	};

	let selected = app.selected;
	let len = app.entries.len();
	let obs = observer.as_mut();
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
		Action::Delete if selected < len => {
			mutate(app, ctx, &[json!(["playlist-remove", selected])], "delete");
		}
		Action::MoveUp if selected > 0 => {
			if mutate(app, ctx, &[json!(["playlist-move", selected, selected - 1])], "move") {
				app.selected = selected - 1;
			}
		}
		Action::MoveDown if selected + 1 < len => {
			if mutate(app, ctx, &[json!(["playlist-move", selected, selected + 2])], "move") {
				app.selected = selected + 1;
			}
		}
		Action::Shuffle => {
			mutate(app, ctx, &[json!(["playlist-shuffle"])], "shuffle");
		}
		Action::Clear => {
			if mutate(app, ctx, &[json!(["stop"])], "clear") {
				app.selected = 0;
			}
		}
		Action::Command => app.mode = Mode::Command(String::new()),
		Action::Help => app.show_help = true,
		Action::Quit => return true,
		_ => {}
	}
	false
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

fn draw(f: &mut Frame, app: &mut App, bindings: &Keybindings, theme: &Theme) {
	let area = f.area();
	if area.height < 5 || area.width < 8 {
		return;
	}
	let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(1), Constraint::Length(1)]).split(area);

	// playlist
	let title = if app.entries.is_empty() {
		" mpvctl ".to_owned()
	} else {
		format!(" mpvctl ─ {} tracks ", app.entries.len())
	};
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
			let style = if entry.current {
				Style::new().fg(theme.current)
			} else {
				Style::new()
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
	f.render_stateful_widget(list, rows[0], &mut app.list_state);

	// status bar (or command prompt)
	let bar_style = Style::new().fg(theme.status_fg).bg(theme.status_bg);
	let bar = match &app.mode {
		Mode::Command(input) => Line::from(format!(":{input}▌")),
		Mode::Normal => status_line(app, theme, bindings),
	};
	f.render_widget(Paragraph::new(bar).style(bar_style), rows[1]);

	if app.show_help {
		draw_help(f, area, bindings, theme);
	}
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
	let footer = format!(
		" {} prompt: seek time jump del move load save prop cmd restart ",
		key_label(bindings.command)
	);
	#[allow(clippy::cast_possible_truncation)]
	let height = (half as u16 + 3).min(area.height.saturating_sub(2));

	let popup = centered_rect(area, width, height);
	f.render_widget(ratatui::widgets::Clear, popup);
	let mut lines = Vec::new();
	for row in 0..half {
		let (mut k2, mut d2) = (String::new(), String::new());
		let (k1, d1) = entries[row].clone();
		if let Some((k, d)) = entries.get(row + half) {
			k2.clone_from(k);
			d2.clone_from(d);
		}
		lines.push(Line::from(format!(
			"{k1:<key_w$} {d1:<desc_w$} {k2:<key_w$} {d2:<desc_w$}"
		)));
	}
	lines.push(Line::from(footer));
	let block = Block::bordered()
		.title(" keymap ")
		.border_style(Style::new().fg(theme.border))
		.style(Style::new().bg(theme.status_bg));
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
	Line::from(vec![
		Span::raw(format!(" {glyph} {state} ")),
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
