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
| `my-shapes.json` | the user's saved shapes, as Cork shapes |
| `clipboard.json` | the last copied shapes, keyed to the picture placed on the clipboard (`hash`: 64-bit FNV-1a over every 7th byte of the picture's RGBA data, XOR the data length, plus `w` and `h`), so pasting that picture back gives editable shapes |

All three are written with `platform::atomic_write`.
