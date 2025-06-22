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
  play		- start playing
  pause		- pause current file
  toggle	- toggle current file
  next		- play next file
  prev		- play previous file
  ls		- list the playlist
  clear		- clear the playlist
  shuffle	- shuffle the playlist
  prop [...]	- get properties
  add [...]	- add parameters to playlist
  jump [i]	- jump to index in playlist
  del [i] [i]	- delete item or range

PIPE:
  find ~/music/ -type f | $PROGNAME

```
