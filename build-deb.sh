#!/bin/sh
# Builds folio-cork_<version>_amd64.deb. The binary is built on Ubuntu 22.04 (/srv/jammy) so it runs
# on 22.04 and later.   sh build-deb.sh [OUTDIR]
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
VERSION="$(sed -n 's/^Version: //p' "$HERE/packaging/DEBIAN/control")"
# the app crate's own version is what the binary reports, so that is the one to match
CARGOV="$(sed -n 's/^version = "\(.*\)"/\1/p' "$HERE/folio-cork/Cargo.toml" | head -1)"
[ "$VERSION" = "$CARGOV" ] || { echo "version mismatch: control $VERSION, Cargo $CARGOV"; exit 1; }
OUT="${1:-$HERE}"
J=/srv/jammy
for d in /root/.cargo /root/.rustup "$HERE"; do mkdir -p "$J$d"; mountpoint -q "$J$d" || mount --bind "$d" "$J$d"; done
chroot $J bash -c "export PATH=/root/.cargo/bin:\$PATH CARGO_TARGET_DIR=/tmp/cork-target; cd $HERE && cargo build --release -q -p folio-cork"
BIN=$J/tmp/cork-target/release/folio-cork
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -a "$HERE/packaging/root/." "$STAGE/"
install -D -m 755 "$BIN" "$STAGE/usr/bin/folio-cork"
"$BIN" --write-icon-svg /tmp/folio-cork-icon.svg
install -D -m 644 /tmp/folio-cork-icon.svg "$STAGE/usr/share/icons/hicolor/scalable/apps/folio-cork.svg"
for s in 16 24 32 48 64 128 256; do
  mkdir -p "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps"
  "$BIN" --write-icon "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps/folio-cork.png" $s
done
mkdir -p "$STAGE/DEBIAN"
cp "$HERE/packaging/DEBIAN/"* "$STAGE/DEBIAN/"
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"
SIZE="$(du -sk "$STAGE" | cut -f1)"
sed -i "s/^Installed-Size: .*/Installed-Size: $SIZE/" "$STAGE/DEBIAN/control"
dpkg-deb --root-owner-group --build -Zxz "$STAGE" "$OUT/folio-cork_${VERSION}_amd64.deb"
echo "Built $OUT/folio-cork_${VERSION}_amd64.deb"
