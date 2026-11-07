#!/bin/sh
# Builds folio-show_<version>_amd64.deb. The binary is built on Ubuntu 22.04 (/srv/jammy) so it
# runs on 22.04 and later.   sh build-deb.sh [OUTDIR]
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
WS="$(cd "$HERE/.." && pwd)"
VERSION="$(sed -n 's/^Version: //p' "$HERE/packaging/DEBIAN/control")"
CARGOV="$(sed -n 's/^version = "\(.*\)"/\1/p' "$HERE/Cargo.toml" | head -1)"
[ "$VERSION" = "$CARGOV" ] || { echo "version mismatch: control $VERSION, Cargo $CARGOV"; exit 1; }
OUT="${1:-$HERE}"
J=/srv/jammy
for d in /root/.cargo /root/.rustup "$WS"; do mkdir -p "$J$d"; mountpoint -q "$J$d" || mount --bind "$d" "$J$d"; done
chroot $J bash -c "export PATH=/root/.cargo/bin:\$PATH CARGO_TARGET_DIR=/tmp/show-target; cd $WS && cargo build --release -q -p folio-show"
BIN=$J/tmp/show-target/release/folio-show
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -a "$HERE/packaging/root/." "$STAGE/"
install -D -m 755 "$BIN" "$STAGE/usr/bin/folio-show"
"$BIN" --write-icon-svg /tmp/folio-show-icon.svg
install -D -m 644 /tmp/folio-show-icon.svg "$STAGE/usr/share/icons/hicolor/scalable/apps/folio-show.svg"
for s in 16 24 32 48 64 128 256; do
  mkdir -p "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps"
  "$BIN" --write-icon "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps/folio-show.png" $s
done
# the blank presentations a new file starts from
mkdir -p "$STAGE/usr/share/folio-show/templates"
cp "$HERE/templates/"*.pptx "$STAGE/usr/share/folio-show/templates/"
mkdir -p "$STAGE/DEBIAN"
cp "$HERE/packaging/DEBIAN/"* "$STAGE/DEBIAN/"
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"
SIZE="$(du -sk "$STAGE" | cut -f1)"
sed -i "s/^Installed-Size: .*/Installed-Size: $SIZE/" "$STAGE/DEBIAN/control"
dpkg-deb --root-owner-group --build -Zxz "$STAGE" "$OUT/folio-show_${VERSION}_amd64.deb"
echo "Built $OUT/folio-show_${VERSION}_amd64.deb"
