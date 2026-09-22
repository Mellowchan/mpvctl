# TODO

1. Rewrite mpvctl in rust and replace the shell tools it uses (e.g. jq, socat) with rust libraries if possible and update the REAMDE.md according to changes made. Use the `mpvctl` command on the system to compare if the reimplementation works as intended.
2. Add feature as command line argument to move tracks in the playlist.
3. Add feature of playlists directory that will list the available playlists files (tracks) in them and load the each playlists by just typing their filenames (not full path as it is now), but also keep the possiblity to load files from custom playlist outside the directory.
4. Add reatime TUI (ncurses like) interface with vi-like keybindings that should do what the command line arguments can do (exception is adding files to the playlist), and status bar on the bottom. The TUI should be invoked just by starting `./mpvctl` without any argument.
5. Add toml config to configure keybindings, TUI colors, and playlists file directory.

# DONE
