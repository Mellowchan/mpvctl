# mpvctl

Terminal tool written in rust to control a long running mpv music daemon from
the command line. It talks to mpv over its JSON IPC unix socket, so it has no
runtime dependencies besides `mpv` itself (no jq, socat, pgrep, tput ...).

## Installation

```
make install
```

Release builds are optimized for size (fat LTO, `opt-level = "z"`, stripped
   symbols) in `Cargo.toml`.

## Usage

```
mpvctl <cmd> [ARGS...]

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
  v | vol [x]		- get/set volume (e.g. 50, +10, -10)
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
  find ~/music/ -type f | mpvctl

```

The daemon (started with `mpvctl start` or automatically on the first command)
is an `mpv --idle` session listening on `$XDG_RUNTIME_DIR/mpvd` and using
`~/.local/share/mpvd_playlist.m3u` as its playlist file. Commands that need a
running server start one automatically.

## Playlists directory

`load` looks names up in a playlists directory (default
`~/.local/share/mpvctl/playlists`, override with `MPVCTL_PLAYLIST_DIR`) so
playlists can be loaded by filename only:

```
mpvctl pl          # list available playlists
mpvctl load rock   # load ~/.local/share/mpvctl/playlists/rock(.m3u)
```

`move` accepts a single index or an inclusive range: `mpvctl m 10-15 2`
moves tracks 10 to 15 (order preserved) in front of track 2.

The `.m3u` extension is optional in both directions (a playlist stored as
`chill` can be loaded with `load chill.m3u` and vice versa). Names containing
a `/`, or names not found in the directory, are used as paths, so playlists
outside the directory keep working:

```
mpvctl load ~/backups/old.m3u
```

## TUI

Running `mpvctl` without arguments in a terminal opens an interactive
interface (in pipes or scripts the old behavior, printing the playlist, is
kept). It updates in realtime through observed mpv properties and has a
status bar at the bottom showing play state, elapsed/total time, position in
the playlist and the current track.

Default (vi-like) keys:

```
  k / j        select previous / next track
  g / G        select first / last track
  enter        play the selected track
  space        toggle pause
  p / s        play / pause
  h / l        previous / next track
  ← / →        seek 5s back / forward
  + / -        volume up / down by 5
  d            delete the selected entry
  v            visual selection: extend with j/k/g/G, then d (delete),
               J/K (move the whole block) or v/esc (cancel)
  K / J        move the selected entry up / down
  S            shuffle the playlist
  c            clear the playlist
  b            playlists browser: j/k move, m mark, a/enter append the
               selected/marked playlists to the main playlist, o overwrite
               the main playlist with them, r reload, b/esc close
  L            daemon log view (j/k scroll, g/G, L/esc close)
  :            command prompt
  q (or ^C)    quit
```

The `:` prompt accepts the commands `seek <s>`, `time <s>`, `jump <i>`,
`del <i> [j]`, `move <i> <j>`, `vol [x]`, `load <name>`, `save <file>`,
`prop <name>`, `cmd <name> [args]`, `restart` and `q`, covering everything the
command line interface can do (except adding files). The current volume is
shown in the status bar. Messages and errors show up in the
status bar; a dead mpv server is detected automatically and reconnected once
it is back. Press `?` for a keymap window (`q`/`esc` close it).

## Configuration

Optional toml config at `~/.config/mpvctl/config.toml` (respecting
`$XDG_CONFIG_HOME`). Everything is optional, unknown values are warned about
on stderr and fall back to the defaults:

```toml
# playlists directory (~ is expanded); MPVCTL_PLAYLIST_DIR overrides this
playlists_dir = "~/.local/share/mpvctl/playlists"

[keys]
# crossterm key names: single chars, "ctrl-j", "alt-x", "enter", "left",
# "f5", "space", ... (upper/lower case are different keys)
up = "k"          # select previous
down = "j"         # select next
top = "g"          # select first
bottom = "G"       # select last
jump = "enter"     # play selected
toggle = "space"   # toggle pause
play = "p"        # play (unpause)
pause = "s"       # pause
prev = "h"         # previous track
next = "l"         # next track
seek_back = "left" # seek back
seek_fwd = "right" # seek forward
vol_up = "+"       # volume up 5
vol_down = "-"     # volume down 5
delete = "d"       # delete selected entry
move_up = "K"      # move selected entry up
move_down = "J"    # move selected entry down
shuffle = "S"
clear = "c"
log = "L"        # open/close the daemon log view
visual = "v"      # visual (multi) selection
browser = "b"     # playlists browser
mark = "m"        # mark a playlist in the browser
append = "a"      # append marked/selected playlist(s)
overwrite = "o"   # replace the main playlist with them
refresh = "r"     # reload the browser listing
command = ":"      # open the command prompt
help = "?"        # toggle the keymap window
quit = "q"

[colors]
# ratatui color names: black, red, green, yellow, blue, magenta, cyan, gray,
# dark_gray, light_red, light_green, light_yellow, light_blue, light_magenta,
# light_cyan, white, reset
border = "blue"        # playlist border and title
current = "yellow"     # current track
selected_fg = "black"  # selected entry
selected_bg = "blue"
status_fg = "black"    # status bar
status_bg = "blue"
hint = "dark_gray"     # key hints bar
help_bg = "black"      # keymap popup background
help_fg = "gray"       # keymap popup descriptions
help_key = "yellow"    # keymap popup keys (bold)
```
