#!/usr/bin/env bash
# Helper for Arch users: fetch the latest dot-switch release from GitHub,
# compute checksums, fill them into PKGBUILD, then run makepkg -si.
set -euo pipefail

REPO="lushprey/dot-switch"
API="https://api.github.com/repos/${REPO}/releases/latest"

echo "==> Fetching latest release from ${API}..."
RELEASE_JSON=$(curl -fsSL "${API}")
VERSION=$(echo "${RELEASE_JSON}" | jq -r .tag_name | sed 's/^v//')

if [[ -z "${VERSION}" || "${VERSION}" == "null" ]]; then
    echo "ERROR: could not determine latest version" >&2
    exit 1
fi
echo "    Latest release: v${VERSION}"

BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
X64_TARBALL="dot-switch-v${VERSION}-x86_64-unknown-linux-musl.tar.gz"
ARM_TARBALL="dot-switch-v${VERSION}-aarch64-unknown-linux-musl.tar.gz"

echo "==> Computing sha256 for both arches..."
X64_SHA=$(curl -fsSL "${BASE}/${X64_TARBALL}" | sha256sum | awk '{print $1}')
ARM_SHA=$(curl -fsSL "${BASE}/${ARM_TARBALL}" | sha256sum | awk '{print $1}')
echo "    x86_64 sha256:  ${X64_SHA}"
echo "    aarch64 sha256: ${ARM_SHA}"

echo "==> Rendering PKGBUILD with version + checksums..."
sed -e "s|%%VERSION%%|${VERSION}|g" \
    -e "s|%%SHA_X86_64%%|${X64_SHA}|g" \
    -e "s|%%SHA_AARCH64%%|${ARM_SHA}|g" \
    PKGBUILD > PKGBUILD.resolved
mv PKGBUILD.resolved PKGBUILD

echo ""
echo "Done. PKGBUILD is ready for v${VERSION}."
echo "Next step: makepkg -si"
