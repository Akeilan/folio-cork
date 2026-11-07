# Folio Chord on the portable base

The music player, rewritten on the suite's own base: no GTK, no GStreamer, no
Python. It reads your music folders, keeps the library in a file of its own, and
decodes and plays the songs itself.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-chord-engine` | The library and its file, playlists and Smart Playlists, the tags, and playback: decoding, resampling, the equaliser, Sound Check and crossfade. |
| `crates/folio-kit` | The window furniture: the title bar, dialogs, menus, the file pane, text and icons. |
| `crates/folio-platform` | The window, and the sound output. The only place the system is touched. |
| `folio-chord` | The window: the player bar, the sources, the column browser and the song list. |

Decoding is the `symphonia` crates — pure Rust, the same on every machine. The
samples go through the gain, the ten-band equaliser and the volume in Chord's
own code, and the platform layer hands them to the sound card.

## Running it

    folio-chord [FOLDER]          open the window, taking in a folder of music
    folio-chord --scan FOLDER     take a folder in without opening a window
    folio-chord --ui-test [DIR]   drive the window as a person would
    folio-chord --ui-test DIR --headless    the same, in memory

## What works

Music folders watched and rescanned, with the tags read from MP3, AAC, Apple
Lossless, FLAC, Ogg Vorbis, WAV and AIFF. The song list with sortable columns,
the Genres/Artists/Albums browser, search, ratings by the stars, play counts and
Get Info. Playlists and Smart Playlists, with the six iTunes started with. Play,
pause, next, previous, seeking, shuffle, repeat one or all, the volume, Sound
Check from the song's own ReplayGain, crossfade and the ten-band equaliser. The
library is kept in one readable file and comes back as it was left.

Not in this version yet: writing tags back into the files, internet radio,
podcasts, Home Sharing, network speakers, CDs, the visualizer and the Mini
Player, and Opus (nothing here decodes it yet — Chord says so and moves on).

## Testing it

    cargo test -p folio-chord-engine        the library, the rules and the decoder
    folio-chord --ui-test DIR               the window test, on X or Wayland
    folio-chord --ui-test DIR --headless    the same without a display
