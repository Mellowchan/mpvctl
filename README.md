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
  m | move [i] [j]	- move item i in front of item j
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
  h / l        previous / next track
  ← / →        seek 5s back / forward
  d            delete the selected entry
  K / J        move the selected entry up / down
  S            shuffle the playlist
  c            clear the playlist
  :            command prompt
  q (or ^C)    quit
```

The `:` prompt accepts the commands `seek <s>`, `time <s>`, `jump <i>`,
`del <i> [j]`, `move <i> <j>`, `load <name>`, `save <file>`, `prop <name>`,
`cmd <name> [args]`, `restart` and `q`, covering everything the command line
interface can do (except adding files). Messages and errors show up in the
status bar; a dead mpv server is detected automatically and reconnected once
it is back.
