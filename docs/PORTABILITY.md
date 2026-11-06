# Portability rules (X3-12)

Every Folio app written in Rust follows these rules. `tools/check-portability.sh` checks the ones a machine can check, and the release script runs it.

## 1. Only the platform layer touches the OS

- `crates/folio-platform` is the only code that calls the operating system: the window, input, clipboard, files beyond plain reading and writing, sockets, threads, printing, the user's name, random numbers and external tools.
- The toolkit (`folio-kit`) and the apps use what the platform layer offers. They never use `std::process`, `std::net`, `std::env`, `std::os`, `libc`, or `cfg(target_os)`.
- No crate that talks to the OS (winit, softbuffer, arboard, windows-sys, objc, alsa, getrandom...) is a direct dependency of the toolkit or an app.
- Plain `std::fs` reads and writes are allowed everywhere: Rust's standard library is the same on every system. Saving always goes through `platform::atomic_write`.

## 2. Vector code has a plain fallback

- Hand-written NEON, AVX, SSE or RISC-V vector code is allowed only with a plain-Rust version beside it, and a test that runs both and compares the results bit for bit.
- There is none today. The renderer (tiny-skia) has its own vector paths and plain fallbacks. The only places they give different results are the ColorBurn and ColorDodge blend modes, which use approximate CPU instructions, so Folio never uses those modes.

## 3. Identical results on every CPU

- No fused multiply-add: no `mul_add`, no FMA intrinsics. Rust never fuses floating-point operations on its own.
- No system maths library. Sine, cosine, tangent, arctangent, logarithm, exponent and power come from `folio_kit::math::Det` (`a.dsin()`, `a.dcos()`, `a.datan2(b)`, `a.sq()`). It is built on the pure-Rust `libm` crate, so every OS and CPU gets the same bits. Square root is exact in IEEE 754 and may be used directly.
- Rotations are built from that same sine and cosine (`geom::rotation`), not the renderer's.
- Text in documents is drawn from glyph outlines, not the system's text rasteriser.
- The pixel tests use only the fonts that ship with Folio (`Text::bundled`), so a page draws the same on every machine.
- Checked by:
  - `cargo test`: the stored pictures in `crates/folio-kit/tests/golden` and the stored fingerprints of rendered pages and exported SVG and PDF in `folio-cork/tests/golden.txt`;
  - `PORTS=1 tools/release.sh`, which repeats the tests built for the build machine's own CPU (AVX2, FMA), and again for ARM64 (the Pi 4B's CPU, with the renderer's NEON paths) under qemu. All of them must give the stored pixels and file fingerprints. These are not part of an ordinary release: see "What a release is tested on" below.
- Known limit: the renderer works out its own curve geometry with the system's maths library in two places (rotating by an angle, which Folio doesn't use, and splitting some cubic curves). That can move an anti-aliased edge pixel on screen between systems. Saved files and exported SVG and PDF are not affected.

## 4. File formats are defined to the byte

- Little-endian, UTF-8, documented in `docs/FORMATS.md`. JSON numbers are written in the shortest form that reads back exactly.
- A file must not carry anything about the machine that wrote it. A .vsdx is a zip, and the zip library stamps each entry with the host system it is running on: Cork sets that field itself, so a drawing saved on Linux, Windows or a Pi is the same file byte for byte (found on 6 November).
- A format change bumps its `format` number, and readers keep reading every older number.

## 5. What a release is tested on

A release must pass on Linux: Ubuntu 22.04 and 24.04, X11 and Wayland. That is the bar for every
app, and it is all `tools/release.sh` runs by default. The rules above keep other processors and
operating systems reachable, but their checks run only when asked for: `PORTS=1 tools/release.sh`
adds the AVX2/FMA build, the ARM64 run under qemu and the Pi budgets. Windows and macOS are not tested at all
(Product Owner, 2026-11-06): no release run touches wine, Windows was proven once by hand with the
command recorded in docs/PLATFORM.md, and macOS is left alone. Any port beyond Zorin OS and Linux
Mint is a bridge crossed when we get there — build so the rules keep it possible, and spend the
time on Zorin. The suite is developed and used on Zorin OS, and Linux Mint is the other system it
is meant for; both are Ubuntu-based, so the 22.04 and 24.04 machines are what releases are held to.

## 6. The Raspberry Pi 4B is the floor

On a Pi 4B, the suite has to be usable, not optimal (Product Owner, 2026-11-04). Faster machines do much better.

Budgets, measured by `folio-cork --bench` (no window, 1920×1080):

| What | Budget on a Pi 4B |
|---|---|
| Open to first frame | under 2 s |
| Scrolling a 400-shape, 380-connector board | 50 ms a frame (20 fps), average and 95th percentile |
| Typing into a shape, keystroke to frame | 50 ms, average and 95th percentile |
| Peak memory | 250 MB per app |

- The bench runs in every release on the build machine. It scales its times by 3, about how much slower a Pi 4B runs this software renderer, to estimate the Pi.
- On the Pi itself, run it with `FOLIO_PI_FACTOR=1`.
- What keeps it there:
  - Only the parts of the drawing that changed are redrawn.
  - What's off screen is skipped.
  - Line routes, glue points, text outlines and the minimap are kept until what they depend on changes.
  - A test checks that a partial redraw gives exactly the pixels of a full one.
