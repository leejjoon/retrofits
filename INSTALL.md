# Installation Guide for RetroFITS

RetroFITS is a high-performance FITS image viewer designed for modern terminal emulators. It utilizes memory-mapping and zero-copy architectures to handle large astronomical data files efficiently.

## Prerequisites

To build and run RetroFITS, you will need:

1.  **Rust Toolchain:**
    -   RetroFITS is written in Rust. You need the latest stable version of Rust and Cargo.
    -   If you don't have Rust installed, you can get it from [rustup.rs](https://rustup.rs/):
        ```bash
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        ```

2.  **Terminal Emulator with Graphics Support:**
    -   While RetroFITS can fall back to Unicode half-blocks, for the best scientific analysis experience, a terminal that supports high-resolution graphics protocols is highly recommended.
    -   **Kitty Graphics Protocol (Best):** [Kitty](https://sw.kovidgoyal.net/kitty/), [WezTerm](https://wezfurlong.org/wezterm/), [Ghostty](https://ghostty.org/), [Konsole](https://konsole.kde.org/).
    -   **iTerm2 Inline Images:** [iTerm2](https://iterm2.com/), [WezTerm](https://wezfurlong.org/wezterm/), [Konsole](https://konsole.kde.org/).
    -   **Sixel Protocol:** [Foot](https://codeberg.org/dnkl/foot), [Konsole](https://konsole.kde.org/), [Xterm](https://invisible-island.net/xterm/), [Contour](https://github.com/contour-terminal/contour).

## Building from Source

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/your-username/retrofits.git
    cd retrofits
    ```

2.  **Build the release binary:**
    ```bash
    cargo build --release
    ```

3.  **Run RetroFITS:**
    The compiled binary will be located at `target/release/retrofits`.
    ```bash
    ./target/release/retrofits path/to/your/image.fits
    ```

## Installing to your System

To install RetroFITS to your local `~/.cargo/bin` directory (ensure this is in your `PATH`):

```bash
cargo install --path .
```

After installation, you can run it from anywhere:

```bash
retrofits my_observation.fits
```

## Protocol Selection

RetroFITS automatically detects your terminal's capabilities. If you need to force a specific rendering protocol, you can use the `--protocol` flag:

```bash
# Force Sixel rendering
retrofits --protocol sixel image.fits

# Force Unicode half-blocks (compatible with almost all terminals)
retrofits --protocol halfblocks image.fits
```

## Remote SSH Usage

RetroFITS is designed to work over SSH. It uses in-band data transmission, meaning no special ports are required. For the best performance over remote connections:

-   Ensure your local terminal emulator supports one of the protocols listed above.
-   If using `tmux`, RetroFITS will automatically handle escape sequence chunking to ensure compatibility.
