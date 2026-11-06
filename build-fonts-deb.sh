#!/bin/sh
# Builds folio-fonts_<version>_all.deb: the fonts every Folio app on the portable base draws
# documents with (X3-16).   sh build-fonts-deb.sh [OUTDIR]
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
VERSION="$(sed -n 's/^Version: //p' "$HERE/packaging-fonts/DEBIAN/control")"
OUT="${1:-$HERE}"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/usr/share/folio/fonts" "$STAGE/DEBIAN" "$STAGE/usr/share/doc/folio-fonts"
install -m 644 "$HERE"/fonts/*.ttf "$HERE"/fonts/LICENSE-*.txt "$STAGE/usr/share/folio/fonts/"
cp "$HERE/packaging-fonts/DEBIAN/control" "$STAGE/DEBIAN/control"
cat > "$STAGE/usr/share/doc/folio-fonts/copyright" <<'TXT'
Carlito (SIL Open Font License 1.1) and Liberation Serif and Mono (SIL Open Font
License 1.1). The licences are beside the fonts in /usr/share/folio/fonts.
TXT
SIZE="$(du -sk "$STAGE" | cut -f1)"
sed -i "s/^Installed-Size: .*/Installed-Size: $SIZE/" "$STAGE/DEBIAN/control"
dpkg-deb --root-owner-group --build -Zxz "$STAGE" "$OUT/folio-fonts_${VERSION}_all.deb"
echo "Built $OUT/folio-fonts_${VERSION}_all.deb"
