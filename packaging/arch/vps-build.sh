#!/usr/bin/env bash
# Builds the binary Arch package (justice-client-<ver>-1-x86_64.pkg.tar.zst) inside an Arch container
# on the VPS, so Arch users just run `sudo pacman -U`. Called by release.ps1: vps-build.sh <stage-dir>
# <stage-dir>/arch holds PKGBUILD, the .desktop file and the source tarball from make-arch.sh.
set -euo pipefail
stage="$1"
cd "$stage"

# The image only caches the toolchain; `pacman -Syu` below keeps it current. Tagged with the
# Dockerfile's hash so a changed dependency list rebuilds it.
tag="justice-arch-builder:$(sha256sum Dockerfile.arch | cut -c1-12)"
if ! sudo docker image inspect "$tag" >/dev/null 2>&1; then
  sudo docker build -t "$tag" -f Dockerfile.arch .
fi

# Capped so the game servers on this box don't notice the build.
sudo docker run --rm --cpus 4 --memory 8g -v "$stage/arch:/build" "$tag" bash -c '
  pacman -Syu --noconfirm >/dev/null
  chown -R builder /build
  su builder -c "cd /build && makepkg -f --noconfirm"
'
ls -la "$stage"/arch/*.pkg.tar.zst
