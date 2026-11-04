#!/bin/bash
# X3-12: the portability rules, checked. Run from the repository root; exits 1 on a breach.
# See docs/PORTABILITY.md for the rules themselves.
cd "$(dirname "$0")/.." || exit 2
fail=0
breach() { echo "BREACH: $1"; shift; printf '  %s\n' "$@"; fail=1; }

# Code above the platform layer: the toolkit and the apps (tests may do as they like).
ABOVE=$(find crates -path crates/folio-platform -prune -o -name '*.rs' -print; find folio-* -name '*.rs' 2>/dev/null)
ABOVE=$(echo "$ABOVE" | grep -v '/tests\.rs$\|/tests/' )

# 1. Only the platform layer touches the OS.
os=$(grep -nE 'std::process::Command|std::net::|std::os::|std::env::(var|set_var|current_exe|temp_dir|home_dir)|libc::|extern "C"|#\[link|cfg\((target_os|windows|unix)|cfg!\((target_os|windows|unix)' $ABOVE)
[ -n "$os" ] && breach "OS calls outside crates/folio-platform" "$os"

# 2. No crate that talks to the OS outside the platform layer.
syscrates='^(winit|softbuffer|arboard|x11|x11rb|x11-dl|wayland-[a-z-]+|smithay[a-z-]*|windows|windows-sys|winapi|objc2?[a-z-]*|cocoa|core-foundation|core-graphics|libc|nix|alsa[a-z-]*|cpal|getrandom) '
for c in folio-kit $(ls -d folio-*/ | tr -d /); do
  direct=$(cargo tree -p "$c" -e normal --depth 1 --prefix none 2>/dev/null | tail -n +2 | grep -E "$syscrates")
  [ -n "$direct" ] && breach "$c depends directly on a system crate" "$direct"
done

# 3. Identical results on every CPU and OS: no fused multiply-add, no system maths library.
fma=$(grep -rnE '\.mul_add\(|fmadd|vfma' $ABOVE crates/folio-platform)
[ -n "$fma" ] && breach "fused multiply-add" "$fma"
trig=$(grep -nE '\.(sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|exp|exp2|ln|log|log2|log10|powf|powi|cbrt|hypot|sin_cos)\(' $ABOVE | grep -v 'crates/folio-kit/src/math.rs')
[ -n "$trig" ] && breach "system maths (use folio_kit::math::Det: .dsin(), .dcos(), .sq() ...)" "$trig"
# The renderer's two approximate blend modes give different pixels on different CPUs.
blend=$(grep -nE 'BlendMode::(ColorBurn|ColorDodge)' $ABOVE)
[ -n "$blend" ] && breach "blend modes that use approximate CPU instructions" "$blend"

# 4. Every vector path has a plain fallback: no hand-written SIMD above the platform layer
#    without a cfg-less twin (none today; tested by building for the baseline CPU too).
simd=$(grep -nE 'core::arch|std::arch|#\[target_feature' $ABOVE)
[ -n "$simd" ] && breach "hand-written vector code (needs a plain-Rust fallback and a test that compares them)" "$simd"

# 5. File formats are written down.
for f in docs/FORMATS.md docs/PORTABILITY.md docs/PLATFORM.md; do
  [ -s "$f" ] || breach "missing $f"
done

[ $fail = 0 ] && echo "portability rules: ok"
exit $fail
