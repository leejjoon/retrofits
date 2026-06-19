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
                  ui.rs        image widget + status bar + popups (ratatui)
```

`app.rs` holds all interactive state and routes keyboard input; `main.rs` wires
up the CLI, terminal, protocol detection, and the event loop.

---

## Module map (`src/`)

| File | Lines | Responsibility |
| :--- | ----: | :--- |
| `main.rs` | 159 | CLI parsing (`clap`), terminal setup, protocol detection/override, 5 ms-poll event loop. |
| `lib.rs` | 7 | Module declarations (exposes everything for the integration tests). |
| `app.rs` | 468 | Central `App` state + all keyboard handling and input modes. |
| `fits.rs` | 466 | FITS parsing via `fitsrs`; extracts pixel data into `Array2<f32>`. |
| `render.rs` | 139 | Background `RenderThread`: viewport crop, stretch+colormap, encode. |
| `ui.rs` | 385 | ratatui rendering: image area, status bar, popups. |
| `stretch.rs` | 125 | Linear / Logarithmic / Asinh stretch (rayon-parallel). |
| `colormap.rs` | 124 | Maps `[0,1]` → RGBA using `colorous` scientific colormaps. |
| `zscale.rs` | 46 | IRAF-style ZScale cut estimation. |

### `main.rs` — entry point
- CLI flags: positional `file`, `--protocol`, `--ext`, `--disable-sixel-clear`
  (also reads `RETROFITS_DISABLE_SIXEL_CLEAR`).
- Queries the terminal for graphics capability via `Picker::from_query_stdio()`.
- **Default protocol**: Kitty when running under Ghostty (detected via
  `TERM_PROGRAM`/`TERM`), otherwise Halfblocks for maximum compatibility. An
  explicit `--protocol` overrides this.
- Event loop (`run_app`) polls events every 5 ms, pulls finished frames from the
  render thread, and only redraws when something changed. Honors
  `clear_screen_next_frame` (the Sixel workaround) by calling `terminal.clear()`
  before the next draw.

### `app.rs` — state & input
- `App` struct holds: the `Arc<FitsImage>`, stretch/colormap, black/white points,
  zoom + center (pan), terminal size, cut mode, the active `StatefulProtocol`,
  the `RenderThread`, and assorted flags.
- `InputMode` enum drives modal key handling: `Normal`, `EditingBlackPoint`,
  `EditingWhitePoint`, `Summary`, `Help { scroll }`,
  `SelectingExtension { selected }`.
- `CutMode`: `MinMax`, `ZScale`, `Custom`. `apply_cut_mode()` recomputes
  black/white points and queues a render.
- `queue_render()` / `queue_render_with_fits()` build a `RenderRequest` and hand
  it to the render thread. `try_update_protocol()` swaps in the newest finished
  frame.
- Keyboard handlers per mode mirror the README shortcut table (pan, zoom,
  cycle stretch/colormap/cut, protocol switching `p`/`H`/`S`/`K`/`I`, manual
  cut entry `m`, summary `w`, help `h`, extension picker `e`, force redraw `R`).

### `fits.rs` — parsing
- `load_fits(path, ext_arg)` returns a `FitsImage { header, data, width, height,
  extensions, current_extension, file_path }`.
- Iterates **all HDUs once** to build the `extensions` list and choose a target
  (by index, by `EXTNAME`, or the first image HDU), then **reopens the file** to
  read the chosen HDU's pixels (the iterator was consumed).
- Normalizes every BITPIX type (`U8/I16/I32/I64/F32/F64`) to `f32`, applying
  `BZERO`/`BSCALE` if present.
- **Flips rows vertically** because FITS stores bottom-to-top while we render
  top-to-bottom.
- Inline unit tests load `example_fits/18109J000.fits`.

### `render.rs` — background rendering
- `RenderThread` spawns a worker fed by an MPSC channel. It **debounces** input
  by draining the queue to the latest `RenderRequest` before working — important
  for responsiveness under rapid key presses.
- `process_frame()` computes the visible crop from zoom/pan/center and terminal
  font size, slices the `ndarray`, runs `compute_stretch` + `apply_colormap`,
  then encodes via `picker.new_resize_protocol()`.

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

- Inline `#[cfg(test)]` modules in `fits.rs`, `stretch.rs`, `colormap.rs`.
- Integration tests in `tests/`: `pipeline_tests.rs`, `render_tests.rs`,
  `viewport_tests.rs`, `tui_tests.rs`, `zscale_tests.rs`.
- Run with `cargo test`.

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
- `DEVELOP.md` — write-ups of the **Kitty** and **Sixel** popup-artifact bugs
  and their fixes (forced `queue_render()` on popup close; `clear_screen_next_frame`
  flag for Sixel, toggleable via `--disable-sixel-clear`).
- `MAINTAINER.md` — release process.
- `references/retrofits_prd.md` — original PRD / design rationale.

---

## Roadmap

- **Phase 1 (done):** core rendering, interactivity, protocols, stretch,
  colormaps, multi-extension handling.
- **Phase 2 (deferred, not implemented):** WCS astrometry — real-time RA/Dec
  readout in the status bar from FITS WCS headers (`CRPIX`/`CRVAL`/`CDELT`/`CTYPE`).

---

## Caveats / known rough edges

- **`fits.rs` duplication:** the `Primary` and `XImage` HDU arms are ~130
  near-identical lines each (header extraction, pixel decode, row-flip). A shared
  helper would roughly halve the file.
- **"Zero-copy memmap" not realized:** the PRD describes a memory-mapped,
  zero-copy architecture, but the implementation reads pixels into an owned
  `Vec<f32>` through a `BufReader`. `memmap2` is a dependency but unused in the
  data path.
- **Protocol popup artifacts:** Kitty and Sixel both need explicit redraw
  workarounds (documented in `DEVELOP.md`); behavior can vary by terminal
  emulator.
- **Non-contiguous colormap views** hit `unimplemented!` — fine in practice
  because the viewport slice is materialized contiguous, but a latent panic.
