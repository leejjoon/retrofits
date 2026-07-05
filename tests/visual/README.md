# Visual regression tests (real graphics protocols)

`run.sh` verifies that opening and closing every UI panel leaves the screen
**pixel-identical** under real terminal graphics protocols — the regression
test for the non-overlapping panel layout that replaced the old Kitty/Sixel
redraw workarounds (see `DEVELOP.md`).

`cargo test` cannot cover this: the artifacts only exist in the interaction
between ratatui's cell model and the out-of-band graphics escape sequences a
real emulator renders. So this harness runs **actual terminal emulators**
headlessly:

| Protocol | Emulator | Key injection |
| :--- | :--- | :--- |
| Kitty | `kitty` | built-in remote control (`kitten @ send-text`) |
| Sixel | `konsole` | DBus (`org.kde.konsole.Session.sendText`) |

Both run inside an Xvfb virtual display; screenshots are taken from the real
framebuffer with ImageMagick `import` and compared with `compare -metric AE`.

## Running

```bash
tests/visual/run.sh
```

Requirements: `Xvfb`, ImageMagick, and `kitty` and/or `konsole` (a missing
emulator is skipped, not failed). Builds `target/release/retrofits` if
needed. Exit code 0 = pass, 1 = artifact detected, 2 = missing dependency.

## What it does per protocol

1. Launch retrofits on `example_fits/18109J000.fits` (override with
   `RETROFITS_TEST_FILE=/path/to.fits`).
2. Press `R` once and screenshot a **baseline** (the force-clear absorbs
   startup quirks — e.g. konsole+sixel scrolls the pre-first-frame draw up
   one row when the first sixel image lands).
3. For each panel — summary `w`, extension picker `e`, manual cut `m`,
   protocol picker `P`, help `?`, header viewer `v`:
   - open it, screenshot, and require a nonzero diff vs the baseline
     (positive control: proves the key was actually received);
   - close it with `q`, screenshot, and require **0 differing pixels**
     vs the baseline.

Screenshots land in `tests/visual/output/` (gitignored) and are kept on
failure for inspection.

## Caveats

- kitty must be forced onto X11 (`linux_display_server=x11`, with
  `WAYLAND_DISPLAY` unset) or it will open on your real desktop session.
- The konsole DBus session path is assumed to be `/Sessions/1`, which holds
  for a freshly launched instance.
- Timings are sleep-based; on a very slow machine, bump the `sleep` values.
