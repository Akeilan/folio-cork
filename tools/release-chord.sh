#!/bin/bash
# Folio Chord release checks (the Rust music player on the portable base): unit tests, the
# portability rules, the window test on X and without a display and the command line from source,
# the .deb, then the installed package on 22.04, 22.04 without LibreOffice and 24.04, and a
# Wayland pass on 24.04.   XD=<X display> tools/release-chord.sh
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/folio-chord/packaging/DEBIAN/control)
MUSIC=${MUSIC:-/home/claude/winbuild/t2/music}
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit tests (no display)"
  (cd $R && cargo test -q --release --no-fail-fast --workspace > /tmp/chord-tests.log 2>&1; grep -E "test result: FAILED|panicked" /tmp/chord-tests.log; echo "  $(grep -c 'test result: ok' /tmp/chord-tests.log) test groups ok")
  echo "== window test (source): X and without a display"
  (cd $R && cargo build -q --release -p folio-chord && rm -rf /tmp/chord-ui-src && FOLIO_CHORD_MUSIC=$MUSIC DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 ./target/release/folio-chord --ui-test /tmp/chord-ui-src --library /tmp/chord-ui-src/library.txt 2>/dev/null | grep -E "^FAIL|passed")
  (cd $R && rm -rf /tmp/chord-hl-src && FOLIO_CHORD_MUSIC=$MUSIC DISPLAY= WAYLAND_DISPLAY= timeout 300 ./target/release/folio-chord --ui-test /tmp/chord-hl-src --library /tmp/chord-hl-src/library.txt --headless 2>/dev/null | grep -E "^FAIL|passed" | sed 's/^/no display: /')
  echo "== the command line (source)"
  (cd $R && D=$(mktemp -d) && ./target/release/folio-chord --scan $MUSIC --library $D/library.txt 2>/dev/null | tail -1)
fi
echo "== debs"
FV=$(sed -n 's/^Version: //p' $R/packaging-fonts/DEBIAN/control)
sh $R/build-fonts-deb.sh /home/claude/out && ls -la /home/claude/out/folio-fonts_${FV}_all.deb
sh $R/folio-chord/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-chord_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-chord_${V}_amd64.deb /home/claude/out/folio-fonts_${FV}_all.deb $m/tmp/
  rm -rf $m/tmp/music; mkdir -p $m/tmp/music; cp $MUSIC/* $m/tmp/music/ 2>/dev/null
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q --allow-downgrades /tmp/folio-fonts_${FV}_all.deb /tmp/folio-chord_${V}_amd64.deb >/tmp/chord-install.log 2>&1 || tail -5 /tmp/chord-install.log; dpkg -l folio-fonts folio-chord | tail -2 | awk '{print \$1, \$2, \$3}'; folio-chord --version"
  chroot $m bash -c "rm -rf /tmp/chord-ui; FOLIO_CHORD_MUSIC=/tmp/music DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 folio-chord --ui-test /tmp/chord-ui --library /tmp/chord-ui/library.txt 2>/dev/null | grep -E '^FAIL|passed'; rm -rf /tmp/chord-hl; FOLIO_CHORD_MUSIC=/tmp/music DISPLAY= WAYLAND_DISPLAY= timeout 300 folio-chord --ui-test /tmp/chord-hl --library /tmp/chord-hl/library.txt --headless 2>/dev/null | grep -E '^FAIL|passed' | sed 's/^/no display: /'; folio-chord --scan /tmp/music --library /tmp/cli-library.txt 2>/dev/null | tail -1; ls /usr/share/folio/fonts/Carlito-Regular.ttf >/dev/null && echo 'bundled fonts installed'; ls /usr/share/icons/hicolor/48x48/apps/folio-chord.png >/dev/null && echo 'icons installed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "rm -rf /tmp/chord-wl; FOLIO_CHORD_MUSIC=/tmp/music DISPLAY=:$XD timeout 400 /tmp/wl.sh folio-chord --ui-test /tmp/chord-wl --library /tmp/chord-wl/library.txt 2>/dev/null | grep -E '^FAIL|passed'"
exit 0
