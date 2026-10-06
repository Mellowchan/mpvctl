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

- Main roadmap of features is in TASKS.md
- If a task in TASKS.md is considered finished then it must be moved to CHANGELOG.md
- Update README.md after changes that e.g. expand features or change how the user interacts with the program or config
- every change must be git commited with relevant description
- each task from TASKS.md must be in separate git commit
- every change must be tested it's working before commited
