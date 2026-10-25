# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-01-01

### Added
- Initial public release.
- `init` — bootstrap `~/.config/dot-switch/` with default `config.toml`.
- `use <profile>` / `switch <profile>` — swap symlinks declared in profile manifest.
- `apply` — apply every profile in the repo to canonical state.
- `list` / `ls` — enumerate known profiles.
- `diff <profile>` — preview changes without writing.
- `current` — print active profile.
- `revert` — pop history ring buffer.
- `snapshot` — manual backup of current live files.
- `new <name> --from-live <paths...>` — capture live `~/.config` files as a profile.
- `config` — open global config in `$EDITOR`.
- `paths` — print XDG paths the tool uses.
- `completions <shell>` — bash/zsh/fish completion scripts (via `clap_complete`).
- Interactive TUI: fuzzy-searchable profile list with live `diff` preview pane.
- Built-in presets (disabled by default) for Hyprland and Quickshell reload.
- Path-escape validation: manifest `source`/`target` cannot use `..` or absolute paths.
- Idempotent switches: re-applying an already-active profile is a no-op.
- Optional `--backup` snapshots before each switch.
- 20-entry history ring buffer in `state.json`.

[Unreleased]: https://github.com/lushprey/dot-switch/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lushprey/dot-switch/releases/tag/v0.1.0
