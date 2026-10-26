# dot-switch

> Swap `~/.config` files between named profiles via symlinks.
> Built for **Hyprland** and **Quickshell**, generic for everything.

`dot-switch` manages a central repository of config profiles and swaps them into `$XDG_CONFIG_HOME` using symlinks. Each profile declares which files it owns via a `dot-switch.toml` manifest. Reload behaviour is opt-in via the global config — by default, `dot-switch` only swaps files and prints what changed.

## Installation

Pick **one** channel. The official builds come from GitHub Releases and are identical across every channel.

### Homebrew tap (Linux + macOS)

```bash
brew trust lushprey/tap
brew tap lushprey/tap
brew install dot-switch
```

### Nix flake (NixOS / any Nix)

```bash
# one-off run
nix run github:lushprey/dot-switch -- init

# permanent install (flakes-enabled profile)
nix profile install github:lushprey/dot-switch
```

For `home-manager`, add to your flake inputs and consume `packages.<system>.default`:

```nix
inputs.dot-switch.url = "github:lushprey/dot-switch";

# in a home.nix somewhere:
home.packages = [ inputs.dot-switch.packages.${system}.default ];
```

### `cargo binstall` (binary install via Rust toolchain)

```bash
cargo binstall dot-switch        # downloads precompiled musl binary
```

### `cargo install` (build from source)

```bash
cargo install --locked dot-switch
```

### Direct download (curl install script)

```bash
curl --proto '=https' --tlsv1.2 -sSf \
  https://github.com/lushprey/dot-switch/releases/latest/download/install.sh \
  | bash
```

Or grab the tarball manually from the [Releases page](https://github.com/lushprey/dot-switch/releases/latest). Each release ships:

- `dot-switch-v$ver-x86_64-unknown-linux-musl.tar.gz`     (Arch / Alpine / any Linux)
- `dot-switch-v$ver-aarch64-unknown-linux-musl.tar.gz`    (Raspberry Pi 4/5, ARM SBCs)
- `dot-switch-v$ver-x86_64-apple-darwin.tar.gz`           (macOS Intel)
- `dot-switch-v$ver-aarch64-apple-darwin.tar.gz`          (macOS Apple Silicon)
- `dot-switch-v$ver-x86_64-pc-windows-msvc.zip`           (Windows)
- A bundled `install.sh` and `install.ps1` for each platform.

### Post-install: enable shell completions (optional)

If you installed via `cargo install` or `cargo binstall`, generate them once:

```bash
# Bash
dot-switch completions bash > /etc/bash_completion.d/dot-switch   # system
# or:
dot-switch completions bash >> ~/.bashrc                          # per-user

# Zsh (place file in a directory that's in your $fpath)
dot-switch completions zsh > ~/.zfunc/_dot-switch

# Fish
dot-switch completions fish > ~/.config/fish/completions/dot-switch.fish
```

AUR / Homebrew / Nix packages ship completions pre-installed to the distro-standard locations — no extra steps needed.

## Quick start

```bash
# Initialize (writes default config.toml, creates profile/state/backups dirs)
dot-switch init

# Capture your current hyprland + quickshell config as a profile called "dark"
dot-switch new dark --from-live hypr/hyprland.conf quickshell/config.qml

# Edit your live files, then capture a second profile
# (e.g. tweak hyprland.conf, then:)
dot-switch new light --from-live hypr/hyprland.conf quickshell/config.qml

# List profiles
dot-switch list

# Switch to a profile
dot-switch use dark

# Or launch the interactive TUI (fuzzy-searchable list + live diff preview)
dot-switch
```

## How it works

1. **Profile repo** — a directory containing one subdirectory per profile. Default: `~/.config/dot-switch/profiles/`.
2. **Manifest** — each profile dir contains a `dot-switch.toml` declaring which files it owns and where they should be symlinked inside `$XDG_CONFIG_HOME`.
3. **Switch** — `dot-switch use <profile>` walks the manifest, symlinks each source file into its target path (replacing any existing file/symlink), and records the active profile in `state.json`.
4. **Reload (optional)** — if a preset's `enabled = true` and the changed files match its `reload_on_files` globs, the preset's `reload_cmd` is run. Off by default — most distros auto-reload Hyprland/Quickshell on config change.

## CLI

```
dot-switch [OPTIONS] [COMMAND]

Commands:
  init         Initialize dot-switch (write default config.toml, create profiles dir)
  use          Switch to a named profile                       [aliases: switch]
  apply        Apply all profiles in the repo (resets everything to canonical state)
  list         List known profiles                             [aliases: ls]
  diff         Show what `use <profile>` would change without touching the filesystem
  current      Print the currently-active profile
  revert       Revert to the previous profile (from history)
  snapshot     Take a manual snapshot of current live files (no switch)
  new          Create a new profile from current live ~/.config files
  config       Edit the global config in $EDITOR
  paths        Print all the paths dot-switch uses (config, state, backups, profiles)
  completions  Generate shell completions
  help         Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase verbosity (-v = debug, default = warn)
  -h, --help        Print help (see more with '--help')
  -V, --version     Print version
```

Every subcommand also has its own `-h` / `--help`.

### `use` flags

| Flag | Purpose |
|---|---|
| `--only <PATH>` | Apply only a single file from the profile (matches `source` or `target`) |
| `--backup` | Snapshot current targets before switching |
| `--dry-run` | Print actions without performing them |
| `--no-reload` | Skip reload commands even if a preset has `enabled = true` |
| `--reload` | Force reload even for presets with `enabled = false` |

### No-arg invocation

Running `dot-switch` with no subcommand launches an interactive **TUI**: a fuzzy-searchable list of profiles with a live `diff` preview pane. Type to filter, `↑`/`↓` to navigate, `Enter` to switch, `q` to quit.

## Configuration — `$XDG_CONFIG_HOME/dot-switch/config.toml`

```toml
# Central profiles repo (expanded; ~ and $ENV supported)
profiles_repo = "~/.config/dot-switch/profiles"

# Days to keep auto-backups before pruning. 0 = never prune.
default_backup_keep_days = 7

# Per-app preset overrides. Hyprland & Quickshell ship as built-in defaults
# (disabled — opt in here if your distro doesn't auto-reload).
[presets.hyprland]
enabled = false
reload_cmd = "hyprctl reload"
reload_on_files = ["hypr/*.conf"]

[presets.quickshell]
enabled = false
reload_cmd = "pkill -SIGUSR1 quickshell"
reload_on_files = ["quickshell/**/*.qml"]

# Add your own presets — any name, any reload command.
[presets.waybar]
enabled = true
reload_cmd = "pkill -SIGUSR2 waybar"
reload_on_files = ["waybar/config", "waybar/style.css"]
```

## Profile manifest — `<profile_dir>/dot-switch.toml`

The user is free to organize the profile dir however they like — the manifest is the only contract.

```toml
name = "my-setup"
description = "Laptop + dark theme"

# Each entry maps a source file (inside this profile dir) to a target in $XDG_CONFIG_HOME.
[[files]]
source = "hypr/hyprland.conf"
target = "hypr/hyprland.conf"

[[files]]
source = "quickshell/config.qml"
target = "quickshell/config.qml"

[[files]]
source = "waybar/config"
target = "waybar/config"
```

`source` and `target` must be relative paths. `target` is interpreted relative to `$XDG_CONFIG_HOME`. The `new --from-live` command writes a manifest with `source == target` for each captured file.

## XDG paths

| Purpose | Path |
|---|---|
| Tool config | `$XDG_CONFIG_HOME/dot-switch/config.toml` |
| State (current profile, history) | `$XDG_STATE_HOME/dot-switch/state.json` |
| Backups (timestamped dirs) | `$XDG_STATE_HOME/dot-switch/backups/<ts>/` |
| Profiles repo (configurable) | `$XDG_CONFIG_HOME/dot-switch/profiles/` |

Run `dot-switch paths` to see exactly where everything lives on your system.

## Shell completions

```bash
# Bash (append to ~/.bashrc)
dot-switch completions bash >> ~/.bashrc

# Zsh (save to a directory in fpath)
dot-switch completions zsh > ~/.zfunc/_dot-switch

# Fish
dot-switch completions fish > ~/.config/fish/completions/dot-switch.fish
```

## Safety

- **Symlink-only** — never moves real files except when displacing an existing target during a switch (in which case it's moved into the backup dir, not deleted).
- **Idempotent** — switching to an already-active profile is a no-op; switching to a profile whose symlinks are already in place skips them.
- **Path validation** — manifest `source`/`target` paths cannot escape the profile dir or `$XDG_CONFIG_HOME` (no `..` or absolute paths).
- **`--dry-run` always available** — and the TUI runs `diff` before any write.
- **History ring buffer** — last 20 profiles are kept in `state.json`; `dot-switch revert` pops the most recent.
- **`--backup` snapshots** — before each switch, optionally snapshot current targets into `backups/<timestamp>/`.

## Why "manual reload" by default?

Most modern Hyprland/Quickshell distros watch config files and auto-reload on change. Running an extra `hyprctl reload` after every switch is wasted work and can cause double-reload glitches. So `dot-switch` ships reload presets **disabled**. If your distro doesn't auto-reload (or you're on a custom setup), flip `enabled = true` in `config.toml` and the reload command will fire when matching files change.

## Packaging & distribution

`dot-switch` is published simultaneously to multiple channels from a single
GitHub Release. The complete infrastructure lives in this repository:

| Channel | Source of truth | Maintainer docs |
|---|---|---|
| **crates.io** | `Cargo.toml` + `.github/workflows/publish.yml` | — |
| **GitHub Releases** | `Cargo.toml` `[workspace.metadata.dist]` + `.github/workflows/release.yml` | [cargo-dist docs](https://opensource.axo.dev/cargo-dist/) |
| **Homebrew tap** | `.github/workflows/publish.yml` (auto-generated Formula) | — |
| **Nix flake** | `flake.nix` | — |

See [`RELEASING.md`](RELEASING.md) for the full step-by-step release procedure.

## License

MIT — see [LICENSE](LICENSE).
