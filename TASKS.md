# TODO

6. Add help keymap window to TUI binded by `?` key instead of bar keymap on the bottom
7. Add keybind to TUI to pause and play (currently there's only toggle)
8. Add keybind to show log of the mpv deamon in the TUI in realtime
9. Add feature to cli to be able to move multiple files e.g. `mpvctl m 10-15 2` would move items in range 10 to 15 in front of item 2
10. Add feature to TUI to select multiple items in the current playlist and to do operations like move or delete with multiple files
11. Add feature to TUI to show playlists from playlists directory and select one or more to load track into the main playlist, add different keybinds to append or overwite the main playlist
12. Add optimizations to Cargo.toml for the size of mpvctl binary release

# DONE

1. Rewrote mpvctl in rust (serde_json for JSON IPC over std unix sockets, libc
   for signals, /proc scan for pid detection) replacing jq/socat/pgrep/tput;
   verified against the system shell mpvctl with an isolated mpv daemon
   (byte identical output for all commands); updated README.md and Makefile,
   removed the old shell script.
2. Added `m | move [i] [j]` command using mpv's `playlist-move`, keeping the
   playlist file in sync afterwards (same as del/shuffle); verified the move
   semantics (forward, backward, no-op) against the live mpv playlist.
3. Added a playlists directory (default ~/.local/share/mpvctl/playlists,
   overridable via MPVCTL_PLAYLIST_DIR) with a `pl | playlists` command that
   lists its playlist files; `load [name]` now resolves names in the
   directory with an optional .m3u extension, while names with a '/' or not
   found there are still treated as paths so custom playlists outside the
   directory keep working.
4. Added a realtime TUI (ratatui + crossterm) invoked by `mpvctl` without
   arguments in a terminal (non-tty keeps printing the playlist). It observes
   playlist/pause/time/duration properties over a persistent IPC connection,
   has vi-like keybindings (k/j/g/G, enter, space, h/l, arrows seek, d, K/J,
   S, c, q), a ':' command prompt with seek/time/jump/del/move/load/save/prop/
   cmd/restart, a status bar plus key-hint bar, and auto-reconnects when the
   server dies. Verified in a pty harness against an isolated --ao=null mpv:
   rendering, all keys, command mode, file syncing and daemon recovery.
6. Added a keymap window to the TUI, toggled by `?` (configurable via the
   `help` key binding, closed with `?`, `q` or `esc`), replacing the key-hint
   bar at the bottom; the status bar keeps a small `[? help]` hint and shows
   the actual configured help key.
7. Added dedicated `play` (default `p`) and `pause` (default `s`) key binds
   to the TUI next to the existing space toggle, mirroring the CLI command
   mnemonics; configurable like every other binding via `[keys]`.
5. Added optional toml config at $XDG_CONFIG_HOME/mpvctl/config.toml: a
   `playlists_dir` key (~ expanded; MPVCTL_PLAYLIST_DIR still overrides), a
   `[keys]` section rebinding every TUI action (crossterm key names like
   "ctrl-j", "enter", "f5") and a `[colors]` section for border, current
   track, selection, status bar and hint colors. Missing/broken files and
   invalid values warn on stderr and fall back to the vi-like defaults;
   unit tests cover parsing, precedence and tilde expansion, and the pty
   harness verified rebinding, colors and dir resolution end to end.
