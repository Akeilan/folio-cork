#!/bin/bash
# X3-16: prove the program is one executable.
#
#   tools/one-executable.sh [folio-draw|folio-cork]
#
# Builds a root holding nothing but the binary, the C runtime it links against, and a shell (the
# scanner-and-camera step runs a stub program, and a machine with no shell could not have a scanner
# program either), then runs that app's window test inside it. No fonts, no fontconfig, no desktop,
# no Folio folder: if the text draws, the fonts really are inside the program (the faces are
# compiled in by crates/folio-kit/src/fonts.rs).
set -u
HERE="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-folio-draw}"
BIN="$HERE/target/release/$APP"
[ -x "$BIN" ] || { echo "build it first: cargo build --release -p $APP" >&2; exit 2; }
B="$(mktemp -d)"
trap 'rm -rf "$B"' EXIT
mkdir -p "$B"/{lib/x86_64-linux-gnu,lib64,tmp,dev,out,bin}
copy_libs() {   # every library a program links against
  for f in $(ldd "$1" 2>/dev/null | grep -oE '/lib[^ ]*'); do
    d="$B$(dirname "$f")"
    mkdir -p "$d"
    cp -L "$f" "$d/" 2>/dev/null
  done
}
copy_libs "$BIN"
cp "$BIN" "$B/$APP"
SH="$(command -v dash || command -v sh)"
cp -L "$SH" "$B/bin/sh"
copy_libs "$SH"
mknod "$B/dev/null" c 1 3 2>/dev/null; chmod 666 "$B/dev/null" 2>/dev/null
mknod "$B/dev/urandom" c 1 9 2>/dev/null; chmod 666 "$B/dev/urandom" 2>/dev/null
echo "the whole machine: $(find "$B" -type f | wc -l) files"
find "$B" -type f | sed "s#$B##" | sort | sed 's/^/  /'
chroot "$B" "/$APP" --ui-test /out --headless
rc=$?
echo "X3-16: $APP $( [ $rc = 0 ] && echo "runs on its own" || echo "FAILED on its own (rc=$rc)" )"
exit $rc
