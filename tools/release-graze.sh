#!/bin/bash
# Folio Graze release checks: portability rules, unit tests, the window test from source (normal and
# private), the .deb built on 22.04, then the installed package on 22.04, 22.04 without LibreOffice and
# 24.04 (window test on X), and a Wayland pass on 24.04.   XD=<X display> tools/release-graze.sh
set -u
R=/home/claude/folio-cork
XD=${XD:-99}
V=$(sed -n 's/^Version: //p' $R/packaging-graze/DEBIAN/control)
NOPROXY="env -u https_proxy -u HTTPS_PROXY -u http_proxy -u HTTP_PROXY"
/home/claude/mounts.sh >/dev/null 2>&1
pgrep -x Xvfb -a | grep -q ":$XD " || { rm -f /tmp/.X$XD-lock /tmp/.X11-unix/X$XD; setsid -f Xvfb :$XD -screen 0 1600x1000x24 -nolisten tcp </dev/null >/dev/null 2>&1; sleep 1; }
if [ -z "${INSTALLED_ONLY:-}" ]; then
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit tests (no display)"
  (cd $R && cargo test -q -p folio-graze 2>&1 | grep -E "test result|FAILED|panicked")
  echo "== window test (source, X)"
  (cd $R && cargo build -q -p folio-graze && rm -rf /tmp/graze-src && $NOPROXY DISPLAY=:$XD WAYLAND_DISPLAY= timeout -k 5 180 ./target/debug/folio-graze --ui-test /tmp/graze-src 2>&1 | grep -E "^FAIL|passed")
  echo "== window test, private window (source, X)"
  (cd $R && rm -rf /tmp/graze-src-p && $NOPROXY DISPLAY=:$XD WAYLAND_DISPLAY= timeout -k 5 180 ./target/debug/folio-graze --private --ui-test /tmp/graze-src-p 2>&1 | grep -E "^FAIL|passed")
fi
echo "== deb"
sh $R/build-graze-deb.sh /home/claude/out >/dev/null && ls -la /home/claude/out/folio-graze_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-graze_${V}_amd64.deb $m/tmp/
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/folio-graze_${V}_amd64.deb >/tmp/graze-install.log 2>&1 || tail -5 /tmp/graze-install.log; apt-get clean; dpkg -l folio-graze | tail -1 | awk '{print \$1, \$3}'; folio-graze --version"
  chroot $m bash -c "unset https_proxy HTTPS_PROXY http_proxy HTTP_PROXY; rm -rf /tmp/graze-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout -k 5 180 folio-graze --ui-test /tmp/graze-ui 2>&1 | grep -E '^FAIL|passed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
chroot /srv/noble bash -c "unset https_proxy HTTPS_PROXY http_proxy HTTP_PROXY; rm -rf /tmp/graze-wl; DISPLAY=:$XD timeout -k 5 180 /tmp/wl.sh folio-graze --ui-test /tmp/graze-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
