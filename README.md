# RetroFITS

A high-performance FITS image viewer for the terminal.

RetroFITS is designed to work with modern terminal emulators, utilizing memory-mapping and zero-copy architectures to handle large astronomical data files efficiently.

## Screenshots

### Halfblocks Protocol

![Halfblocks Protocol](assets/halfblocks.gif)

### Kitty Protocol (Ghostty)

![Kitty Protocol in Ghostty](assets/screenshot_retrofits_ghostty_w_kitty.png)

### Sixel Protocol (Konsole)

![Sixel Protocol in Konsole](assets/screenshot_retrofits_konsole_w_sixel.png)

## Features

- High-performance FITS viewing directly in the terminal
- Support for various rendering protocols (Kitty, iTerm2, Sixel, Halfblocks)
- Interactive exploration (zoom, pan, adjust scaling)
- Remote SSH support

## Installation

### Static Binaries (Linux)

For Linux x86_64 users, fully standalone static binaries are available on the [GitHub Releases](https://github.com/leejjoon/retrofits/releases) page. These binaries have zero runtime dependencies.

For other platforms or detailed build instructions, see [INSTALL.md](INSTALL.md).

## Usage

```bash
retrofits path/to/your/image.fits
```

By default, RetroFITS will use the `halfblocks` protocol for maximum compatibility. You can force a specific protocol using the `--protocol` flag:

```bash
retrofits --protocol kitty image.fits
retrofits --protocol sixel image.fits
```

## Keyboard Shortcuts

### General
- `q`, `Esc`: Quit application
- `h`: Show help window
- `w`: Show FITS header summary
- `e`: Select FITS extension (for multi-extension files)
- `R`: Force full screen redraw (useful for clearing artifacts)

### Navigation
- `Arrow Keys` or `h/j/k/l`: Pan image
- `+`, `i`: Zoom in
- `-`, `o`: Zoom out
- `r`: Reset zoom and pan

### Image Adjustment
- `c`: Cycle through color maps
- `s`: Cycle through stretch functions (Linear, Log, Asinh)
- `z`: Cycle through cut modes (MinMax, ZScale, Custom)
- `m`: Manually enter black/white point values

### Protocol Switching
- `p`: Cycle through rendering protocols
- `H`: Switch to Halfblocks
- `S`: Switch to Sixel
- `K`: Switch to Kitty
- `I`: Switch to iTerm2

### Sixel Artifact Workaround

When using the Sixel protocol, opening and closing UI popups (like the Help or Summary windows) may leave lingering graphical artifacts. By default, RetroFITS forces a full screen clear to fix this caching issue on Sixel.

If your terminal correctly clears the Sixel image under the popup without issues and you experience flickering with this workaround, you can disable it via a flag or environment variable:

```bash
retrofits --disable-sixel-clear image.fits
# or via environment variable
RETROFITS_DISABLE_SIXEL_CLEAR=1 retrofits image.fits
```
