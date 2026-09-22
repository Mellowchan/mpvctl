# TODO

2. Add feature as command line argument to move tracks in the playlist.
3. Add feature of playlists directory that will list the available playlists files (tracks) in them and load the each playlists by just typing their filenames (not full path as it is now), but also keep the possiblity to load files from custom playlist outside the directory.
4. Add reatime TUI (ncurses like) interface with vi-like keybindings that should do what the command line arguments can do (exception is adding files to the playlist), and status bar on the bottom. The TUI should be invoked just by starting `./mpvctl` without any argument.
5. Add toml config to configure keybindings, TUI colors, and playlists file directory.

# DONE

1. Rewrote mpvctl in rust (serde_json for JSON IPC over std unix sockets, libc
   for signals, /proc scan for pid detection) replacing jq/socat/pgrep/tput;
   verified against the system shell mpvctl with an isolated mpv daemon
   (byte identical output for all commands); updated README.md and Makefile,
   removed the old shell script.
