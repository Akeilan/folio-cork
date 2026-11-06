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
  - the clipboard: text and RGBA images, system-wide;
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

## Backends

| System | State |
|---|---|
| Linux, X11 and Wayland | Done (winit + softbuffer + arboard, cpal for sound). |
| Windows | Builds and runs. `tools/release.sh` with `PORTS=1` cross-builds the workspace for `x86_64-pc-windows-gnu` and runs every test, Cork's window test and Draw's under wine: all pass, and the files written match the Linux ones byte for byte. What remains: printing (currently "export to PDF"), the conformance suite on a real Windows machine rather than wine, and an installer that puts the fonts beside the .exe. |
| macOS | After Windows. The same crates cover it. Needs a real Mac for the conformance suite. |
