# RetroFITS — Codebase Guide

A high-performance **FITS image viewer for the terminal**, written in Rust. It
renders astronomical `.fits` images directly in the terminal using modern
graphics protocols (Kitty / iTerm2 / Sixel) with a Unicode half-blocks fallback,
and provides interactive zoom / pan / stretch / colormap controls via a
[`ratatui`](https://ratatui.rs) TUI.

> Status format note: file path references below use `path:line` and are
> clickable in most editors/terminals.

---

## Quick mental model

The core flow is a **load → stretch → colormap → encode → display** pipeline:

```
.fits file
   │  fits.rs        parse HDUs, normalize pixels to f32, flip rows
   ▼
Array2<f32>  ──►  stretch.rs   Linear / Log / Asinh  →  [0,1]
                      │        (cut points from MinMax / ZScale / Custom)
                      ▼
                  colormap.rs  Grayscale / Viridis / Plasma / Inferno / Magma  →  RGBA
                      │
                      ▼
                  render.rs    crop viewport (zoom/pan), encode via ratatui-image
                      │        (runs on a debounced background thread)
                      ▼
                  ui.rs        image widget + status bar + side/bottom panels
                               + full-screen pages (ratatui)
```

`app.rs` holds all interactive state and routes keyboard/mouse input through
the `keymap.rs` binding table; `main.rs` wires up the CLI, terminal, protocol
detection, and the event loop.

---

## Module map (`src/`)

| File | Lines | Responsibility |
| :--- | ----: | :--- |
| `main.rs` | ~190 | CLI parsing (`clap`), terminal setup, protocol detection/override, 5 ms-poll event loop, mouse-capture sync. |
| `lib.rs` | 8 | Module declarations (exposes everything for the integration tests). |
| `app.rs` | ~1060 | Central `App` state, `Action` dispatch, all per-mode key handlers, mouse handling, status messages. |
| `keymap.rs` | ~370 | `Action` enum + `KEYMAP` table: single source of truth for normal-mode bindings; drives both dispatch and the generated help. |
| `fits.rs` | ~520 | FITS parsing via `fitsrs`; ordered full-fidelity header (`FitsHeader`), pixel data as `Array2<f32>`, extension metadata. |
| `render.rs` | ~260 | Background `RenderThread`: shared `viewport()` math, stretch+colormap, crosshair compositing, encode. |
| `ui.rs` | ~730 | ratatui rendering: image area, status bar, side/bottom panels, full-screen pages (help, header viewer). |
| `stretch.rs` | 135 | Linear / Logarithmic / Asinh stretch (rayon-parallel). |
| `colormap.rs` | 136 | Maps `[0,1]` → RGBA using `colorous` scientific colormaps. |
| `zscale.rs` | 225 | IRAF-style ZScale cut estimation. |

### `main.rs` — entry point
- CLI flags: positional `file`, `--protocol`, `--ext`, `--mouse`.
- Queries the terminal for graphics capability via `Picker::from_query_stdio()`.
- **Default protocol**: Kitty when running under Ghostty (detected via
  `TERM_PROGRAM`/`TERM`), otherwise Halfblocks for maximum compatibility. An
  explicit `--protocol` overrides this.
- Event loop (`run_app`) polls events every 5 ms, pulls finished frames from the
  render thread, expires transient status messages, syncs terminal mouse
  capture to `app.mouse_enabled`, and only redraws when something changed.
  `clear_screen_next_frame` (set by the `R` key) forces a `terminal.clear()`
  before the next draw as an artifact escape hatch.

### `keymap.rs` — bindings as data
- `Action` enum decouples what happens from which key triggers it.
- `KEYMAP: &[Binding]` lists keys (+ modifiers), action, category, help text
  and a `global` flag (allowed while the summary panel is open).
- `lookup(KeyEvent)` resolves an event (SHIFT is stripped for `Char` keys and
  matched exactly for arrows, enabling `Shift+Arrow` bindings).
- The help page content is generated from this table, so help and actual
  bindings cannot drift apart.

### `app.rs` — state & input
- `App` struct holds: the `Arc<FitsImage>`, stretch/colormap, black/white points,
  zoom + center (pan), terminal size + font size, cut mode, the active
  `StatefulProtocol`, the `RenderThread`, transient `StatusMessage`, mouse
  state, and the rects last drawn (for mouse hit-testing).
- `InputMode` enum drives modal key handling: `Normal`, `EditingBlackPoint`,
  `EditingWhitePoint`, `Summary`, `Help { scroll }`,
  `HeaderView { scroll, search }`, `SelectingExtension { state }`,
  `SelectingProtocol { state }`, `Crosshair { pos }`.
- Normal-mode keys dispatch through `keymap::lookup` → `App::dispatch(Action)`.
  `close_mode()` centralizes returning to `Normal` (+ re-render).
- `notify(Severity, text)` shows a vim-style transient status message
  (Info 2.5 s, Warn/Error 5 s); consumed for bad input, extension load
  failures, protocol switches, etc.
- Vim-style navigation: `h/j/k/l`/arrows fine pan (1/8 view), `H/J/K/L`/
  Shift+arrows coarse (1/2), zoom floor 0.1 (below 1x the image shrinks),
  `Esc` never quits (only `q` does).
- `handle_mouse()` (opt-in): scroll = zoom at cursor, drag = pan, click =
  select/activate picker rows, drag in crosshair mode moves the crosshair.

### `fits.rs` — parsing
- `load_fits(path, ext_arg)` returns a `FitsImage { header, data, width, height,
  extensions, current_extension, file_path }`.
- `header` is a `FitsHeader(Vec<HeaderEntry>)`: **every card in file order**,
  preserving values, per-card comments, `COMMENT`/`HISTORY` cards and blank
  lines; long-string `CONTINUE` cards are merged. `FitsHeader::get(kw)` does a
  linear lookup.
- `extensions: Vec<ExtensionInfo>` carries index, `EXTNAME`, image flag,
  HDU kind (IMAGE/BINTABLE/TABLE), NAXIS dims and pixel type for the picker.
- Iterates **all HDUs once** to build the `extensions` list and choose a target
  (by index, by `EXTNAME`, or the first image HDU), then **reopens the file** to
  read the chosen HDU's pixels (the iterator was consumed).
- Normalizes every BITPIX type (`U8/I16/I32/I64/F32/F64`) to `f32`, applying
  `BZERO`/`BSCALE` if present.
- **Flips rows vertically** because FITS stores bottom-to-top while we render
  top-to-bottom (pinned by a test against astropy reference pixel values).
- Inline unit tests load `example_fits/18109J000.fits`.

### `render.rs` — background rendering
- `RenderThread` spawns a worker fed by an MPSC channel. It **debounces** input
  by draining the queue to the latest `RenderRequest` before working — important
  for responsiveness under rapid key presses. A fits swap carried by a drained
  request is still applied.
- `viewport()` computes the visible crop from zoom/center/terminal geometry;
  it is shared with `app.rs` (crosshair positioning, mouse hit-testing) so the
  two can never disagree.
- `process_frame()` slices the `ndarray`, runs `compute_stretch` +
  `apply_colormap`, composites the crosshair into the RGBA if active (per-axis,
  protocol-aware thickness), then encodes via `picker.new_resize_protocol()`.

### `ui.rs` — layout
- **Panels never overlay the image** (that was the root cause of the old
  Kitty/Sixel artifacts; see DEVELOP.md). Help and the header viewer are
  full-screen pages; summary/extension/protocol pickers are a right side
  panel; manual cut entry is a bottom strip. The image rect shrinks and the
  size change triggers a natural re-encode.
- Below 1x zoom the image is drawn into a centered sub-rect scaled by zoom.
- Status bar: three segments (file/ext/dims | zoom·stretch·colormap·cuts |
  protocol + hints); the middle segment is replaced by the crosshair readout
  or a transient severity-colored message when active.
- The header viewer (`v`) is a `less`-style page: scrolling, `/` incremental
  search with highlighted matches, `n`/`N` navigation.

### `stretch.rs`, `colormap.rs`, `zscale.rs`
- `stretch.rs`: normalizes to `[0,1]`, clamps, then applies the chosen curve
  (Log/Asinh use intensity `a = 1000`). Parallelized with `ndarray::Zip` +
  rayon. `auto_stretch_params` is a simple min / 99%-of-range heuristic.
- `colormap.rs`: parallel (`par_chunks_exact_mut`) RGBA fill using `colorous`
  maps. Requires a contiguous array view (`unimplemented!` otherwise).
- `zscale.rs`: IRAF ZScale — samples up to 10 000 pixels, fits a line to the
  sorted sample with iterative k-sigma rejection, and sets the window from the
  fitted slope near the median divided by `contrast` (IRAF default 0.25). Skips
  non-finite pixels. Outlier rejection keeps the faint background visible.

---

## Dependencies (`Cargo.toml`)

- `fitsrs` — FITS parsing (multi-HDU, many BITPIX types).
- `ndarray` (with `rayon`) — 2D pixel arrays + parallel ops.
- `image` — RGBA buffers handed to the renderer.
- `colorous` — scientific colormaps.
- `rayon` — data parallelism.
- `ratatui` + `ratatui-image` (`chafa-static` feature) + `crossterm` — TUI and
  terminal graphics protocols.
- `clap` (derive + env) — CLI.
- `anyhow` — error handling.
- `memmap2` — **declared but currently unused** in the data path (see caveats).
- Dev: `assert_cmd`, `predicates`, `tempfile`.

---

## Tests

- Inline `#[cfg(test)]` modules in `fits.rs`, `stretch.rs`, `colormap.rs`,
  `app.rs`, `ui.rs`.
- Integration tests in `tests/`: `pipeline_tests.rs`, `render_tests.rs`,
  `viewport_tests.rs`, `tui_tests.rs`, `zscale_tests.rs`.
- Run with `cargo test`.
- **Visual regression** (`tests/visual/run.sh`, not part of `cargo test`):
  drives real kitty and konsole emulators headlessly under Xvfb and
  pixel-diffs screenshots to prove panel open/close cycles leave no
  graphics-protocol artifacts. Requires Xvfb + ImageMagick.

---

## Build & distribution

- Standard: `cargo build --release` → `target/release/retrofits`.
- Static Linux binary: `Dockerfile.static` produces a fully standalone musl
  binary (`retrofits-static`, ~9.8 MB, zero runtime deps).
- `.github/workflows/release.yml` automates static-binary releases; process
  documented in `MAINTAINER.md`.
- Demo GIFs generated via `generate_gifs.sh` + `*.tape` (vhs) files; assets in
  `assets/`.

---

## Docs in the repo

- `README.md` — features, install, usage, full keyboard-shortcut table.
- `INSTALL.md` — build/prereqs (incl. `libchafa`) and an AI-agent quick start.
- `DEVELOP.md` — the non-overlapping panel layout rationale, plus historical
  write-ups of the **Kitty** and **Sixel** popup-artifact bugs that the layout
  made obsolete.
- `MAINTAINER.md` — release process.
- `references/retrofits_prd.md` — original PRD / design rationale.

---

## Roadmap

- **Phase 1 (done):** core rendering, interactivity, protocols, stretch,
  colormaps, multi-extension handling.
- **UI overhaul (done):** keymap-as-data, non-overlapping panels (workarounds
  removed), header viewer with search, extension picker polish, vim-style
  keys, zoom-out below fit, status messages, crosshair pixel readout,
  opt-in mouse support.
- **Phase 2 (deferred, not implemented):** WCS astrometry — real-time RA/Dec
  readout in the status bar from FITS WCS headers (`CRPIX`/`CRVAL`/`CDELT`/`CTYPE`).
  The crosshair readout is the natural surface for this.

---

## Caveats / known rough edges

- **"Zero-copy memmap" not realized:** the PRD describes a memory-mapped,
  zero-copy architecture, but the implementation reads pixels into an owned
  `Vec<f32>` through a `BufReader`. `memmap2` is a dependency but unused in the
  data path.
- **No extension caching:** switching extensions re-reads the file from disk
  each time (deliberate — a 4k×4k f32 frame is ~64 MB of RAM per cached entry).
- **Non-contiguous colormap views** hit `unimplemented!` — fine in practice
  because the viewport slice is materialized contiguous, but a latent panic.
- **3D+ image HDUs** are listed as viewable but fail to load (pixel-count
  mismatch); the failure now surfaces as a status-bar error rather than
  silently doing nothing.
