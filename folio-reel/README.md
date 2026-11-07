# Folio Reel on the portable base

The video editor, rewritten in Rust on the suite's own base: no GStreamer, no
GES, no GTK. The project, the edits, the timeline and the window are ours.

Reading and writing video is not. H.264 and AAC are patented formats with
decades of engineering in their encoders, and writing our own would be neither
honest nor safe. So Reel hands each file to **ffmpeg** when the computer has
one, and says so plainly when it has not. ffmpeg is not shipped in the package;
it is used the way any program on the computer is used.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-reel-engine` | The project (the bin, four tracks, clips, markers, groups), every edit with its undo step, the `.reel` file, and the ffmpeg command an export runs. |
| `crates/folio-platform` | `media`: the one place that starts ffmpeg — probe a file, pull a frame, read a waveform, run an export and watch it get on. |
| `crates/folio-kit` | The window furniture: the ribbon, the title bar, dialogs, menus, the file pane, text and icons. |
| `folio-reel` | The window: the media bin, the preview, the four-track timeline and the properties pane. |

The engine builds the ffmpeg command as plain strings and never runs anything,
so the whole graph — scaling, crops, colour, green screen, pan-and-zoom, fades,
crossfades, wipes, volume curves, the music bed, the mix — is tested on a
computer with no ffmpeg on it at all.

## Running it

    folio-reel [FILE.reel]                   open a film
    folio-reel --export OUT.mp4 IN.reel      write it out (--preset NAME)
    folio-reel --frame OUT.png IN.reel [AT]  one frame as a picture
    folio-reel --info IN.reel                what is on the timeline
    folio-reel --probe FILE                  what a media file is
    folio-reel --presets                     the sizes it writes
    folio-reel --ui-test [DIR]               drive the window as a person would
    folio-reel --ui-test DIR --headless      the same, in memory

## What works

Four tracks: Titles, Video, Audio and Music. A media bin that takes video,
sound and pictures. Clips that drag between tracks, trim by either edge, split
at the playhead, delete with or without closing the gap, group and ungroup,
copy and paste. Snapping to clip edges, markers and the playhead. Crossfades,
fades to black, and the six wipes, boxes, clocks and irises. Speed from a
quarter to four times. Volume, fades, volume points, mute, and a music bed that
dips under the voice on the Audio track. Green screen, crop, brightness,
contrast, saturation, zoom and pan-and-zoom. Titles, lower thirds and captions,
with subtitles imported from SRT or WebVTT. Markers in six colours. Undo and
redo throughout. The preview shows the frame under the playhead and plays the
timeline's own sound. Exports to MP4 and WebM at 720p, 1080p and 4K, with the
progress shown as it goes.

The `.reel` file is the same JSON the earlier Reel wrote, so a project made
there opens here, and anything this build does not know about is kept when it
saves.

Not in this version yet: video while playing is the frame under the playhead
rather than a running picture (the sound plays in time, and an export is exact);
proxies for large files; the waveform on every clip rather than per file; and
chroma key fine-tuning in the window.

## When there is no ffmpeg

Reel does not pretend. The bin refuses the file and says what to install, the
status line says so, the preview says so, and an export stops before it starts.
Everything that does not need a decoder — the project, the edits, the timeline,
saving and opening — works as it always does.
