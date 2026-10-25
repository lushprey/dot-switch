# AUR: `dot-switch-bin`

This directory contains the **PKGBUILD** and **.SRCINFO** templates used by
`.github/workflows/publish.yml` to push binary updates to the
[`dot-switch-bin`](https://aur.archlinux.org/packages/dot-switch-bin) AUR package.

## One-time setup (do this manually)

1. Create an account on <https://aur.archlinux.org/> if you don't have one.
2. Generate an SSH keypair locally and add the **public** key to your AUR account:
   ```bash
   ssh-keygen -t ed25519 -f ~/.ssh/aur_dot-switch -C "dot-switch AUR bot"
   # paste ~/.ssh/aur_dot-switch.pub into AUR → My Account → SSH Public Keys
   ```
3. Create the AUR package repo (empty repo is fine, the first push will populate it):
   ```bash
   git clone ssh://aur@aur.archlinux.org/dot-switch-bin.git
   cd dot-switch-bin
   # initial placeholder PKGBUILD is required to register the package name:
   cat > PKGBUILD <<'EOF'
   # Maintainer: Your Name <your@email.dev>
   pkgname=dot-switch-bin
   pkgver=0.0.1
   pkgrel=1
   pkgdesc="placeholder until first real release"
   arch=('any')
   url="https://github.com/lushprey/dot-switch"
   license=('MIT')
   package() { true; }
   EOF
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "initial placeholder"
   git push
   ```
4. Add the corresponding **private** key as a GitHub Actions secret named
   `AUR_SSH_PRIVATE_KEY` (Repository → Settings → Secrets and variables →
   Actions → New repository secret). Paste the full content of
   `~/.ssh/aur_dot-switch` (the file with no `.pub` extension).

## How it works from there

On every GitHub **release published** event (i.e. when you push a `v*` tag and
the `release.yml` workflow completes successfully), `publish.yml` will:

1. Download the musl tarballs from the GitHub Release.
2. Compute their SHA-256 sums.
3. Substitute `%%VERSION%%`, `%%SHA_X86_64%%`, `%%SHA_AARCH64%%` in `PKGBUILD`
   and `SRCINFO`.
4. Clone `ssh://aur@aur.archlinux.org/dot-switch-bin.git` and push the
   rendered files.

## Manual fallback

If the CI ever breaks, you can push by hand:

```bash
# from this directory
V=0.1.0
X64=$(curl -fsSL "https://github.com/lushprey/dot-switch/releases/download/v${V}/dot-switch-v${V}-x86_64-unknown-linux-musl.tar.gz" | sha256sum | awk '{print $1}')
ARM=$(curl -fsSL "https://github.com/lushprey/dot-switch/releases/download/v${V}/dot-switch-v${V}-aarch64-unknown-linux-musl.tar.gz" | sha256sum | awk '{print $1}')
sed -e "s|%%VERSION%%|${V}|g" -e "s|%%SHA_X86_64%%|${X64}|g" -e "s|%%SHA_AARCH64%%|${ARM}|g" PKGBUILD > /tmp/PKGBUILD
sed -e "s|%%VERSION%%|${V}|g" -e "s|%%SHA_X86_64%%|${X64}|g" -e "s|%%SHA_AARCH64%%|${ARM}|g" SRCINFO > /tmp/.SRCINFO

git clone ssh://aur@aur.archlinux.org/dot-switch-bin.git
cp /tmp/PKGBUILD /tmp/.SRCINFO dot-switch-bin/
cd dot-switch-bin
git add PKGBUILD .SRCINFO
git commit -m "Update to v${V}"
git push
```
