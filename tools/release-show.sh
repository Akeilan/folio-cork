#!/bin/bash
# Folio Show release checks (the Rust spreadsheet on the portable base): unit tests, the portability
# rules, the window test on X and without a display and the command line from source, the .deb,
# then the installed package on 22.04, 22.04 without LibreOffice and 24.04, and a Wayland pass
# on 24.04.   XD=<X display> tools/release-show.sh
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/folio-show/packaging/DEBIAN/control)
DOC=$R/folio-show/templates/Blank.pptx
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit tests (no display)"
  # once, into a file: running the whole workspace's tests twice doubles the wait
  (cd $R && cargo test -q --release --no-fail-fast --workspace > /tmp/show-tests.log 2>&1; grep -E "test result: FAILED|panicked" /tmp/show-tests.log; echo "  $(grep -c 'test result: ok' /tmp/show-tests.log) test groups ok")
  echo "== window test (source): X and without a display"
  (cd $R && cargo build -q --release -p folio-show && rm -rf /tmp/show-ui-src && DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 ./target/release/folio-show --ui-test /tmp/show-ui-src 2>&1 | grep -E "^FAIL|passed")
  (cd $R && rm -rf /tmp/show-hl-src && DISPLAY= WAYLAND_DISPLAY= timeout 300 ./target/release/folio-show --ui-test /tmp/show-hl-src --headless | grep -E "^FAIL|passed" | sed 's/^/no display: /')
  echo "== the command line (source)"
  (cd $R && D=$(mktemp -d) && ./target/release/folio-show --png $DOC $D/cli.png && echo "  --png wrote $(stat -c%s $D/cli.png) bytes of PNG")
fi
echo "== debs"
FV=$(sed -n 's/^Version: //p' $R/packaging-fonts/DEBIAN/control)
sh $R/build-fonts-deb.sh /home/claude/out && ls -la /home/claude/out/folio-fonts_${FV}_all.deb
sh $R/folio-show/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-show_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-show_${V}_amd64.deb /home/claude/out/folio-fonts_${FV}_all.deb $m/tmp/
  cp $DOC $m/tmp/blank.pptx
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q --allow-downgrades /tmp/folio-fonts_${FV}_all.deb /tmp/folio-show_${V}_amd64.deb >/tmp/show-install.log 2>&1 || tail -5 /tmp/show-install.log; dpkg -l folio-fonts folio-show | tail -2 | awk '{print \$1, \$2, \$3}'; folio-show --version"
  chroot $m bash -c "rm -rf /tmp/show-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 folio-show --ui-test /tmp/show-ui 2>&1 | grep -E '^FAIL|passed'; rm -rf /tmp/show-hl; DISPLAY= WAYLAND_DISPLAY= timeout 300 folio-show --ui-test /tmp/show-hl --headless | grep -E '^FAIL|passed' | sed 's/^/no display: /'; folio-show --png /tmp/blank.pptx /tmp/show-cli.png && echo \"  --png wrote \$(stat -c%s /tmp/show-cli.png) bytes\"; ls /usr/share/folio-show/templates/Blank.pptx >/dev/null && echo 'templates installed'; ls /usr/share/folio/fonts/Carlito-Regular.ttf >/dev/null && echo 'bundled fonts installed'; ls /usr/share/icons/hicolor/48x48/apps/folio-show.png >/dev/null && echo 'icons installed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "rm -rf /tmp/show-wl; DISPLAY=:$XD timeout 400 /tmp/wl.sh folio-show --ui-test /tmp/show-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
