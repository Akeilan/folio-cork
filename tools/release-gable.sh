#!/bin/bash
# Folio Gable release checks (the Rust spreadsheet on the portable base): unit tests, the portability
# rules, the window test on X and without a display and the command line from source, the .deb,
# then the installed package on 22.04, 22.04 without LibreOffice and 24.04, and a Wayland pass
# on 24.04.   XD=<X display> tools/release-gable.sh
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/folio-gable/packaging/DEBIAN/control)
DOC=/tmp/gable-sample.gable
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit tests (no display)"
  # once, into a file: running the whole workspace's tests twice doubles the wait
  (cd $R && cargo test -q --release --no-fail-fast --workspace > /tmp/gable-tests.log 2>&1; grep -E "test result: FAILED|panicked" /tmp/gable-tests.log; echo "  $(grep -c 'test result: ok' /tmp/gable-tests.log) test groups ok")
  echo "== window test (source): X and without a display"
  (cd $R && cargo build -q --release -p folio-gable && rm -rf /tmp/gable-ui-src && DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 ./target/release/folio-gable --ui-test /tmp/gable-ui-src 2>&1 | grep -E "^FAIL|passed")
  (cd $R && rm -rf /tmp/gable-hl-src && DISPLAY= WAYLAND_DISPLAY= timeout 300 ./target/release/folio-gable --ui-test /tmp/gable-hl-src --headless | grep -E "^FAIL|passed" | sed 's/^/no display: /')
  echo "== the command line (source)"
  (cd $R && ./target/release/folio-gable --ui-test /tmp/gable-make >/dev/null 2>&1; cp /tmp/gable-make/ui-test.gable $DOC 2>/dev/null)
  (cd $R && D=$(mktemp -d) && ./target/release/folio-gable --png $DOC $D/cli.png && echo "  --png wrote $(stat -c%s $D/cli.png) bytes of PNG" && ./target/release/folio-gable --export $D/cli.stl $DOC && echo "  --export wrote $(stat -c%s $D/cli.stl) bytes of STL")
fi
echo "== debs"
FV=$(sed -n 's/^Version: //p' $R/packaging-fonts/DEBIAN/control)
sh $R/build-fonts-deb.sh /home/claude/out && ls -la /home/claude/out/folio-fonts_${FV}_all.deb
sh $R/folio-gable/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-gable_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-gable_${V}_amd64.deb /home/claude/out/folio-fonts_${FV}_all.deb $m/tmp/
  cp $DOC $m/tmp/sample.gable
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q --allow-downgrades /tmp/folio-fonts_${FV}_all.deb /tmp/folio-gable_${V}_amd64.deb >/tmp/gable-install.log 2>&1 || tail -5 /tmp/gable-install.log; dpkg -l folio-fonts folio-gable | tail -2 | awk '{print \$1, \$2, \$3}'; folio-gable --version"
  chroot $m bash -c "rm -rf /tmp/gable-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 folio-gable --ui-test /tmp/gable-ui 2>&1 | grep -E '^FAIL|passed'; rm -rf /tmp/gable-hl; DISPLAY= WAYLAND_DISPLAY= timeout 300 folio-gable --ui-test /tmp/gable-hl --headless | grep -E '^FAIL|passed' | sed 's/^/no display: /'; folio-gable --png /tmp/sample.gable /tmp/gable-cli.png && echo \"  --png wrote \$(stat -c%s /tmp/gable-cli.png) bytes\"; folio-gable --export /tmp/gable-cli.stl /tmp/sample.gable >/dev/null && echo \"  --export wrote \$(stat -c%s /tmp/gable-cli.stl) bytes\"; ls /usr/share/folio/fonts/Carlito-Regular.ttf >/dev/null && echo 'bundled fonts installed'; ls /usr/share/icons/hicolor/48x48/apps/folio-gable.png >/dev/null && echo 'icons installed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "rm -rf /tmp/gable-wl; DISPLAY=:$XD timeout 400 /tmp/wl.sh folio-gable --ui-test /tmp/gable-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
