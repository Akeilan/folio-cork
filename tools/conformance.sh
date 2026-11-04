#!/bin/bash
# X3-13: window conformance for a platform backend, on a real display (X11, or Wayland through
# folio-suite/tests/wl.sh). Needs xdotool and xwd.   conformance.sh [path/to/conform]
# Input goes to $XTEST_DISPLAY (the X display behind a nested Wayland session) or $DISPLAY.
BIN=${1:-$(dirname "$0")/../target/debug/examples/conform}
XD=${XTEST_DISPLAY:-$DISPLAY}
LOG=$(mktemp); SHOT=$(mktemp)
pass=0; fail=0
ok() { if [ "$1" = 0 ]; then pass=$((pass+1)); echo "ok   $2"; else fail=$((fail+1)); echo "FAIL $2"; fi; }
has() { grep -qE "$1" "$LOG"; }
x() { DISPLAY=$XD xdotool "$@"; sleep 0.25; }

"$BIN" > "$LOG" 2>&1 & P=$!
for i in $(seq 100); do has '^PAINT' && break; sleep 0.1; done
sleep 1.5
# Where is the window? Find the red top-left square and the green bottom-right one on screen.
locate() {
DISPLAY=$XD xwd -root -silent > "$SHOT"
read OX OY EX EY CENTRE < <(python3 - "$SHOT" <<'EOF'
import struct, sys
d = open(sys.argv[1], "rb").read()
h = struct.unpack(">25I", d[:100])
hs, w, ht, bpp, bpl, ncol = h[0], h[4], h[5], h[11], h[12], h[19]
lsb = h[7] == 0
px = d[hs + ncol * 12:]
def at(x, y):
    i = y * bpl + x * (bpp // 8)
    b = px[i:i + 4]
    return (b[2], b[1], b[0]) if lsb else (b[1], b[2], b[3])
red = green = None
for y in range(0, ht, 2):
    for x in range(0, w, 2):
        c = at(x, y)
        if c == (255, 0, 0) and red is None:
            red = (x, y)
        if c == (0, 255, 0):
            green = (x, y)
if not red or not green:
    print(-1, -1, -1, -1, "none"); sys.exit()
# exact corners
ox, oy = red
while ox > 0 and at(ox - 1, oy) == (255, 0, 0): ox -= 1
while oy > 0 and at(ox, oy - 1) == (255, 0, 0): oy -= 1
ex, ey = green
while ex + 1 < w and at(ex + 1, ey) == (0, 255, 0): ex += 1
while ey + 1 < ht and at(ex, ey + 1) == (0, 255, 0): ey += 1
c = at((ox + ex) // 2, (oy + ey) // 2)
print(ox, oy, ex, ey, "%02x%02x%02x" % c)
EOF
)
}
locate
# Click inside it first: some compositors dim a window until it has the keyboard.
if [ "$OX" -ge 0 ] 2>/dev/null; then x mousemove $((OX+150)) $((OY+150)); x click 1; sleep 0.8; locate; fi
ok $([ "$OX" -ge 0 ] 2>/dev/null; echo $?) "the window shows the app's pixels (red corner at $OX,$OY)"
ok $([ "$CENTRE" = 3366cc ]; echo $?) "pixels are shown unchanged (centre #$CENTRE)"
SIZE=$(grep '^PAINT' "$LOG" | tail -1 | cut -d' ' -f2)
ok $([ "$SIZE" = "$((EX-OX+1))x$((EY-OY+1))" ]; echo $?) "the painted size is the window's size ($SIZE)"
if [ -z "$WAYLAND_DISPLAY" ]; then
  DISPLAY=$XD xprop -name folio-conform-ready WM_NAME >/dev/null 2>&1; ok $? "the title is set"
fi
# Input, in the middle of the window. With no window manager (a bare X server), give the
# window the keyboard the way a window manager would.
if [ -z "$WAYLAND_DISPLAY" ] && ! DISPLAY=$XD xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q window; then
  DISPLAY=$XD xdotool search --name folio-conform-ready windowfocus --sync 2>/dev/null
fi
MX=$(( (OX+EX)/2 )); MY=$(( (OY+EY)/2 ))
x mousemove $MX $MY; x click 1; sleep 0.6
x mousemove $((MX+20)) $((MY+10)); x click --repeat 2 --delay 90 1
x key a; sleep 1; x key shift+b; x click 4; x click 5; x key ctrl+c
x mousedown 1; x mousemove $((MX+80)) $((MY+40)); x mouseup 1
sleep 0.5; x key Escape
for i in $(seq 50); do kill -0 $P 2>/dev/null || break; sleep 0.1; done
kill $P 2>/dev/null; wait $P; RC=$?

first=$(grep -E '^(EV|PAINT)' "$LOG" | head -1)
ok $(echo "$first" | grep -q 'Resized'; echo $?) "Resized comes before anything else"
ok $(has 'Resized .*scale: [0-9]'; echo $?) "Resized carries the scale"
ok $(has 'Focus\(true\)'; echo $?) "focus is reported"
ok $(has 'MouseMove'; echo $?) "mouse movement"
ok $(has 'MouseDown .*button: Left, clicks: 1'; echo $?) "a click"
ok $(has 'MouseDown .*button: Left, clicks: 2'; echo $?) "a double click is counted"
ok $(has 'MouseUp'; echo $?) "mouse release"
ok $(has "Key \{ key: Char\('a'\), mods: Mods \{ ctrl: false, shift: false.*down: true"; echo $?) "a key press"
ok $(has "Key \{ key: Char\('a'\).*down: false"; echo $?) "a key release"
ok $(has 'Text\("a"\)'; echo $?) "typed text"
ok $(has 'Text\("B"\)'; echo $?) "typed text with Shift"
ok $(has "Key \{ key: Char\('b'\), mods: Mods \{ ctrl: false, shift: true"; echo $?) "keys are lower case with the Shift modifier"
ok $(has "Key \{ key: Char\('c'\), mods: Mods \{ ctrl: true"; echo $?) "Ctrl shortcuts"
ok $(! has 'Text\("c"\)'; echo $?) "no typed text for Ctrl shortcuts"
ok $(has 'Modifiers'; echo $?) "modifier changes"
ok $(has 'Wheel \{ dx: [-0-9.]+, dy: [1-9]'; echo $?) "wheel away from the user is positive"
ok $(has 'Wheel \{ dx: [-0-9.]+, dy: -'; echo $?) "wheel towards the user is negative"
ok $(has 'Tick'; echo $?) "timer ticks"
ok $(has '^CLIP text ok'; echo $?) "clipboard text round trip"
if [ -n "$WAYLAND_DISPLAY" ] && [ -z "$DISPLAY" ]; then
  echo "skip clipboard image round trip (pictures on Wayland go through XWayland, which this session doesn't run)"
else
  ok $(has '^CLIP image ok'; echo $?) "clipboard image round trip"
fi
ok $(has 'Key \{ key: Escape'; echo $?) "named keys"
ok $([ "$RC" = 0 ]; echo $?) "the app's exit code comes back ($RC)"
echo "conformance: $pass passed, $fail failed"
[ $fail = 0 ] || { echo "--- log"; head -c 3000 "$LOG"; }
rm -f "$LOG" "$SHOT"
[ $fail = 0 ]
