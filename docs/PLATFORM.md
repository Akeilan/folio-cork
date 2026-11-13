# The platform layer contract (X3-13)

`crates/folio-platform` is everything Folio needs from an operating system. A port to a new system is a new implementation of this crate. The toolkit and apps above it don't change.

**A backend is done when it passes the conformance suite:**
- `cargo test -p folio-platform --features audio` for the services;
- `tools/conformance.sh` for the window, on a real display.

## Window, pixels, keyboard and mouse

- `run(app, WindowSpec) -> exit code` opens one window and drives an `App` until it asks to exit.
- **Window:**
  - Borderless. Folio draws its own title bar.
  - Resizable, with a minimum of 640×420 logical pixels.
  - Has an app id (`folio-<app>`) for the taskbar and the desktop file, and an icon.
- **Pixels:**
  - The app paints the whole window into premultiplied RGBA8 (`w * h * 4` bytes, rows top to bottom, no padding) in `App::paint`.
  - The backend shows exactly those pixels, with no scaling or colour management.
  - A paint happens after any `Host::redraw` and after every resize.
- **Events,** all in physical pixels (`Event`):

| Event | Contract |
|---|---|
| `Resized{w,h,scale}` | Sent first, before any paint, and on every size or scale change. `scale` is 1.0 at 96 dpi. |
| `MouseMove`, `MouseDown{button,clicks}`, `MouseUp`, `MouseLeave` | `clicks` counts double and triple clicks: 450 ms and 5 logical pixels between presses (`ClickCounter`). |
| `Wheel{dx,dy}` | In lines. Positive `dy` is away from the user. Touchpad pixels are divided by 40. |
| `Key{key,mods,down,repeat}` | `Key::Char` is the logical key in lower case, whatever the layout. Function keys are `F(1..12)`. |
| `Text(s)` | Composed text from the system's input method. Never sent for Ctrl or Cmd shortcuts. |
| `Modifiers(m)` | Whenever Ctrl, Shift, Alt or the logo key changes. |
| `FileDropped(path)` | Once per dropped file. |
| `Focus(bool)` | The window gained or lost the keyboard. |
| `ThemeChanged{dark}` | The system's light or dark preference changed. |
| `CloseRequested` | The user asked to close. The app decides; nothing closes by itself. |
| `Tick` | When `App::next_tick` asked for it, no earlier than asked and no more than one frame late. |

- **Host** (what an app may ask of the window):
  - redraw, title, cursor shape;
  - start a window move or edge resize (for the drawn title bar and borders);
  - minimise, maximise toggle, close;
  - the system's dark preference;
  - the clipboard: text and RGBA images, system-wide. A Wayland session passes whole files
    between programs rather than raw pixels, so a picture travels as a PNG there through the
    standard wl-clipboard helpers (`clipboard_png` / `set_clipboard_png`, X4-04); without those
    helpers installed `clipboard_images()` says so and the app keeps its own copy;
  - a monotonic clock.
- **Headless:** `run_headless(app, w, h, scale, ticks, frame)` and `Headless` give the same contract with no display, on a virtual clock. The pixel tests and `--ui-test --headless` use them.

## Sound

`audio::audio_out(rate, channels, fill)`, behind the `audio` feature:
- Plays interleaved f32 samples, which the app writes from the sound thread.
- With no sound device, it fails with a plain message and never panics.
- `has_audio_out()` says whether a device exists.

## Files

| Function | Contract |
|---|---|
| `atomic_write(path, bytes)` | Never leaves a half-written file. The old file survives any failure. |
| `config_dir(app)` | Linux `$XDG_CONFIG_HOME/<app>`, Windows `%APPDATA%\<app>`, macOS `~/Library/Application Support/<app>`. `FOLIO_CONFIG_DIR` overrides it. |
| `home_dir`, `documents_dir`, `places` | What the Open and Save panes offer. |
| `temp_dir`, `temp_file` | Temporary files. |
| `font_dirs` | Where the bundled fonts are. |
| `open_externally`, `print_pdf`, `printers` | Hand a file to the system. |
| `convert_with_office` | Older formats through LibreOffice, when installed. |

## Sockets

`net::listen_any`, `net::listen_local` and `net::connect(addr, timeout)` give blocking TCP streams. `net::local_ip` is the address other computers on the network use; finding it sends no packet.

## Threads and timers

- `spawn(name, f)` starts a named thread; `sleep(d)` waits.
- The UI runs on one thread. Other threads hand results back through channels the app polls on `Tick`.

## The rest

| Function | Contract |
|---|---|
| `user_name()` | Who is signed in, for comments. `FOLIO_USER` overrides it. |
| `random_bytes`, `random_u64` | The system's secure random source. |
| `memory_used`, `memory_peak` | Resident memory, for the budgets in PORTABILITY.md. |
| `setting`, `set_setting` | The few environment overrides Folio reads. |

### Pictures from a scanner or a camera (`capture`)

| Function | Contract |
|---|---|
| `scanners()`, `cameras()`, `devices()` | What is plugged in. Empty is the usual answer; never an error. |
| `have_scanner_support()`, `have_camera_support()` | Whether the system has the tool at all. |
| `scanner_missing()`, `camera_missing()`, `nothing_found()` | What to tell the person when it has not. |
| `scan(device, dpi, colour)`, `snap(device, warmup)` | One picture, as PNG bytes, or a plain message saying why not. |

Linux drives scanners through SANE's `scanimage` and cameras through ffmpeg's `v4l2`, both from the
distribution. Neither is a dependency: when one is absent the app says which package to install and
leaves the picture alone. A port replaces this module with the system's own scanning and camera
services (WIA and Media Foundation on Windows, ImageCaptureCore and AVFoundation on macOS); nothing
above the platform layer changes. `FOLIO_NO_CAPTURE=1` makes a machine look as if it has neither
tool, and `FOLIO_CAPTURE_STUB` points both at one program, which is how the window test proves the
arguments Folio builds without a scanner on the bench.

## Backends

| System | State |
|---|---|
| Linux, X11 and Wayland | Done (winit + softbuffer + arboard, cpal for sound). |
| Windows | Builds and runs, proven once on 6 November and not to be tested again — releases are held to Linux (see PORTABILITY.md rule 5). The workspace cross-builds for `x86_64-pc-windows-gnu` with no code changes, every test passed under wine, and Cork's and Draw's window tests ran start to finish; the files written match the Linux ones byte for byte. To repeat it by hand: `RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=/tmp/cork-win FOLIO_FONTS='Z:\path\to\fonts' cargo test --config tools/win64.toml -Z build-std=std,panic_abort,panic_unwind --target x86_64-pc-windows-gnu --workspace`. What remains: printing (currently "export to PDF"), the conformance suite on a real Windows machine, and an installer that puts the fonts beside the .exe. |
| macOS | Not a target and not to be proven (Product Owner, 2026-11-06). The rules keep it reachable — the same crates cover it and the layer has no per-OS code — but nobody is to spend time on it. |

## The go-between (`--relay`), K3-09

Sharing a board directly needs the person joining to be able to reach the person hosting. On one
network that holds; across two it usually does not, and neither end can do anything about the
other's router. So Folio Cork can be run as a go-between on a computer both ends can reach:

```
folio-cork --relay 7600
```

What it carries, it cannot read. A board's words are sealed end to end with a key made from the
board code; the go-between never sees the code and is never given the key. All it is told is a
room — the SHA-256 of the code, which it cannot turn back into the code — and it copies bytes
between the people in that room. It keeps nothing.

The person hosting opens one connection and says which room they are hosting. Each person joining
opens a connection asking for that room, and is answered with one byte: *here*, or *nobody is
sharing that board*. That byte is also how Folio Cork tells a go-between from a host sharing
directly, so the person joining types one address either way and does not need to know which kind
they were handed. The go-between then tells the host that someone has arrived; the host opens one
more connection for that person, and the two streams are joined. That is the same shape as the
listener a host uses on a local network, so the board itself does not know the difference.

Nothing about this needs an account, a service, or anything outside the two computers and the one
in the middle, which is the Product Owner's own.
