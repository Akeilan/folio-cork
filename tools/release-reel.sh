#!/bin/bash
# Folio Reel release checks (the Rust video editor on the portable base): unit tests, the
# portability rules, the window test on X and without a display and the command line from source,
# the .deb, then the installed package on 22.04, 22.04 without LibreOffice and 24.04, and a
# Wayland pass on 24.04.   XD=<X display> tools/release-reel.sh
#
# Reel needs ffmpeg to read and write video, so each machine is asked whether it has one and the
# answer is printed: with ffmpeg the test cuts and exports a real film, without it the test proves
# that Reel says so plainly instead of pretending.
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/folio-reel/packaging/DEBIAN/control)
DOC=/tmp/reel-sample.reel
MEDIA=/tmp/reel-sample-media
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit tests (no display)"
  (cd $R && cargo test -q --release --no-fail-fast --workspace > /tmp/reel-tests.log 2>&1; grep -E "test result: FAILED|panicked" /tmp/reel-tests.log; echo "  $(grep -c 'test result: ok' /tmp/reel-tests.log) test groups ok")
  echo "== window test (source): X and without a display"
  (cd $R && cargo build -q --release -p folio-reel && rm -rf /tmp/reel-ui-src && DISPLAY=:$XD WAYLAND_DISPLAY= timeout 900 ./target/release/folio-reel --ui-test /tmp/reel-ui-src 2>&1 | grep -E "^FAIL|passed")
  (cd $R && rm -rf /tmp/reel-hl-src && DISPLAY= WAYLAND_DISPLAY= timeout 900 ./target/release/folio-reel --ui-test /tmp/reel-hl-src --headless | grep -E "^FAIL|passed" | sed 's/^/no display: /')
  echo "== the command line (source)"
  rm -rf $MEDIA && mkdir -p $MEDIA
  (cd $R && cp /tmp/reel-ui-src/ui-test.reel $DOC 2>/dev/null; cp /tmp/reel-ui-src/*.mp4 /tmp/reel-ui-src/*.mp3 /tmp/reel-ui-src/still.png $MEDIA/ 2>/dev/null)
  (cd $R && ./target/release/folio-reel --presets | head -2)
  (cd $R && [ -f $DOC ] && ./target/release/folio-reel --info $DOC | head -4)
  (cd $R && D=$(mktemp -d) && if command -v ffmpeg >/dev/null; then
      ./target/release/folio-reel --export $D/cli.mp4 $DOC --preset "Smaller file (MP4, 720p)" >/dev/null && echo "  --export wrote $(stat -c%s $D/cli.mp4) bytes of MP4"
      ./target/release/folio-reel --frame $D/cli.png $DOC 1 >/dev/null && echo "  --frame wrote $(stat -c%s $D/cli.png) bytes of PNG"
      ./target/release/folio-reel --probe $D/cli.mp4 | sed 's/^/ /'
    else echo "  no ffmpeg here: $(./target/release/folio-reel --export $D/cli.mp4 $DOC 2>&1 | head -1)"; fi)
fi
echo "== debs"
FV=$(sed -n 's/^Version: //p' $R/packaging-fonts/DEBIAN/control)
sh $R/build-fonts-deb.sh /home/claude/out && ls -la /home/claude/out/folio-fonts_${FV}_all.deb
sh $R/folio-reel/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-reel_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-reel_${V}_amd64.deb /home/claude/out/folio-fonts_${FV}_all.deb $m/tmp/
  [ -f $DOC ] && cp $DOC $m/tmp/sample.reel
  mkdir -p $m/tmp/reel-media && cp $MEDIA/* $m/tmp/reel-media/ 2>/dev/null
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q --allow-downgrades /tmp/folio-fonts_${FV}_all.deb /tmp/folio-reel_${V}_amd64.deb >/tmp/reel-install.log 2>&1 || tail -5 /tmp/reel-install.log; dpkg -l folio-fonts folio-reel | tail -2 | awk '{print \$1, \$2, \$3}'; folio-reel --version; command -v ffmpeg >/dev/null && echo 'ffmpeg: yes' || echo 'ffmpeg: no (Reel should say so plainly)'"
  chroot $m bash -c "rm -rf /tmp/reel-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout 900 folio-reel --ui-test /tmp/reel-ui 2>&1 | grep -E '^FAIL|passed'; rm -rf /tmp/reel-hl; DISPLAY= WAYLAND_DISPLAY= timeout 900 folio-reel --ui-test /tmp/reel-hl --headless | grep -E '^FAIL|passed' | sed 's/^/no display: /'; if [ -f /tmp/sample.reel ]; then folio-reel --info /tmp/sample.reel | head -2; fi; if command -v ffmpeg >/dev/null && [ -f /tmp/reel-ui/ui-test.reel ]; then folio-reel --export /tmp/reel-cli.mp4 /tmp/reel-ui/ui-test.reel --preset 'Smaller file (MP4, 720p)' >/dev/null && echo \"  --export wrote \$(stat -c%s /tmp/reel-cli.mp4) bytes\"; fi; ls /usr/share/folio/fonts/Carlito-Regular.ttf >/dev/null && echo 'bundled fonts installed'; ls /usr/share/icons/hicolor/48x48/apps/folio-reel.png >/dev/null && echo 'icons installed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "rm -rf /tmp/reel-wl; DISPLAY=:$XD timeout 900 /tmp/wl.sh folio-reel --ui-test /tmp/reel-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
