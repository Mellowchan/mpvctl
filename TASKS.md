# TODO


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
5. Added optional toml config at $XDG_CONFIG_HOME/mpvctl/config.toml: a
   `playlists_dir` key (~ expanded; MPVCTL_PLAYLIST_DIR still overrides), a
   `[keys]` section rebinding every TUI action (crossterm key names like
   "ctrl-j", "enter", "f5") and a `[colors]` section for border, current
   track, selection, status bar and hint colors. Missing/broken files and
   invalid values warn on stderr and fall back to the vi-like defaults;
   unit tests cover parsing, precedence and tilde expansion, and the pty
   harness verified rebinding, colors and dir resolution end to end.
