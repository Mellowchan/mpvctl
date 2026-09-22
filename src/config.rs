//! TUI keybindings and colors with vi-like defaults.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

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
	Prev,
	Next,
	SeekBack,
	SeekFwd,
	Delete,
	MoveUp,
	MoveDown,
	Shuffle,
	Clear,
	Command,
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
	pub prev: Key,
	pub next: Key,
	pub seek_back: Key,
	pub seek_fwd: Key,
	pub delete: Key,
	pub move_up: Key,
	pub move_down: Key,
	pub shuffle: Key,
	pub clear: Key,
	pub command: Key,
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
			prev: key("h"),
			next: key("l"),
			seek_back: key("left"),
			seek_fwd: key("right"),
			delete: key("d"),
			move_up: key("K"),
			move_down: key("J"),
			shuffle: key("S"),
			clear: key("c"),
			command: key(":"),
			quit: key("q"),
		}
	}
}

impl Keybindings {
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
			(self.prev, Action::Prev),
			(self.next, Action::Next),
			(self.seek_back, Action::SeekBack),
			(self.seek_fwd, Action::SeekFwd),
			(self.delete, Action::Delete),
			(self.move_up, Action::MoveUp),
			(self.move_down, Action::MoveDown),
			(self.shuffle, Action::Shuffle),
			(self.clear, Action::Clear),
			(self.command, Action::Command),
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
	fn parse_colors() {
		assert_eq!(parse_color("blue"), Some(Color::Blue));
		assert_eq!(parse_color("Light_Blue"), Some(Color::LightBlue));
		assert_eq!(parse_color("grey"), Some(Color::Gray));
		assert_eq!(parse_color("bogus"), None);
	}
}
