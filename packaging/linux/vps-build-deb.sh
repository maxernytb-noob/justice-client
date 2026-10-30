#!/usr/bin/env bash
# Builds the .deb and the AppImage inside an Ubuntu 22.04 container on the VPS.
# Called by release.ps1: vps-build-deb.sh <stage-dir> <version>
# <stage-dir> holds Dockerfile.deb and the source tarball justice-client-<version>.tar.gz (from make-arch.sh).
# Output: <stage-dir>/out/justice-client_<version>_amd64.deb and JusticeClient-<version>-x86_64.AppImage
set -euo pipefail
stage="$1"
version="$2"
cd "$stage"

# The image only caches the toolchain; rebuilt when this Dockerfile changes.
tag="justice-deb-builder:$(sha256sum Dockerfile.deb | cut -c1-12)"
if ! sudo docker image inspect "$tag" >/dev/null 2>&1; then
  sudo docker build -t "$tag" -f Dockerfile.deb .
fi

# The container writes as root; clear leftovers from an earlier run the same way.
sudo rm -rf src out && mkdir -p src out
tar -xzf "justice-client-$version.tar.gz" -C src --strip-components=1

# Capped so the game servers on this box don't notice the build.
# APPIMAGE_EXTRACT_AND_RUN: linuxdeploy is itself an AppImage and there's no FUSE in the container.
# NO_STRIP: linuxdeploy's bundled strip can't read newer ELF sections and fails the build.
# No updater artifacts: Linux builds update through the website, not the signed Windows feed.
sudo docker run --rm --cpus 4 --memory 8g -e APPIMAGE_EXTRACT_AND_RUN=1 -e NO_STRIP=1 \
  -v "$stage/src:/src" -v "$stage/out:/out" "$tag" bash -c '
  set -e
  cd /src
  npm ci --no-audit --no-fund
  npx tauri build --bundles deb,appimage --config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"
  cp src-tauri/target/release/bundle/deb/*.deb /out/
  cp src-tauri/target/release/bundle/appimage/*.AppImage /out/
  chown -R '"$(id -u):$(id -g)"' /out /src
'
cd out
mv ./*.deb "justice-client_${version}_amd64.deb"
mv ./*.AppImage "JusticeClient-${version}-x86_64.AppImage"
chmod +x "JusticeClient-${version}-x86_64.AppImage"
ls -la
