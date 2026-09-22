# mpvctl

Terminal tool written in rust to control a long running mpv music daemon from
the command line. It talks to mpv over its JSON IPC unix socket, so it has no
runtime dependencies besides `mpv` itself (no jq, socat, pgrep, tput ...).

## Installation

```
make install
```

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
  t | time [i]		- jump to time in seconds
  O | prop [...]	- get property
  C | cmd [...]		- send custom command
  a | add [...]		- add parameters to playlist
  d | del [i] [i]	- delete item or range
  save [file]		- save current playlist to file
  load [file]		- load playlist from file
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
