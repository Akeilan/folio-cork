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
  echo "== portability rules (X3-12)"
  (cd $R && tools/check-portability.sh)
  echo "== unit, pixel and conformance tests (no display)"
  (cd $R && cargo test -q --workspace --features folio-platform/audio 2>&1 | grep -E "test result: FAILED|FAILED|panicked"; echo "  $(cd $R && cargo test -q --workspace --features folio-platform/audio 2>&1 | grep -c 'test result: ok') test groups ok")
  echo "== same results built for this CPU's own instructions (AVX2/FMA)"
  (cd $R && RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=/tmp/cork-native cargo test -q -p folio-cork templates 2>&1 | grep "test result"; RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=/tmp/cork-native cargo test -q -p folio-kit --test pixels 2>&1 | grep "test result"; rm -rf /tmp/cork-native)
  echo "== the same tests on ARM64 (the Pi's CPU), under qemu: identical pixels and files"
  (cd $R && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=/tmp/cork-arm64 cargo test -q --config tools/arm64.toml -Z build-std=std,panic_abort,panic_unwind --target aarch64-unknown-linux-gnu --workspace 2>&1 | grep -E "test result: FAILED|panicked|differ"; echo "  $(cd $R && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=/tmp/cork-arm64 cargo test -q --config tools/arm64.toml -Z build-std=std,panic_abort,panic_unwind --target aarch64-unknown-linux-gnu --workspace 2>&1 | grep -c 'test result: ok') test groups ok on arm64"; rm -rf /tmp/cork-arm64)
  echo "== window conformance (X11)"
  (cd $R && cargo build -q -p folio-platform --example conform && DISPLAY=:$XD WAYLAND_DISPLAY= tools/conformance.sh | tail -1)
  echo "== Pi 4B budgets, measured here (X3-12)"
  (cd $R && cargo build -q --release -p folio-cork && ./target/release/folio-cork --bench)
  echo "== window test (source, X)"
  (cd $R && DISPLAY=:$XD WAYLAND_DISPLAY= cargo run -q --release -p folio-cork -- --ui-test /tmp/cork-ui-src 2>&1 | grep -E "^FAIL|passed")
  echo "== Folio Draw on the portable base (D3-06, not shipped yet): window test on X and without a display"
  (cd $R && cargo build -q --release -p folio-draw && rm -rf /tmp/draw-ui && DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 ./target/release/folio-draw --ui-test /tmp/draw-ui 2>&1 | grep -E "^FAIL|passed"; rm -rf /tmp/draw-hl; DISPLAY= WAYLAND_DISPLAY= timeout 300 ./target/release/folio-draw --ui-test /tmp/draw-hl --headless | grep -E "^FAIL|passed" | sed 's/^/no display: /')
fi
echo "== deb"
sh $R/build-deb.sh /home/claude/out && ls -la /home/claude/out/folio-cork_${V}_amd64.deb
for m in ${MACHINES:-/srv/jammy /srv/nolo /srv/noble}; do
  echo "== installed in $m"
  cp /home/claude/out/folio-cork_${V}_amd64.deb $m/tmp/
  flock /home/claude/.dpkg.lock chroot $m bash -c "DEBIAN_FRONTEND=noninteractive apt-get install -y -q /tmp/folio-cork_${V}_amd64.deb >/tmp/cork-install.log 2>&1 || tail -5 /tmp/cork-install.log; dpkg -l folio-cork | tail -1 | awk '{print \$1, \$3}'; folio-cork --version"
  chroot $m bash -c "rm -rf /tmp/cork-ui; DISPLAY=:$XD WAYLAND_DISPLAY= timeout 300 folio-cork --ui-test /tmp/cork-ui 2>&1 | grep -E '^FAIL|passed'; folio-cork --export /tmp/cork-ui/ui-test.vsdx /tmp/cork-ui/cli.pdf && head -c 5 /tmp/cork-ui/cli.pdf && echo ' cli export ok'; folio-cork --export /tmp/cork-ui/config/clipboard.png /tmp/cork-ui/dia.vsdx && echo 'diagram picture opens'; rm -rf /tmp/cork-hl; DISPLAY= WAYLAND_DISPLAY= timeout 300 folio-cork --ui-test /tmp/cork-hl --headless | grep -E '^FAIL|passed' | sed 's/^/no display: /'; ls /usr/share/folio/fonts/Carlito-Regular.ttf >/dev/null && echo 'bundled fonts installed'"
done
echo "== Wayland (Weston) on 24.04"
cp /home/claude/folio-suite/tests/wl.sh /srv/noble/tmp/wl.sh
cp $R/target/debug/examples/conform $R/tools/conformance.sh /srv/noble/tmp/
chroot /srv/noble bash -c "DISPLAY=:$XD /tmp/wl.sh /tmp/conformance.sh /tmp/conform | tail -1"
chroot /srv/noble bash -c "rm -rf /tmp/cork-wl; DISPLAY=:$XD timeout 300 /tmp/wl.sh folio-cork --ui-test /tmp/cork-wl 2>&1 | grep -E '^FAIL|passed'"
exit 0
