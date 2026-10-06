# Tue Oct  6 23:30:00 2026 +0200

1. The count prefix now also works with g/G: 5G jumps to line 5 (clamped to
   the list), plain G still jumps to the last entry.
2. Added vim-like search to the TUI: / opens a search buffer in the bottom
   bar, return jumps to the next track whose name matches the keyword
   (case-insensitive, wildcards * and ?), n/N cycle forward/backward through
   the matches (wrapping around), an empty pattern repeats the last search
   and esc cancels the buffer. Keys are rebindable (search, search_next,
   search_prev). Verified in a tmux-run TUI against an isolated mpv daemon.

# Tue Oct  6 23:00:00 2026 +0200

1. Made the bottom status bar and the selected list item text bold by default.
2. Added a vi-like count prefix to the TUI navigation: digits typed before a
   motion key repeat it (5j selects 5 tracks down, 3k 3 tracks up, v5j makes
   a visual selection spanning 5 tracks down); the pending count shows in the
   status bar and esc cancels it. Verified in a tmux-run TUI against an
   isolated mpv daemon (count jumps, visual ranges and range deletes).

# Wed Sep 23 01:12:01 2026 +0200

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
5. Added a keymap window to the TUI, toggled by `?` (configurable via the
   `help` key binding, closed with `?`, `q` or `esc`), replacing the key-hint
   bar at the bottom; the status bar keeps a small `[? help]` hint and shows
   the actual configured help key.
6. Added dedicated `play` (default `p`) and `pause` (default `s`) key binds
   to the TUI next to the existing space toggle, mirroring the CLI command
   mnemonics; configurable like every other binding via `[keys]`.
7. Added a daemon log view to the TUI (default key `L`): it subscribes to
   mpv's `request_log_messages` (info level) over the observer connection
   and streams `log-message` events into a 2000 line buffer, following the
   tail until the user scrolls (j/k/g/G); `L` or esc closes and unsubscribes
   (re-subscribing automatically after a daemon restart). Also reclaimed
   the row left empty by the removed hint bar for the main view.
8. Extended CLI `move` with ranges: `mpvctl m 10-15 2` moves entries 10 to
   15 (order preserved) in front of entry 2. Implemented as a pure command
   builder (greedy single-entry `playlist-move` sequence derived from the
   target order, unit tested against mpv's move semantics) shared by single
   and range moves; invalid ranges, out-of-bounds and in-range targets are
   rejected with clear errors.
9. Added vim-like visual selection to the TUI: `v` starts it at the
   cursor, j/k/g/G extend the range (shown with the selection background
   and a VISUAL flag in the status bar), then `d` deletes the whole range,
   `J`/`K` move the block (reusing the range-move primitive from the CLI
   move task, so single-entry moves gained correct end-of-list behavior),
   and `v`/esc cancel. The anchor is clamped when the playlist changes.
10. Added a playlists browser to the TUI (`b`): lists the files from the
    configured playlists directory (title shows the path, `r` reloads),
    `m` marks/unmarks entries vim-style, and `a`/enter appends the marked
    (or cursor) playlists to the main playlist via mpv's `loadlist` while
    `o` overwrites it (first replace, rest appended); the m3u mirror is
    rewritten from the read-back so it always matches. b/esc closes.
11. Optimized the release binary size in Cargo.toml: `[profile.release]`
    with opt-level "z", fat LTO, one codegen unit, panic=abort and stripped
    symbols, plus trimmed dependency default-features (ratatui without the
    calendar widget/macros, toml parse-only). 1.82 MB -> 0.90 MB (-51%),
    verified with the CLI regression suite and a TUI smoke test.
12. Added optional toml config at $XDG_CONFIG_HOME/mpvctl/config.toml: a
    `playlists_dir` key (~ expanded; MPVCTL_PLAYLIST_DIR still overrides), a
    `[keys]` section rebinding every TUI action (crossterm key names like
    "ctrl-j", "enter", "f5") and a `[colors]` section for border, current
    track, selection, status bar and hint colors. Missing/broken files and
    invalid values warn on stderr and fall back to the vi-like defaults;
    unit tests cover parsing, precedence and tilde expansion, and the pty
    harness verified rebinding, colors and dir resolution end to end.
13. Styled the TUI keymap popup: dark background with a colored border and
    bottom close hint, and the listed keys are now highlighted (bold
    yellow by default) next to dim gray descriptions. New `[colors]`
    options `help_bg`/`help_fg`/`help_key` control it.
14. Added volume control: CLI `v | vol [x]` (get, absolute set, or `+10`/`-10`
    relative like seek) and TUI `+`/`-` keys (±5, work in every view) plus a
    `:vol [x]` command. The `volume` property is now observed, so the status
    bar shows the live volume.
15. Fixed the empty TUI log view. Root cause: mpv's request_log_messages
    only delivers messages emitted after the subscription, so with a paused
    or idle daemon nothing new arrived and the view started empty, while
    `mpvctl log` dumps the daemon log file that has the full history. The
    observer now subscribes for the whole TUI session (so the buffer fills
    even while the view is closed, and survives reconnects), and opening the
    view seeds an empty buffer with the log file tail (A:/V: playback status
    lines filtered out), i.e. exactly what `mpvctl log` shows, before the
    live info-level events take over.

16. Added a `vol_reset` TUI binding (default `=`) that sets the volume back
    to 100%; like `+`/`-` it works in every view and is listed in the keymap
    window and the README config example.
17. The TUI status bar volume now shows a percent sign after the value
    (`vol 42%` instead of `vol 42`).

18. The daemon volume now shows in both status places: the TUI status bar
    (live via the observed volume property, added with the volume task) and
    the `mpvctl ls` bottom status line, which gained a `volume: N%` segment
    (fetched in the same batched round trip; `?` if unavailable). Verified
    against a live daemon at several volumes and with the CLI regression
    suite (identical to the shell tool apart from the new segment).

19. Shortened the `mpvctl ls` status line volume segment from `volume:` to
    `vol:` (`status: paused, time: 0:0:0/0:0:2, vol: 55%, file: track1`),
    matching the TUI status bar wording. CLI regression suite still
    identical apart from the volume segment.
