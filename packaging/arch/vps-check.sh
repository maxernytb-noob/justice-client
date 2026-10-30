#!/usr/bin/env bash
# Quick Linux compile check on the VPS (no packaging): vps-check.sh <stage-dir with src.tar.gz>
# Reuses a cargo cache between runs, so only the launcher crate itself is rebuilt.
set -euo pipefail
stage="$1"
cd "$stage"
mkdir -p src && tar -xzf src.tar.gz -C src
sudo mkdir -p /srv/justice-cargo-cache
sudo docker run --rm --cpus 4 --memory 8g -v "$stage/src:/src" -v /srv/justice-cargo-cache:/cargo -e CARGO_HOME=/cargo justice-arch-builder bash -c '
  cd /src && mkdir -p dist && [ -f dist/index.html ] || echo "<html></html>" > dist/index.html
  cargo check --manifest-path src-tauri/Cargo.toml --message-format short 2>&1 | grep -E "^(error|warning)|^src|Finished" | head -80
'
