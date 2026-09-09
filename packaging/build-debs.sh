#!/bin/bash
# packaging/build-debs.sh — assemble .deb packages from release binaries.
# Usage: ./packaging/build-debs.sh   (output goes to ./dist/)
# Version is read from fv-calendar/Cargo.toml (all crates share it).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' fv-calendar/Cargo.toml | cut -d'"' -f2)
ARCH=amd64
OUT=dist
rm -rf "$OUT" staging
mkdir -p "$OUT"

declare -A DESCS=(
  [fv-calendar]="Simple calendar with per-day notes and holiday highlighting"
  [fv-notepad]="Simple GTK text editor"
  [fv-calculator]="Simple GTK calculator"
  [fv-passman]="Simple encrypted password manager (AES-256-GCM + Argon2)"
)

cargo build --release

for app in fv-calendar fv-notepad fv-calculator fv-passman; do
  pkgdir="staging/$app"
  mkdir -p "$pkgdir/DEBIAN" "$pkgdir/usr/bin" "$pkgdir/usr/share/applications"
  cp "target/release/$app" "$pkgdir/usr/bin/"
  cp "assets/$app.desktop" "$pkgdir/usr/share/applications/"
  cat > "$pkgdir/DEBIAN/control" <<EOF
Package: $app
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: amogusgggy <amogusgggy@internet.ru>
Depends: libgtk-4-1 (>= 4.12)
Description: ${DESCS[$app]}
 Part of the fv-pack system utilities for FVLinux.
EOF
  dpkg-deb --build "$pkgdir" "$OUT/${app}_${VERSION}_${ARCH}.deb"
done

rm -rf staging
echo "Built:"
ls -la "$OUT"/
