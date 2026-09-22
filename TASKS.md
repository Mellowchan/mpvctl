# TODO

4. Add reatime TUI (ncurses like) interface with vi-like keybindings that should do what the command line arguments can do (exception is adding files to the playlist), and status bar on the bottom. The TUI should be invoked just by starting `./mpvctl` without any argument.
5. Add toml config to configure keybindings, TUI colors, and playlists file directory.

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
