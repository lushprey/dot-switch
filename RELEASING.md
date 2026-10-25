# Releasing dot-switch

This document describes the **end-to-end procedure** for cutting a new release
of `dot-switch` across all distribution channels (crates.io, GitHub Releases,
AUR, Homebrew tap, Nix flake, install script).

The whole pipeline is automated. Once you've done the one-time setup below, a
release is essentially:

```bash
# 1. Bump version + write changelog entry
$EDITOR Cargo.toml                       # bump `version = "x.y.z"`
$EDITOR CHANGELOG.md                      # add entry under [x.y.z]

# 2. Commit & tag
git commit -am "release: vx.y.z"
git tag vx.y.z
git push --tags

# 3. Watch CI publish to crates.io / AUR / Homebrew tap
#    (cargo-dist has already shipped binaries to the GitHub Release on tag push)
```

The remainder of this document is the **setup** that makes the above work, plus
troubleshooting.

---

## 1. Before the first release — one-time setup

### 1.1 Replace every `lushprey` placeholder

Throughout this repo the string `lushprey` is used as a placeholder for your
GitHub username. Replace it everywhere before committing:

```bash
git grep -l "lushprey" | xargs sed -i 's/lushprey/YOUR_REAL_GH_USERNAME/g'
```

Files that contain the placeholder:

- `Cargo.toml` — `homepage`, `repository`, `[workspace.metadata.dist]` fields
- `README.md` — installation commands, links
- `CHANGELOG.md` — compare links
- `flake.nix` — `homepage`, maintainer `github` field
- `packaging/aur/PKGBUILD` — `url`, `source_x86_64` URL
- `packaging/aur/SRCINFO` — same
- `packaging/aur/README.md` — clone URLs
- `.github/workflows/release.yml` — `ci-repository`, `github-organization`
- `.github/workflows/publish.yml` — `lushprey/homebrew-tap` repo path

### 1.2 Add GitHub Actions secrets

Repository → **Settings → Secrets and variables → Actions → New repository secret**:

| Name | Purpose | How to obtain |
|---|---|---|
| `CARGO_REGISTRY_TOKEN` | Publish to crates.io | <https://crates.io/settings/api-tokens> → "Create token" with scope **publish-new** (or **publish-update** after the first release). |
| `AUR_SSH_PRIVATE_KEY` | Push to AUR | The private half of the SSH keypair you added to your AUR account (see `packaging/aur/README.md`). |
| `HOMEBREW_TAP_TOKEN`  | Push to `lushprey/homebrew-tap` | A Personal Access Token with `repo` scope on GitHub (different from the auto-injected `GITHUB_TOKEN` because the latter cannot cross repo boundaries). |

### 1.3 Create the auxiliary repos

The release pipeline expects **two** sibling repositories to exist on your
GitHub account:

1. **`lushprey/dot-switch`** — this repo (the source).
2. **`lushprey/homebrew-tap`** — empty repo for Homebrew formulae. The CI will
   write `Formula/dot-switch.rb` here on each release. Create it with a
   trivial `README.md` so it's not totally empty.

### 1.4 Register the AUR package name

Follow the steps in [`packaging/aur/README.md`](packaging/aur/README.md) to
register `dot-switch-bin` on AUR with an initial placeholder `PKGBUILD`. This
reserves the package name so no one else can squat it.

### 1.5 First crates.io publish (manual)

The very first crates.io publish must be done by hand so that the
`publish-new` token permission is exercised and the crate is "claimed":

```bash
cargo login     # paste your API token, stored in ~/.cargo/credentials.toml
cargo publish --dry-run
cargo publish
```

After the first publish, the `publish-crates` job in
`publish.yml` will handle subsequent releases automatically.

### 1.6 Verify the cargo-dist plan

Before tagging, run a local plan to catch configuration errors:

```bash
cargo install cargo-dist@0.25.1 --locked
cargo dist plan
```

You should see all the targets from `Cargo.toml → [workspace.metadata.dist]`
listed, plus the planned installers (`shell`, `homebrew`, `msi`, `npm`).

---

## 2. Cutting a release

### 2.1 Bump version

Edit `Cargo.toml`:

```toml
[package]
name = "dot-switch"
version = "0.1.1"   # ← bump here, semver
```

Also regenerate `Cargo.lock` so it tracks the new version:

```bash
cargo update -p dot-switch --precise 0.1.1
```

### 2.2 Update changelog

Add a new entry to `CHANGELOG.md`:

```markdown
## [0.1.1] - 2025-XX-YY

### Added
- New `--foo` flag for `use` subcommand.

### Fixed
- Crash when profile dir is empty.
```

And update the comparison links at the bottom:

```markdown
[Unreleased]: https://github.com/lushprey/dot-switch/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/lushprey/dot-switch/releases/tag/v0.1.1
[0.1.0]: https://github.com/lushprey/dot-switch/releases/tag/v0.1.0
```

### 2.3 Commit, tag, push

```bash
git switch -c release/v0.1.1
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: v0.1.1"
git push -u origin release/v0.1.1

# Merge the PR (or fast-forward) and tag main:
git checkout main
git merge --ff-only release/v0.1.1
git tag v0.1.1
git push origin main
git push origin v0.1.1
```

### 2.4 Watch CI

Pushing the `v*` tag triggers **two** workflows in parallel:

1. **`release.yml`** — runs `cargo dist host`, builds all targets, uploads
   binaries + checksums + `install.sh` to a new GitHub Release (draft →
   published).
2. **`publish.yml`** — fires when the Release goes from draft → published.
   Runs the crates.io publish first, then (in parallel) the AUR and Homebrew
   tap updates.

You can watch progress under the **Actions** tab. Typical durations:

- `release.yml` ~6–10 min (musl builds are CPU-bound).
- `publish.yml` `publish-crates` job ~2 min.
- `publish.yml` `publish-aur` job ~1 min.
- `publish.yml` `publish-homebrew` job ~1 min.

### 2.5 Verify

After all jobs finish, sanity-check each channel:

```bash
# crates.io
cargo search dot-switch         # should show 0.1.1

# GitHub Releases
curl -fsSL https://github.com/lushprey/dot-switch/releases/latest \
  | grep -o "dot-switch-v0.1.1-.*musl.tar.gz"

# AUR
curl -fsSL "https://aur.archlinux.org/rpc/?v=5&type=info&arg=dot-switch-bin" \
  | jq '.results[0].Version'
# → "0.1.1-1"

# Homebrew
brew update && brew info dot-switch     # Version: 0.1.1

# Nix (cached after a few minutes of GitHub pulling)
nix run github:lushprey/dot-switch -- --version
# → dot-switch 0.1.1
```

---

## 3. Nix flake — no per-release action required

The `flake.nix` builds from the source at whatever git rev the user pins. After
a release, users automatically get the new version when they:

```bash
nix run github:lushprey/dot-switch/v0.1.1 -- --version
# or, after the latest commit is updated:
nix run github:lushprey/dot-switch -- --version
```

The Nix userland will automatically re-resolve to the tagged ref. **No publish
step is required** — the git tag is the publish.

If you want to ship a binary-cached version to `cache.nixos.org`, you can add
the cachix action — out of scope for the initial release.

---

## 4. Rollback / re-publish

### 4.1 Re-publish a broken release

If a release is broken (e.g. AUR `PKGBUILD` has a typo), you can re-run the
`publish.yml` workflow manually:

**GitHub → Actions → publish → Run workflow → enter tag `v0.1.1` → Run.**

It will skip crates.io (because the version already exists there) but refresh
AUR + Homebrew.

### 4.2 Yank a broken crates.io version

```bash
cargo yank --version 0.1.1
```

This **does not** delete the version — it just hides it from the index, so new
`cargo install` attempts fail. Already-compiled versions in users' caches are
unaffected. Yanks are permanent for the version number.

### 4.3 AUR: roll back a release

```bash
git clone ssh://aur@aur.archlinux.org/dot-switch-bin.git
cd dot-switch-bin
git log
# find the previous commit
git checkout <previous-commit-sha> -- PKGBUILD .SRCINFO
git commit -m "rollback: revert to v0.1.0"
git push
```

---

## 5. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `release.yml` builds 0 targets | `[workspace.metadata.dist]` not found | Make sure your `Cargo.toml` has both `[workspace] members = ["."]` and `[workspace.metadata.dist]` blocks. |
| `cargo binstall` 404s | `[package.metadata.binstall].pkg-url` shape doesn't match cargo-dist output | Confirm the actual tarball name in the GitHub Release and align the `pkg-url` template. |
| AUR push fails with "permission denied" | `AUR_SSH_PRIVATE_KEY` secret is wrong or the public key isn't on your AUR account | Re-do step 1.2 of `packaging/aur/README.md`. |
| crates.io publish fails "already exists" | The version was already published; `publish.yml` re-ran | Use the "Run workflow" input with a higher version, or skip this job. |
| Homebrew formula push fails | `HOMEBREW_TAP_TOKEN` PAT expired | Re-issue the PAT and update the secret. |
| `cargo dist plan` complains about missing `targets` | You're using an older cargo-dist | `cargo install cargo-dist@0.25.1 --locked` |

---

## 6. Versioning policy

`dot-switch` follows **Semantic Versioning 2.0.0**:

- **MAJOR (0.x → 1.0)**: breaking changes to the CLI flags or the
  `dot-switch.toml` manifest schema. Will be accompanied by a profile
  migration tool if non-trivial.
- **MINOR (0.x → 0.(x+1))**: new subcommands, new `config.toml` keys. No
  breaking changes to existing profiles.
- **PATCH (0.x.y → 0.x.(y+1))**: bug fixes, dependency updates that don't
  change observable behavior.

While the major version is still `0`, expect **more frequent breaking changes**
— treat every minor bump as potentially breaking until `1.0` lands.
