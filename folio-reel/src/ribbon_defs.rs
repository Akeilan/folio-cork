//! Folio Reel's ribbon tabs.

use folio_kit::ribbon::{Group, Item as I, Tab};

pub fn tabs() -> Vec<Tab> {
    vec![
        Tab::new(
            "home",
            "Home",
            "H",
            vec![
                Group::new(
                    "Clipboard",
                    vec![
                        I::large("paste", "Paste", "paste").key("Ctrl+V"),
                        I::small("cut", "Cut", "cut").key("Ctrl+X"),
                        I::small("copy", "Copy", "copy").key("Ctrl+C"),
                    ],
                ),
                Group::new(
                    "Media",
                    vec![
                        I::large("add-media", "Add Media", "add-media").desc("Put video, pictures or sound in the bin.").key("Ctrl+I"),
                        I::small("to-video", "To Video", "track-video"),
                        I::small("to-audio", "To Audio", "track-audio"),
                    ],
                ),
                Group::rows(
                    "Edit",
                    vec![
                        vec![
                            I::icon("split", "Split at the playhead", "split").key("S"),
                            I::icon("delete", "Delete the clip", "delete").key("Delete"),
                            I::icon("ripple-delete", "Delete and close the gap", "ripple").key("Shift+Delete"),
                        ],
                        vec![
                            I::icon("trim-start", "Trim the start to the playhead", "trim-start"),
                            I::icon("trim-end", "Trim the end to the playhead", "trim-end"),
                            I::icon("group", "Group the clips", "group").key("Ctrl+G"),
                        ],
                    ],
                ),
                Group::rows(
                    "Titles",
                    vec![
                        vec![I::icon("title", "Title", "title"), I::icon("lower-third", "Lower third", "lower-third"), I::icon("caption", "Caption", "caption")],
                        vec![I::icon("subtitles", "Import subtitles", "subtitles")],
                    ],
                ),
                Group::rows(
                    "Playhead",
                    vec![
                        vec![I::icon("play", "Play or pause", "play").key("Space"), I::icon("home", "To the start", "to-start"), I::icon("end", "To the end", "to-end")],
                        vec![I::icon("marker", "Put a marker here", "marker").key("M"), I::icon("next-marker", "Next marker", "next-marker"), I::icon("prev-marker", "Previous marker", "prev-marker")],
                    ],
                ),
            ],
        ),
        Tab::new(
            "clip",
            "Clip",
            "C",
            vec![
                Group::new(
                    "Sound",
                    vec![
                        I::combo("volume", "Volume", 92),
                        I::small("mute", "Mute", "mute"),
                        I::small("music-bed", "Music Bed", "music-bed").desc("Dip the music under the voice."),
                    ],
                ),
                Group::new("Fades", vec![I::combo("fade-in", "Fade in", 92), I::combo("fade-out", "Fade out", 92)]),
                Group::new("Speed", vec![I::combo("speed", "Speed", 92)]),
                Group::new(
                    "Picture",
                    vec![
                        I::combo("transition", "Transition", 140),
                        I::small("panzoom", "Pan and Zoom", "panzoom").menu(),
                        I::small("chroma", "Green Screen", "chroma"),
                    ],
                ),
                Group::new("Colour", vec![I::combo("brightness", "Brightness", 92), I::combo("contrast", "Contrast", 92), I::combo("saturation", "Saturation", 92)]),
            ],
        ),
        Tab::new(
            "export",
            "Export",
            "X",
            vec![
                Group::new(
                    "Video",
                    vec![
                        I::large("export", "Export Video", "export").desc("Write the film out as one file.").key("Ctrl+E"),
                        I::small("export-preset", "Choose a size", "size").menu(),
                        I::small("poster", "Save a Frame", "poster"),
                    ],
                ),
                Group::new("Project", vec![I::combo("project-size", "Picture size", 150), I::combo("project-fps", "Frames a second", 110)]),
            ],
        ),
        Tab::new(
            "view",
            "View",
            "V",
            vec![
                Group::rows(
                    "Timeline",
                    vec![
                        vec![I::icon("zoom-in", "Closer in", "zoom-in"), I::icon("zoom-out", "Further out", "zoom-out"), I::icon("zoom-fit", "Fit the whole film", "zoom-fit")],
                        vec![I::icon("snapping", "Snap to edges", "snap"), I::icon("waveforms", "Show waveforms", "wave")],
                    ],
                ),
                Group::new("Panes", vec![I::small("show-bin", "Media Bin", "bin"), I::small("show-props", "Properties", "props"), I::small("dark", "Dark", "dark")]),
            ],
        ),
    ]
}
