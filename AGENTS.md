# Agents Guidelines

mpvctl is a simple terminal tool to control over socket long running mpv session, that is mainly used for music playback.

## Repository Layout

| Directory | Purpose          |
| --------- | ---------------- |
| `src/`    | rust code folder |

## Toolchain

- **Rust stable** from `cargo -V`.
- **Edition:** 2024.
- `rustfmt.toml`
- Clippy lints are configured in the workspace Cargo.toml under [workspace.lints.clippy]. Every member crate must have [lints] workspace = true.

## Coding Guidelines

The coding guidelines are the authoritative standard for both writing and reviewing code:

- every change must be git commited with relevant description
- TASKS.md has TODO section on the top, and DONE section on the bottom
- If a task in TASKS.md is considered finished then it must be moved from TODO to DONE
- Make sure to update README.md after changes that e.g. expand features or change how the user interacts with the program or config
