//! Configuration: keybindings, colors and paths for the TUI, with vi-like
//! defaults and toml overrides from ~/.config/mpvctl/config.toml.

use std::env;
use std::fs;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::Deserialize;

/// A key binding: key code plus ctrl/alt modifiers (shift is implied by the
/// character itself, so `k` and `K` are different keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(pub KeyCode, pub KeyModifiers);

/// Actions that can be bound to keys in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
	Up,
	Down,
	Top,
	Bottom,
	Jump,
	Toggle,
	Play,
	Pause,
	Prev,
	Next,
	SeekBack,
	SeekFwd,
	VolUp,
	VolDown,
	Delete,
	MoveUp,
	MoveDown,
	Shuffle,
	Clear,
	LogView,
	Visual,
	Browser,
	Mark,
	Append,
	Overwrite,
	Refresh,
	Command,
	Help,
	Quit,
}

/// Keybindings for every TUI action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keybindings {
	pub up: Key,
	pub down: Key,
	pub top: Key,
	pub bottom: Key,
	pub jump: Key,
	pub toggle: Key,
	pub play: Key,
	pub pause: Key,
	pub prev: Key,
	pub next: Key,
	pub seek_back: Key,
	pub seek_fwd: Key,
	pub vol_up: Key,
	pub vol_down: Key,
	pub delete: Key,
	pub move_up: Key,
	pub move_down: Key,
	pub shuffle: Key,
	pub clear: Key,
	pub log: Key,
	pub visual: Key,
	pub browser: Key,
	pub mark: Key,
	pub append: Key,
	pub overwrite: Key,
	pub refresh: Key,
	pub command: Key,
	pub help: Key,
	pub quit: Key,
}

impl Default for Keybindings {
	fn default() -> Self {
		fn key(desc: &str) -> Key {
			parse_key(desc).expect("valid default key")
		}
		Self {
			up: key("k"),
			down: key("j"),
			top: key("g"),
			bottom: key("G"),
			jump: key("enter"),
			toggle: key("space"),
			play: key("p"),
			pause: key("s"),
			prev: key("h"),
			next: key("l"),
			seek_back: key("left"),
			seek_fwd: key("right"),
			vol_up: key("+"),
			vol_down: key("-"),
			delete: key("d"),
			move_up: key("K"),
			move_down: key("J"),
			shuffle: key("S"),
			clear: key("c"),
			log: key("L"),
			visual: key("v"),
			browser: key("b"),
			mark: key("m"),
			append: key("a"),
			overwrite: key("o"),
			refresh: key("r"),
			command: key(":"),
			help: key("?"),
			quit: key("q"),
		}
	}
}

impl Keybindings {
	/// Bindings from the defaults, overridden by the toml `[keys]` section.
	pub fn from_config(config: &KeysToml) -> Self {
		fn resolve(desc: &str, value: Option<&str>, default: Key) -> Key {
			match value.map(parse_key) {
				Some(Some(key)) => key,
				Some(None) => {
					eprintln!("mpvctl: ignoring invalid key binding for {desc}");
					default
				}
				None => default,
			}
		}
		let d = Self::default();
		Self {
			up: resolve("up", config.up.as_deref(), d.up),
			down: resolve("down", config.down.as_deref(), d.down),
			top: resolve("top", config.top.as_deref(), d.top),
			bottom: resolve("bottom", config.bottom.as_deref(), d.bottom),
			jump: resolve("jump", config.jump.as_deref(), d.jump),
			toggle: resolve("toggle", config.toggle.as_deref(), d.toggle),
			play: resolve("play", config.play.as_deref(), d.play),
			pause: resolve("pause", config.pause.as_deref(), d.pause),
			prev: resolve("prev", config.prev.as_deref(), d.prev),
			next: resolve("next", config.next.as_deref(), d.next),
			seek_back: resolve("seek_back", config.seek_back.as_deref(), d.seek_back),
			seek_fwd: resolve("seek_fwd", config.seek_fwd.as_deref(), d.seek_fwd),
			vol_up: resolve("vol_up", config.vol_up.as_deref(), d.vol_up),
			vol_down: resolve("vol_down", config.vol_down.as_deref(), d.vol_down),
			delete: resolve("delete", config.delete.as_deref(), d.delete),
			move_up: resolve("move_up", config.move_up.as_deref(), d.move_up),
			move_down: resolve("move_down", config.move_down.as_deref(), d.move_down),
			shuffle: resolve("shuffle", config.shuffle.as_deref(), d.shuffle),
			clear: resolve("clear", config.clear.as_deref(), d.clear),
			log: resolve("log", config.log.as_deref(), d.log),
			visual: resolve("visual", config.visual.as_deref(), d.visual),
			browser: resolve("browser", config.browser.as_deref(), d.browser),
			mark: resolve("mark", config.mark.as_deref(), d.mark),
			append: resolve("append", config.append.as_deref(), d.append),
			overwrite: resolve("overwrite", config.overwrite.as_deref(), d.overwrite),
			refresh: resolve("refresh", config.refresh.as_deref(), d.refresh),
			command: resolve("command", config.command.as_deref(), d.command),
			help: resolve("help", config.help.as_deref(), d.help),
			quit: resolve("quit", config.quit.as_deref(), d.quit),
		}
	}

	/// Find the action bound to a key event.
	#[must_use]
	pub fn action_for(&self, event: KeyEvent) -> Option<Action> {
		let table = [
			(self.up, Action::Up),
			(self.down, Action::Down),
			(self.top, Action::Top),
			(self.bottom, Action::Bottom),
			(self.jump, Action::Jump),
			(self.toggle, Action::Toggle),
			(self.play, Action::Play),
			(self.pause, Action::Pause),
			(self.prev, Action::Prev),
			(self.next, Action::Next),
			(self.seek_back, Action::SeekBack),
			(self.seek_fwd, Action::SeekFwd),
			(self.vol_up, Action::VolUp),
			(self.vol_down, Action::VolDown),
			(self.delete, Action::Delete),
			(self.move_up, Action::MoveUp),
			(self.move_down, Action::MoveDown),
			(self.shuffle, Action::Shuffle),
			(self.clear, Action::Clear),
			(self.log, Action::LogView),
			(self.visual, Action::Visual),
			(self.browser, Action::Browser),
			(self.mark, Action::Mark),
			(self.append, Action::Append),
			(self.overwrite, Action::Overwrite),
			(self.refresh, Action::Refresh),
			(self.command, Action::Command),
			(self.help, Action::Help),
			(self.quit, Action::Quit),
		];
		table
			.iter()
			.find(|(k, _)| key_matches(event, *k))
			.map(|(_, action)| *action)
	}
}

/// Parse a key description: "k", "K", "ctrl-j", "alt-x", "enter", "left",
/// "f5", ...
#[must_use]
pub fn parse_key(desc: &str) -> Option<Key> {
	let desc = desc.trim();
	if desc.is_empty() {
		return None;
	}
	let (modifiers, rest) = if let Some(r) = desc.strip_prefix("ctrl-") {
		(KeyModifiers::CONTROL, r)
	} else if let Some(r) = desc.strip_prefix("C-") {
		(KeyModifiers::CONTROL, r)
	} else if let Some(r) = desc.strip_prefix("alt-") {
		(KeyModifiers::ALT, r)
	} else if let Some(r) = desc.strip_prefix("M-") {
		(KeyModifiers::ALT, r)
	} else {
		(KeyModifiers::NONE, desc)
	};
	let code = match rest {
		"space" => KeyCode::Char(' '),
		"enter" | "return" => KeyCode::Enter,
		"esc" | "escape" => KeyCode::Esc,
		"tab" => KeyCode::Tab,
		"backspace" => KeyCode::Backspace,
		"left" => KeyCode::Left,
		"right" => KeyCode::Right,
		"up" => KeyCode::Up,
		"down" => KeyCode::Down,
		"home" => KeyCode::Home,
		"end" => KeyCode::End,
		"pageup" => KeyCode::PageUp,
		"pagedown" => KeyCode::PageDown,
		"delete" => KeyCode::Delete,
		"insert" => KeyCode::Insert,
		other => {
			if let Some(num) = other.strip_prefix('f')
				&& let Ok(n) = num.parse::<u8>()
				&& (1..=12).contains(&n)
			{
				KeyCode::F(n)
			} else if let Some(c) = other.chars().next()
				&& other.chars().count() == 1
			{
				KeyCode::Char(c)
			} else {
				return None;
			}
		}
	};
	Some(Key(code, modifiers))
}

/// Short label for a key, used in the hint bar.
#[must_use]
pub fn key_label(key: Key) -> String {
	let mods = key.1;
	let base = match key.0 {
		KeyCode::Char(' ') => "space".to_owned(),
		KeyCode::Char(c) => c.to_string(),
		KeyCode::Enter => "⏎".to_owned(),
		KeyCode::Esc => "esc".to_owned(),
		KeyCode::Tab => "tab".to_owned(),
		KeyCode::Backspace => "BS".to_owned(),
		KeyCode::Left => "←".to_owned(),
		KeyCode::Right => "→".to_owned(),
		KeyCode::Up => "↑".to_owned(),
		KeyCode::Down => "↓".to_owned(),
		KeyCode::Home => "home".to_owned(),
		KeyCode::End => "end".to_owned(),
		KeyCode::PageUp => "PgUp".to_owned(),
		KeyCode::PageDown => "PgDn".to_owned(),
		KeyCode::Delete => "del".to_owned(),
		KeyCode::Insert => "ins".to_owned(),
		KeyCode::F(n) => format!("F{n}"),
		KeyCode::Null => "null".to_owned(),
		_ => "?".to_owned(),
	};
	if mods.contains(KeyModifiers::CONTROL) {
		format!("C-{base}")
	} else if mods.contains(KeyModifiers::ALT) {
		format!("M-{base}")
	} else {
		base
	}
}

/// Does a key event match a binding? Shift is ignored (it is encoded in the
/// character itself), ctrl/alt must agree.
fn key_matches(event: KeyEvent, key: Key) -> bool {
	let significant = KeyModifiers::CONTROL | KeyModifiers::ALT;
	if event.modifiers.intersects(significant) != key.1.intersects(significant) {
		return false;
	}
	event.code == key.0
}

/// Colors used by the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
	pub border: Color,
	pub current: Color,
	pub selected_fg: Color,
	pub selected_bg: Color,
	pub status_fg: Color,
	pub status_bg: Color,
	pub hint: Color,
	pub help_bg: Color,
	pub help_fg: Color,
	pub help_key: Color,
}

impl Default for Theme {
	fn default() -> Self {
		fn color(name: &str) -> Color {
			parse_color(name).expect("valid default color")
		}
		Self {
			border: color("blue"),
			current: color("yellow"),
			selected_fg: color("black"),
			selected_bg: color("blue"),
			status_fg: color("black"),
			status_bg: color("blue"),
			hint: color("dark_gray"),
			help_bg: color("black"),
			help_fg: color("gray"),
			help_key: color("yellow"),
		}
	}
}

impl Theme {
	/// Theme from the defaults, overridden by the toml `[colors]` section.
	pub fn from_config(config: &ColorsToml) -> Self {
		fn resolve(desc: &str, value: Option<&str>, default: Color) -> Color {
			match value.map(parse_color) {
				Some(Some(color)) => color,
				Some(None) => {
					eprintln!("mpvctl: ignoring invalid color for {desc}");
					default
				}
				None => default,
			}
		}
		let d = Self::default();
		Self {
			border: resolve("border", config.border.as_deref(), d.border),
			current: resolve("current", config.current.as_deref(), d.current),
			selected_fg: resolve("selected_fg", config.selected_fg.as_deref(), d.selected_fg),
			selected_bg: resolve("selected_bg", config.selected_bg.as_deref(), d.selected_bg),
			status_fg: resolve("status_fg", config.status_fg.as_deref(), d.status_fg),
			status_bg: resolve("status_bg", config.status_bg.as_deref(), d.status_bg),
			hint: resolve("hint", config.hint.as_deref(), d.hint),
			help_bg: resolve("help_bg", config.help_bg.as_deref(), d.help_bg),
			help_fg: resolve("help_fg", config.help_fg.as_deref(), d.help_fg),
			help_key: resolve("help_key", config.help_key.as_deref(), d.help_key),
		}
	}
}

/// Parse a color name (ratatui color names, case insensitive).
#[must_use]
pub fn parse_color(name: &str) -> Option<Color> {
	let name = name.trim().to_ascii_lowercase();
	Some(match name.as_str() {
		"reset" | "none" | "default" => Color::Reset,
		"black" => Color::Black,
		"red" => Color::Red,
		"green" => Color::Green,
		"yellow" => Color::Yellow,
		"blue" => Color::Blue,
		"magenta" => Color::Magenta,
		"cyan" => Color::Cyan,
		"gray" | "grey" => Color::Gray,
		"dark_gray" | "dark_grey" => Color::DarkGray,
		"light_red" => Color::LightRed,
		"light_green" => Color::LightGreen,
		"light_yellow" => Color::LightYellow,
		"light_blue" => Color::LightBlue,
		"light_magenta" => Color::LightMagenta,
		"light_cyan" => Color::LightCyan,
		"white" => Color::White,
		_ => return None,
	})
}

/// User configuration loaded from config.toml; everything is optional.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
	pub playlists_dir: Option<String>,
	pub keys: KeysToml,
	pub colors: ColorsToml,
}

/// The `[keys]` section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct KeysToml {
	pub up: Option<String>,
	pub down: Option<String>,
	pub top: Option<String>,
	pub bottom: Option<String>,
	pub jump: Option<String>,
	pub toggle: Option<String>,
	pub play: Option<String>,
	pub pause: Option<String>,
	pub prev: Option<String>,
	pub next: Option<String>,
	pub seek_back: Option<String>,
	pub seek_fwd: Option<String>,
	pub vol_up: Option<String>,
	pub vol_down: Option<String>,
	pub delete: Option<String>,
	pub move_up: Option<String>,
	pub move_down: Option<String>,
	pub shuffle: Option<String>,
	pub clear: Option<String>,
	pub log: Option<String>,
	pub visual: Option<String>,
	pub browser: Option<String>,
	pub mark: Option<String>,
	pub append: Option<String>,
	pub overwrite: Option<String>,
	pub refresh: Option<String>,
	pub command: Option<String>,
	pub help: Option<String>,
	pub quit: Option<String>,
}

/// The `[colors]` section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ColorsToml {
	pub border: Option<String>,
	pub current: Option<String>,
	pub selected_fg: Option<String>,
	pub selected_bg: Option<String>,
	pub status_fg: Option<String>,
	pub status_bg: Option<String>,
	pub hint: Option<String>,
	pub help_bg: Option<String>,
	pub help_fg: Option<String>,
	pub help_key: Option<String>,
}

/// Path of the config file (`$XDG_CONFIG_HOME/mpvctl/config.toml`).
#[must_use]
pub fn config_path() -> PathBuf {
	let dir = env::var_os("XDG_CONFIG_HOME")
		.filter(|v| !v.is_empty())
		.map_or_else(|| crate::home().join(".config"), PathBuf::from);
	dir.join("mpvctl/config.toml")
}

/// Load the config file, warning and falling back to defaults when it is
/// missing or invalid.
#[must_use]
pub fn load() -> Config {
	let path = config_path();
	let Ok(text) = fs::read_to_string(&path) else {
		return Config::default();
	};
	match toml::from_str(&text) {
		Ok(config) => config,
		Err(e) => {
			eprintln!("mpvctl: ignoring invalid config {}: {e}", path.display());
			Config::default()
		}
	}
}

/// Expand a leading `~` to the given home directory.
#[must_use]
fn expand_tilde_with(home: &std::path::Path, value: &str) -> PathBuf {
	let mut path = home.to_path_buf();
	if let Some(rest) = value.strip_prefix('~') {
		path.push(rest.trim_start_matches('/'));
		path
	} else {
		PathBuf::from(value)
	}
}

/// Resolve the playlists directory: the `MPVCTL_PLAYLIST_DIR` environment
/// variable wins, then the config value, then the default.
#[must_use]
pub fn playlists_dir_from(config: Option<&str>, env_value: Option<&str>) -> PathBuf {
	if let Some(dir) = env_value.filter(|v| !v.is_empty()) {
		return PathBuf::from(dir);
	}
	match config.filter(|v| !v.trim().is_empty()) {
		Some(dir) => expand_tilde_with(&crate::home(), dir),
		None => crate::home().join(".local/share/mpvctl/playlists"),
	}
}

/// Resolve the playlists directory for a loaded config.
#[must_use]
pub fn playlists_dir(config: &Config) -> PathBuf {
	playlists_dir_from(
		config.playlists_dir.as_deref(),
		env::var("MPVCTL_PLAYLIST_DIR").ok().as_deref(),
	)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
		KeyEvent::new(code, modifiers)
	}

	#[test]
	fn parse_keys() {
		assert_eq!(parse_key("k"), Some(Key(KeyCode::Char('k'), KeyModifiers::NONE)));
		assert_eq!(parse_key("K"), Some(Key(KeyCode::Char('K'), KeyModifiers::NONE)));
		assert_eq!(
			parse_key("ctrl-j"),
			Some(Key(KeyCode::Char('j'), KeyModifiers::CONTROL))
		);
		assert_eq!(parse_key("alt-left"), Some(Key(KeyCode::Left, KeyModifiers::ALT)));
		assert_eq!(parse_key("space"), Some(Key(KeyCode::Char(' '), KeyModifiers::NONE)));
		assert_eq!(parse_key("f12"), Some(Key(KeyCode::F(12), KeyModifiers::NONE)));
		assert_eq!(parse_key("f13"), None);
		assert_eq!(parse_key("xx"), None);
		assert_eq!(parse_key(""), None);
	}

	#[test]
	fn match_ignores_shift_but_respects_case() {
		let up = Key(KeyCode::Char('k'), KeyModifiers::NONE);
		let move_up = Key(KeyCode::Char('K'), KeyModifiers::NONE);
		// plain k matches "k" but not "K"
		assert!(key_matches(key_event(KeyCode::Char('k'), KeyModifiers::NONE), up));
		assert!(!key_matches(key_event(KeyCode::Char('k'), KeyModifiers::NONE), move_up));
		// shifted k arrives as 'K' with or without the shift modifier
		assert!(key_matches(key_event(KeyCode::Char('K'), KeyModifiers::SHIFT), move_up));
		assert!(key_matches(key_event(KeyCode::Char('K'), KeyModifiers::NONE), move_up));
		assert!(!key_matches(key_event(KeyCode::Char('K'), KeyModifiers::NONE), up));
		// ctrl combos must agree
		let ctrl_k = Key(KeyCode::Char('k'), KeyModifiers::CONTROL);
		assert!(key_matches(
			key_event(KeyCode::Char('k'), KeyModifiers::CONTROL),
			ctrl_k
		));
		assert!(!key_matches(key_event(KeyCode::Char('k'), KeyModifiers::NONE), ctrl_k));
	}

	#[test]
	fn defaults_resolve() {
		let bindings = Keybindings::default();
		assert_eq!(
			bindings.action_for(key_event(KeyCode::Char('j'), KeyModifiers::NONE)),
			Some(Action::Down)
		);
		assert_eq!(
			bindings.action_for(key_event(KeyCode::Char('J'), KeyModifiers::SHIFT)),
			Some(Action::MoveDown)
		);
		assert_eq!(
			bindings.action_for(key_event(KeyCode::Enter, KeyModifiers::NONE)),
			Some(Action::Jump)
		);
		assert_eq!(
			bindings.action_for(key_event(KeyCode::Char('x'), KeyModifiers::NONE)),
			None
		);
	}

	#[test]
	fn toml_overrides() {
		let config: Config = toml::from_str(
			r#"
			playlists_dir = "~/music/playlists"

			[keys]
			up = "up"
			quit = "Q"

			[colors]
			status_bg = "magenta"
			"#,
		)
		.unwrap();
		assert_eq!(config.playlists_dir.as_deref(), Some("~/music/playlists"));

		let bindings = Keybindings::from_config(&config.keys);
		assert_eq!(bindings.up, parse_key("up").unwrap());
		assert_eq!(bindings.quit, parse_key("Q").unwrap());
		// unset keys keep their defaults
		assert_eq!(bindings.down, parse_key("j").unwrap());

		let theme = Theme::from_config(&config.colors);
		assert_eq!(theme.status_bg, Color::Magenta);
		assert_eq!(theme.border, Color::Blue);
	}

	#[test]
	fn toml_invalid_values_fall_back() {
		let config: Config = toml::from_str(
			r#"
			[keys]
			up = "not a key"
			[colors]
			border = "no such color"
			"#,
		)
		.unwrap();
		assert_eq!(Keybindings::from_config(&config.keys).up, parse_key("k").unwrap());
		assert_eq!(Theme::from_config(&config.colors).border, Color::Blue);
	}

	#[test]
	fn toml_broken_file_is_rejected() {
		assert!(toml::from_str::<Config>("[keys\n").is_err());
	}

	#[test]
	fn playlists_dir_precedence() {
		let home = std::path::Path::new("/home/test");
		// env wins
		assert_eq!(
			playlists_dir_from(Some("~/playlists"), Some("/env/dir")),
			PathBuf::from("/env/dir")
		);
		// then config, with ~ expanded
		assert_eq!(
			expand_tilde_with(home, "~/playlists"),
			PathBuf::from("/home/test/playlists")
		);
		assert_eq!(expand_tilde_with(home, "/abs"), PathBuf::from("/abs"));
		// then the default
		assert_eq!(
			playlists_dir_from(None, None),
			crate::home().join(".local/share/mpvctl/playlists")
		);
	}

	#[test]
	fn parse_colors() {
		assert_eq!(parse_color("blue"), Some(Color::Blue));
		assert_eq!(parse_color("Light_Blue"), Some(Color::LightBlue));
		assert_eq!(parse_color("grey"), Some(Color::Gray));
		assert_eq!(parse_color("bogus"), None);
	}
}
