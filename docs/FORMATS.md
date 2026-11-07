# File and wire formats

All text is UTF-8. All binary integers are little-endian. JSON is written by `serde_json`:
- numbers in their shortest exact form;
- object keys in declaration order;
- no byte-order mark.

## Cork drawings: `.vsdx`

- A Visio package (OPC zip, deflate), readable by Visio and LibreOffice:
  - `visio/document.xml`
  - `visio/pages/pageN.xml`
  - masters, `docProps`, `[Content_Types].xml`
  - `_rels`
  - pictures under `visio/media/`
- Cork adds one part, `folio/cork.json`, related from the package root with type `https://folio.studio/relationships/cork`:

```json
{ "format": 1, "pages": ["<hash of visio/pages/page1.xml>", ...], "document": { ...Cork document... } }
```

- **Page hash:** 64-bit FNV-1a over the page part's bytes, written as 16 lower-case hex digits.
  - **Hashes match** (nobody changed the pages in another program): Cork reads `document` and ignores the Visio parts. This round trip is lossless.
  - **Hashes differ:** Cork reads the Visio parts instead.
- **`document`:** the Cork model (`folio-cork/src/model.rs`) as JSON.
  - Colours are `"#RRGGBB"` or `"#RRGGBBAA"`.
  - Positions are page pixels at 96 per inch, as floats.
  - Ids are unsigned 64-bit integers.
  - Pictures are stored once, in `images`, as base64 of the original file bytes, keyed by a content hash.

### What rides in the Visio parts as well

So that another program sees them too, three things are written into the Visio shape sheets, not
only into `folio/cork.json`:

- **Shape data** (K4-02): a `<Section N='Property'>` per shape, one `Row` per property. The row's
  `N` is a plain identifier made from the name; the name as typed is the row's `Label` cell.
- **Hyperlinks** (K4-04): a `<Section N='Hyperlink'>` with one `Row N='Link'`. A file or a web
  address is the `Address` cell; a page of this drawing is the `SubAddress` cell, holding the page
  name. Cork reads both back, and a link never appears among the shape data rows.
- **Linked spreadsheets** (K4-03): the files a drawing takes its shape data from are Cork's own
  business, so they live in `folio/cork.json` under `links` as `{ "path", "key", "match_on" }`.
  `match_on` empty means the shape's text is matched; otherwise it names a property.

## Visio stencils: `.vssx` (read only)

Insert > Import Stencil reads a stencil package (K4-06): `visio/masters/masters.xml` and one part
per master. Each master becomes a named set of shapes in the Shapes pane, measured in page pixels
with its top-left corner at the origin. Cork does not write `.vssx`.

## Diagrams inside other documents: PNG with a `foLK` chunk

Page, Sheet and Show hold a Cork diagram as an ordinary PNG picture (K3-08). Office and other viewers just show the picture.
- The picture is the first page's shapes at 2x, transparent around them.
- One private chunk named `foLK` sits just before `IEND`. Its data is the whole drawing as `.vsdx` bytes, with the usual PNG CRC.
- The name marks it ancillary, private and not safe to copy, so an editor that changes the pixels drops it.
- Cork opens such a `.png` and saves back to it (`files::board_png`, `files::board_in_png`).
- The apps' side is `folio-suite/shared/corkembed.py`. Double-click writes the picture to a temporary `Diagram.png` and starts `folio-cork` on it (or `FOLIO_CORK`). Each change to that file replaces the picture, keeping its place and scale.

## Working together on a board: TCP

Each message is one frame:

| Bytes | Content |
|---|---|
| 4 | `len`, u32 LE: the length of what follows (12 to 268,435,456) |
| 12 | nonce: sender id (u32 LE), then a per-sender counter (u64 LE) |
| len − 12 | ChaCha20-Poly1305 ciphertext and 16-byte tag of the JSON message |

- **Key:** PBKDF2-HMAC-SHA256 of the board code, with 60,000 rounds, salt `folio-cork-board`, and a 32-byte output.
  - The code is first stripped to its letters and digits and upper-cased.
- **Messages:** JSON tagged by `"t"`:
  - `Hello {name}`
  - `Doc {doc, rev, base, from}`: the whole document
  - `Cursor {name, page, x, y}`
  - `Bye {name}`
- A frame that fails to decrypt ends the connection.

## Settings: `config_dir("folio-cork")`

| File | Holds |
|---|---|
| `settings.json` | the Options pane and view choices: theme, author, grid, snapping, rulers, minimap, panes, recent files |
| `my-shapes.json` | the user's saved shapes, as Cork shapes; each one may name the stencil it came from in `group`, which is how imported `.vssx` stencils appear in the pane (K4-06) |
| `clipboard.json` | the last copied shapes, keyed to the picture placed on the clipboard (`hash`: 64-bit FNV-1a over every 7th byte of the picture's RGBA data, XOR the data length, plus `w` and `h`), so pasting that picture back gives editable shapes |
| `clipboard.png` | the same copy as a diagram picture (above); Page, Sheet and Show paste it instead of the clipboard picture when the two match in size and pixels |

All of them are written with `platform::atomic_write`.
