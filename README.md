# mpvctl

Shell script to control a long running mpv from command line.

## Installation

```
make install
```

## Usage

```
mpvd start|stop|restart|status|log
```

```
mpvctl <cmd> [ARGS...]

COMMANDS:
  p | play		    - start playing
  s | pause		    - pause current file
  T | toggle	    - toggle current file
  N | next		    - play next file
  P | prev		    - play previous file
  l | ls		    - list the playlist
  c | clear		    - clear the playlist
  S | shuffle		- shuffle the playlist
  j | jump [i]		- jump to index in playlist
  e | seek [i]		- seek in seconds
  t | time [i]		- jump to time in seconds
  O | prop [...]	- get property
  C | cmd [...]		- send custom command
  a | add [...]		- add parameters to playlist
  d | del [i] [i]	- delete item or range
  L | log		    - list the mpvd logs
  R | restart		- restart mpvd

PIPE:
  find ~/music/ -type f | $PROGNAME

```
