#!/usr/bin/env bash
# Builds the Arch source package: ./make-arch.sh <version> <output-dir>
# Produces <output-dir>/justice-client-arch.tar.gz containing PKGBUILD + source tarball.
set -euo pipefail
version="$1"
outdir="$2"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

cd "$root"
tar --exclude=node_modules --exclude=dist --exclude='src-tauri/target' --exclude='src-tauri/target-linux' \
    --exclude='src-tauri/gen' --exclude=packaging \
    --transform "s,^\.,justice-client-$version," -czf /tmp/jc-src.tar.gz .

cd "$here"
rm -f justice-client-*.tar.gz
mv /tmp/jc-src.tar.gz "justice-client-$version.tar.gz"
sed -i "s/^pkgver=.*/pkgver=$version/" PKGBUILD
sum="$(sha256sum "justice-client-$version.tar.gz" | cut -d' ' -f1)"
sed -i "s/^sha256sums=('[0-9a-f]*'/sha256sums=('$sum'/" PKGBUILD
sed -i 's/\r$//' PKGBUILD justice-client.desktop

cd "$here/.."
tar --exclude='arch/make-arch.sh' -czf "$outdir/justice-client-arch.tar.gz" arch
echo "arch package: $outdir/justice-client-arch.tar.gz"
