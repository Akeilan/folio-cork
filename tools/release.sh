#!/bin/bash
# Folio Cork release checks: unit tests and the window test from source, the .deb, then the installed
# package on 22.04, 22.04 without LibreOffice and 24.04 (window test on X), and a Wayland pass on 24.04.
#   XD=<X display> tools/release.sh
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/packaging/DEBIAN/control)
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== unit tests"
  (cd $R && cargo test -q 2>&1 | grep -E "test result|FAILED|panicked")
  echo "== window test (source, X)"
  (cd $R && DISPLAY=:$XD WAYLAND_DISPLAY= cargo run -q -- --ui-test /tmp/cork-ui-src 2>&1 | grep -E "^FAIL|passed")
fi
echo "== deb"
sh $R/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-cork_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-cork_${V}_amd64.deb $m/tmp/
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/folio-cork_${V}_amd64.deb >/tmp/cork-install.log 2>&1 || tail -5 /tmp/cork-install.log; dpkg -l folio-cork | tail -1 | awk '{print \$1, \$3}'; folio-cork --version"
  chroot $m bash -c "rm -rf /tmp/cork-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 folio-cork --ui-test /tmp/cork-ui 2>&1 | grep -E '^FAIL|passed'; folio-cork --export /tmp/cork-ui/ui-test.vsdx /tmp/cork-ui/cli.pdf && head -c 5 /tmp/cork-ui/cli.pdf && echo ' cli export ok'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "rm -rf /tmp/cork-wl; DISPLAY=:$XD timeout 300 /tmp/wl.sh folio-cork --ui-test /tmp/cork-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
