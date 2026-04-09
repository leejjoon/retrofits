# **Product Requirements Document (PRD): RetroFITS**

## **1\. Product Overview and Architecture Strategy**

The intersection of astronomical data processing and modern command-line interface (CLI) development presents a highly specialized architectural domain. Flexible Image Transport System (FITS) files, the established standard format for raw astrophysical data, are heavily structured, uncompressed, and frequently massive, designed to store high-dynamic-range (HDR) scientific arrays rather than consumer-friendly visual formats.1 Simultaneously, the Linux terminal ecosystem—historically constrained to monospaced text grids—has recently evolved to support sophisticated, high-resolution pixel rendering through advanced terminal graphics protocols.3

Developing RetroFITS, a high-performance, Rust-based FITS viewer for the Linux terminal, requires harmonizing these two entirely different computing environments. While existing terminal image viewers such as viu and timg demonstrate the basic plausibility of rendering graphics in the shell, they are structurally insufficient for an interactive scientific tool.5 Building RetroFITS to support real-time, responsive operations like zooming, panning, and non-linear photometric stretching across multi-gigabyte files necessitates a fundamental shift from stateless image dumping to a stateful, hardware-accelerated, immediate-mode Text User Interface (TUI) architecture.7

## **2\. Technical Requirements**

### **2.1 The FITS Data Standard and Rust Parsing Ecosystem**

Astronomical images fundamentally differ from consumer raster graphics (like JPEG or PNG) in their structure, intent, and encoding. A FITS file consists of one or more Header Data Units (HDUs), each containing an ASCII header block describing the telemetry, coordinate systems, and exposure metadata, followed by an N-dimensional data array.9 The pixel data is uncompressed, strictly linear, and stored in formats ranging from 8-bit integers to 64-bit floating-point numbers, denoted in the header by BITPIX values of 8, 16, 32, \-32, or \-64.8

Selecting the appropriate parsing engine is the first critical step in RetroFITS's lifecycle:

| Crate | Primary Architectural Focus | Capabilities and Trade-offs | Source |
| :---- | :---- | :---- | :---- |
| fitrs | Pure Rust, minimalistic experimental parsing. | Utilizes minimal dependencies (primarily byteorder for endianness resolution). Designed specifically for multi-threaded environments but lacks support for complex multi-extension FITS (MEF) conventions. Highly unstable API. | 12 |
| fitsrs | Exhaustive multi-HDU support and legacy compatibility. | Supports tiled compressed images, asynchronous reading, iterators for large files, and WCS header parsing. Heavily ported from the legacy C-based CFITSIO library logic, including RICE decompression algorithms. | 9 |
| ravensky-astro | Modular, production-tested astrophotography tooling. | Handles both FITS and XISF formats natively. Features the astro-io module for efficient I/O operations and astro-metadata for header extraction. Designed specifically for integration into larger astrophotography workflows. | 14 |
| fitparser | FIT file SDK compliance via Serde serialization. | Heavily focused on Garmin/ANT FIT profiles rather than astronomical images, making it entirely unsuitable for telescope data visualization. | 15 |

For a robust terminal viewer, fitsrs and ravensky-astro present the most viable foundations due to their ability to navigate multiple HDUs and manage compressed binary tables.10

### **2.2 Memory Mapping and Zero-Copy System Architectures**

A fundamental physical constraint in rendering deep-sky images is file size. Mosaic images, multi-filter sensor dumps, or large-format CCD arrays easily exceed several gigabytes.16 Standard file input/output routines, such as invoking std::fs::File::read\_to\_end(), force the operating system to copy these massive payloads directly into heap-allocated Vec\<T\> structures.18 This naive approach guarantees unacceptable spikes in random access memory (RAM) usage.

The optimal architectural pattern relies on memory-mapped file input/output via the memmap2 crate.20 By utilizing an immutable Mmap struct, RetroFITS entirely avoids loading the file into user-space RAM. Instead, it instructs the Linux kernel's virtual memory manager to map the file directly into the application's address space.19

To manipulate this memory-mapped data mathematically without triggering allocations, the mmap byte slice must be safely cast into a multi-dimensional mathematical array using the ndarray crate.8 Converting the raw bytes into an ArrayView2\<f32\> yields a zero-copy data structure that fully supports advanced matrix operations.8

### **2.3 Terminal Graphics Protocols and Capabilities**

Modern terminal emulators implement specialized, proprietary escape sequence protocols designed to intercept and render high-resolution raster graphics directly over the text grid.3 RetroFITS must intelligently query the host environment upon initialization and automatically select the most capable protocol available.7

| Protocol Specification | Architectural Mechanism | Key Features and Limitations | Ecosystem Adoption |
| :---- | :---- | :---- | :---- |
| **Kitty Graphics Protocol** | Transmits highly structured base64 payloads encapsulated by the \\e\_G escape sequence. Supports explicit commands for placement, Z-index layering, and truecolor. | Full GPU acceleration, RGBA transparency, high-speed rendering, and explicit image ID management. | Kitty, WezTerm, Ghostty, Konsole.7 |
| **iTerm2 Inline Protocol** | Utilizes the \\e\]1337;File= Operating System Command (OSC) sequence with direct base64 encoded file data. Originally designed exclusively for macOS. | Handles Retina scaling seamlessly and supports animation, but lacks the granular layout control of the Kitty protocol. | iTerm2, WezTerm, Konsole, mintty.6 |
| **Sixel (Six Pixels)** | Legacy DEC VT340 standard initiated by the \\ePq sequence. Encodes images as vertical strips of 6 pixels using an ASCII run-length encoding (RLE) mechanism. | Widely supported fallback. However, it is severely limited to indexed palettes (often a maximum of 256 colors) and produces inefficiently large payloads. | Foot, Konsole, Xterm, Contour, WezTerm.7 |

RetroFITS must be built with a strict graceful degradation hierarchy:

1. Attempt the **Kitty Protocol** first for GPU-accelerated, truecolor rendering capable of matching desktop GUI performance.  
2. Fall back to **iTerm2** or **Sixel** protocols if Kitty is unsupported.  
3. Fall back to **Unicode Half-Blocks** (using characters like ▄ or ▀ coupled with 24-bit ANSI background and foreground escape codes) if no true graphics protocol exists.5

### **2.4 Remote SSH and Multiplexer Compatibility**

Operating RetroFITS over a remote SSH connection introduces critical network and protocol constraints that the architecture must address natively:

* **In-Band Data Transmission:** Over SSH, the local terminal emulator cannot access the remote server's filesystem or rely on shared memory optimizations. The architecture must rely strictly on in-band transmission, meaning all image pixel data is serialized (e.g., base64 for Kitty) and transmitted directly within the terminal escape sequences.4  
* **Multiplexer Compatibility (tmux / screen):** Users frequently run remote sessions inside terminal multiplexers, which enforce strict limits on the size of escape sequences. RetroFITS will rely on ratatui-image to actively chunk the encoded image payloads, preventing tmux from truncating the data or dropping the connection.  
* **Graceful Degradation:** If the specific SSH client or an intermediate proxy strips complex raster graphics escape sequences entirely, the initialization capability detection will fail gracefully. RetroFITS will automatically fall back to rendering the FITS data using Unicode half-blocks and ANSI truecolor, ensuring the viewer remains functional for visual checks regardless of the connection's graphical limitations.7

### **2.5 UI Architecture: Viewport Construction (Zoom and Pan)**

To manage the terminal window, capture complex keyboard and mouse events, and handle dynamic layout constraints, the application should be constructed using ratatui.28

For an interactive astronomical viewer that requires persistent coordinate tracking for zooming and panning, the standard stateless Image widget is entirely insufficient. Instead, the system must utilize ratatui\_image::StatefulImage.7 By binding the decoded, memory-mapped FITS ndarray into a StatefulProtocol, RetroFITS gains precise, continuous control over how the pixel data maps to the ratatui\_core::layout::Rect viewport.30

* **Panning Mechanics:** In the ratatui-image ecosystem, continuous movement is governed by configuring the underlying protocol state to use the Resize::Crop enumerator.7 To initiate panning, the application manages a persistent Offset { x, y } struct within its main application state, incremented via user inputs to virtually move the rendering window across the underlying FITS ndarray.30  
* **Zoom Mechanics:** Zooming requires recalculating the aspect ratio and executing resampling algorithms manually before passing the buffer to the protocol encoder.7 Decimation via ndarray::s\! slicing allows for rapid strided reads for fast, low-quality previews, followed by asynchronous high-quality interpolation once scrolling ceases.8

### **2.6 Decoupling State and Rendering**

To maintain a 60 frames-per-second interface, heavy image transformations must be aggressively offloaded to a background thread.7 The architecture employs a multi-threaded channel paradigm separating the UI state from the mathematical rendering engine using std::sync::mpsc or tokio::sync::mpsc.7

### **2.7 Astrophotographic Signal Processing and Colormaps**

Raw FITS images possess extreme, highly skewed histograms and require a mathematically rigorous non-linear stretch, often referred to as applying a Screen Transfer Function (STF).2 RetroFITS must natively support:

1. **Linear Normalization:**  
   ![][image1]  
2. **Logarithmic Stretch:**  
   ![][image2]  
3. **Asinh Stretch:**  
   ![][image3]

Hardware acceleration (via rayon for parallel processing and SIMD) should be utilized to apply these transformations in real-time across the zero-copy array.8

To aid human visual analysis, RetroFITS will use the prismatica crate to apply scientifically rigorous, perceptually uniform colormaps to the stretched monochromatic data.37

| Scientific Colormap | Visual Characteristics | Optimal Use Case in Astronomy | Source |
| :---- | :---- | :---- | :---- |
| **Viridis** | A remarkably smooth gradient shifting from deep blue through green to bright yellow. Features monotonically increasing luminance. | Ideal for high-contrast data where subtle feature discrimination across all signal levels is strictly required. | 39 |
| **Plasma** | A smooth arc transitioning through blue, purple, and vibrant yellow. Features generally brighter lower bounds than Viridis. | Excellent for 2D projections without deep absolute black requirements. | 39 |
| **Inferno / Magma** | Features a deep absolute black base mapping, smoothly shifting upward through red, orange, and finally to white/yellow. | The absolute optimal choice for representing thermal emissions, X-ray data, or faint nebulosity against a naturally black sky background. | 39 |

## **3\. Phased Rollout Plan**

### **Phase 1: Core Rendering and Interactivity**

* **Data Standard & Parsing:** Implement fitsrs or ravensky-astro for FITS Header/Data Unit parsing.  
* **Memory Management:** Utilize memmap2 and ndarray for zero-copy memory mapping of gigabyte-scale images.  
* **Terminal Graphics:** Implement protocol detection (Kitty, iTerm2, Sixel, Unicode) using ratatui-image and rustix.  
* **Remote Networking:** Implement payload chunking for multiplexer compatibility and ensure strictly in-band transmission for SSH workflows.  
* **UI & Viewport:** Build stateful TUI with ratatui, implementing pan (Resize::Crop \+ Offsets) and fractional zooming.  
* **Concurrency:** Decouple UI state and mathematical rendering via MPSC channels to maintain a 60 FPS input loop.  
* **Image Processing:** Implement SIMD-accelerated Asinh, Logarithmic, and Linear stretching algorithms.  
* **Color Mapping:** Integrate prismatica for applying Viridis, Plasma, Inferno, and Magma palettes.

### **Phase 2: Astrometric Integration (WCS) \- *Deferred***

*If Phase 1 stabilization is successful, the following analytical context features will be integrated into RetroFITS.*

A highly advanced terminal viewer should function as an analytical tool that contextualizes raw pixels within the broader celestial sphere. Embedded within standard FITS headers are specific World Coordinate System (WCS) matrices (encompassing keywords such as CRPIX, CRVAL, CDELT, and CTYPE). These parameters contain the rigorous mathematical transformations necessary to bind the flat, two-dimensional pixel grid to standard spherical equatorial coordinates—specifically, Right Ascension (RA) and Declination (Dec).41

The Rust crate celestial-wcs provides a comprehensive, pure-Rust implementation capable of interpreting these complex FITS keywords without relying on legacy C-based foreign function interfaces (FFI), ensuring memory safety and cross-platform reliability.42 During file initialization, the application parses the header ASCII block and constructs a persistent WcsBuilder object.42

As the user pans across the high-resolution image using the keyboard or mouse, the main TUI thread reads the central pixel coordinate of the current Rect viewport. This continuous ![][image4] and ![][image5] pixel location is passed instantly through the WCS spherical projection pipeline. The system calculates the true celestial coordinates in real-time, allowing the TUI to dynamically display the exact Right Ascension and Declination of the current viewpoint in the bottom Ratatui status bar.42 This critical functionality fully transitions RetroFITS from a simple command-line image previewer to a legitimate, robust observational analysis utility.

#### **Works cited**

1. rust reading .fits file : r/rust \- Reddit, accessed April 9, 2026, [https://www.reddit.com/r/rust/comments/q0l7xs/rust\_reading\_fits\_file/](https://www.reddit.com/r/rust/comments/q0l7xs/rust_reading_fits_file/)  
2. Image stretching — Siril 1.5.0 documentation, accessed April 9, 2026, [https://siril.readthedocs.io/en/latest/processing/stretching.html](https://siril.readthedocs.io/en/latest/processing/stretching.html)  
3. Terminal Graphics Protocol for fast embedded development \- Nicolas Mattia, accessed April 9, 2026, [https://nmattia.com/posts/2026-03-10-kitty-graphics-micropython/](https://nmattia.com/posts/2026-03-10-kitty-graphics-micropython/)  
4. Terminal graphics protocol \- kitty \- Kovid Goyal, accessed April 9, 2026, [https://sw.kovidgoyal.net/kitty/graphics-protocol/](https://sw.kovidgoyal.net/kitty/graphics-protocol/)  
5. atanunq/viuer: Rust library for displaying images in the terminal. \- GitHub, accessed April 9, 2026, [https://github.com/atanunq/viuer](https://github.com/atanunq/viuer)  
6. GitHub \- hzeller/timg: A terminal image and video viewer., accessed April 9, 2026, [https://github.com/hzeller/timg](https://github.com/hzeller/timg)  
7. Ratatui widget for rendering image graphics in terminals that support it \- GitHub, accessed April 9, 2026, [https://github.com/ratatui/ratatui-image](https://github.com/ratatui/ratatui-image)  
8. AstroBurst: astronomical FITS image processor in Rust — memmap2 \+ Rayon \+ WebGPU, 1.4 GB/s batch throughput \- Reddit, accessed April 9, 2026, [https://www.reddit.com/r/rust/comments/1ri29nu/astroburst\_astronomical\_fits\_image\_processor\_in/](https://www.reddit.com/r/rust/comments/1ri29nu/astroburst_astronomical_fits_image_processor_in/)  
9. fitsrs \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/fitsrs](https://docs.rs/fitsrs)  
10. GitHub \- cds-astro/fitsrs: FITS file reader library implemented in pure Rust, accessed April 9, 2026, [https://github.com/cds-astro/fitsrs](https://github.com/cds-astro/fitsrs)  
11. celestial-images \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/celestial-images](https://crates.io/crates/celestial-images)  
12. fitrs \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/fitrs](https://docs.rs/fitrs)  
13. fitrs \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/fitrs](https://crates.io/crates/fitrs)  
14. ravensky-astro \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/ravensky-astro](https://crates.io/crates/ravensky-astro)  
15. fitparser \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/fitparser](https://crates.io/crates/fitparser)  
16. tiled \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/tiled](https://docs.rs/tiled)  
17. Image-rs and images with huge'ish dimensions : r/rust \- Reddit, accessed April 9, 2026, [https://www.reddit.com/r/rust/comments/irgb4u/imagers\_and\_images\_with\_hugeish\_dimensions/](https://www.reddit.com/r/rust/comments/irgb4u/imagers_and_images_with_hugeish_dimensions/)  
18. Read FITS file in rust \- help \- The Rust Programming Language Forum, accessed April 9, 2026, [https://users.rust-lang.org/t/read-fits-file-in-rust/65490](https://users.rust-lang.org/t/read-fits-file-in-rust/65490)  
19. Mmap in memmap \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/memmap/latest/memmap/struct.Mmap.html](https://docs.rs/memmap/latest/memmap/struct.Mmap.html)  
20. I built AstroBurst — an open-source FITS processor in Rust/WebGPU. Here's JWST's Pillars of Creation composed from raw NIRCam data. : r/Astronomy \- Reddit, accessed April 9, 2026, [https://www.reddit.com/r/Astronomy/comments/1rhhtkn/i\_built\_astroburst\_an\_opensource\_fits\_processor/](https://www.reddit.com/r/Astronomy/comments/1rhhtkn/i_built_astroburst_an_opensource_fits_processor/)  
21. How to Parse Large Files with Zero-Copy Techniques in Rust \- OneUptime, accessed April 9, 2026, [https://oneuptime.com/blog/post/2026-01-25-parse-large-files-zero-copy-rust/view](https://oneuptime.com/blog/post/2026-01-25-parse-large-files-zero-copy-rust/view)  
22. Mmap/Ndarray: Manage lifetime of two instances tied together (design question), accessed April 9, 2026, [https://users.rust-lang.org/t/mmap-ndarray-manage-lifetime-of-two-instances-tied-together-design-question/126096](https://users.rust-lang.org/t/mmap-ndarray-manage-lifetime-of-two-instances-tied-together-design-question/126096)  
23. Efficiently map array to a (possibly) different type · Issue \#1031 \- GitHub, accessed April 9, 2026, [https://github.com/rust-ndarray/ndarray/issues/1031](https://github.com/rust-ndarray/ndarray/issues/1031)  
24. Are We Sixel Yet?, accessed April 9, 2026, [https://www.arewesixelyet.com/](https://www.arewesixelyet.com/)  
25. Feature Request: Terminal Graphics Protocol Support (Sixel, Kitty, iTerm2) · Issue \#2266 · anthropics/claude-code \- GitHub, accessed April 9, 2026, [https://github.com/anthropics/claude-code/issues/2266](https://github.com/anthropics/claude-code/issues/2266)  
26. ratatui-image \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/ratatui-image/1.0.0](https://crates.io/crates/ratatui-image/1.0.0)  
27. ratatui-image \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/ratatui-image/0.6.0](https://crates.io/crates/ratatui-image/0.6.0)  
28. v0.23.0 \- Ratatui, accessed April 9, 2026, [https://ratatui.rs/highlights/v023/](https://ratatui.rs/highlights/v023/)  
29. v0.24.0 · ratatui ratatui · Discussion \#590 \- GitHub, accessed April 9, 2026, [https://github.com/ratatui-org/ratatui/discussions/590](https://github.com/ratatui-org/ratatui/discussions/590)  
30. Rect in ratatui::layout \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/ratatui/latest/ratatui/layout/struct.Rect.html](https://docs.rs/ratatui/latest/ratatui/layout/struct.Rect.html)  
31. ratatui-image \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/ratatui-image/2.0.1](https://crates.io/crates/ratatui-image/2.0.1)  
32. ratatui\_image \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/ratatui-image](https://docs.rs/ratatui-image)  
33. ratatui-image \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/ratatui-image/0.5.1](https://crates.io/crates/ratatui-image/0.5.1)  
34. Offset in ratatui::layout \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/ratatui/latest/ratatui/layout/struct.Offset.html](https://docs.rs/ratatui/latest/ratatui/layout/struct.Offset.html)  
35. Add an option so that image resizes up not only down · Issue \#59 \- GitHub, accessed April 9, 2026, [https://github.com/benjajaja/ratatui-image/issues/59](https://github.com/benjajaja/ratatui-image/issues/59)  
36. \[Media\] AppCUI-rs \- Powerful & Easy TUI Framework written in Rust \- Reddit, accessed April 9, 2026, [https://www.reddit.com/r/rust/comments/1lsy0n9/media\_appcuirs\_powerful\_easy\_tui\_framework/](https://www.reddit.com/r/rust/comments/1lsy0n9/media_appcuirs_powerful_easy_tui_framework/)  
37. Prismatica — Rust data vis library // Lib.rs, accessed April 9, 2026, [https://lib.rs/crates/prismatica](https://lib.rs/crates/prismatica)  
38. prismatica \- Rust \- Docs.rs, accessed April 9, 2026, [https://docs.rs/prismatica](https://docs.rs/prismatica)  
39. Color Map Advice for Scientific Visualization \- Kenneth Moreland, accessed April 9, 2026, [https://www.kennethmoreland.com/color-advice/](https://www.kennethmoreland.com/color-advice/)  
40. How to implement colormaps like Rainbow and Viridis in code \- Habrador Blog, accessed April 9, 2026, [https://blog.habrador.com/2023/04/colormaps-overview-code-implementations-rainbow-virids.html](https://blog.habrador.com/2023/04/colormaps-overview-code-implementations-rainbow-virids.html)  
41. wcs \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/wcs](https://crates.io/crates/wcs)  
42. celestial-wcs \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/celestial-wcs/0.1.1-alpha.2](https://crates.io/crates/celestial-wcs/0.1.1-alpha.2)  
43. celestial-wcs \- crates.io: Rust Package Registry, accessed April 9, 2026, [https://crates.io/crates/celestial-wcs](https://crates.io/crates/celestial-wcs)  
44. List of all items in this crate \- Docs.rs, accessed April 9, 2026, [https://docs.rs/celestial-wcs/latest/celestial\_wcs/all.html](https://docs.rs/celestial-wcs/latest/celestial_wcs/all.html)

[image1]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAmwAAAAxCAYAAABnGvUlAAAEWklEQVR4Xu3dSYgcVRgH8GfURFwQclEJGNTgIRpBEeMhyKiIoJKLILiA0UNUIi45CEEQDIiISETFg7ggXgTFBRSXBDcQcQG9BA+ueFATQaPgQSHo+6hX09XV1a0Tumd6Zn4/+FOvvq6ZquNH1atXKQEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAsdf90ZE/fEZO1Lw2e/8+cjc2DAACWu2iSNjX2jyy1+fJxqpq0pvk8PwDAVDs6DTZH53fUJinOdW9HDQCA7O402BztzfmjVZukOH80jrXTSg0AgDQ4f+zbnDV9R0zW9jR4Dff0HQEAsMxFg3RKu1gcbBdGiDt1u0dkmN9yfmgX/6df2gUAgKVooR89xvlvaBcBAOgZ1rCty7mkjJ9I1WPKV3IemT1iPIadP8RduwdzPi377+asL+P3c04t49dzTsh5NefmUgMAWPTicWc9Z6zr0We8BHBFY/+nsh3VYM3FttQ/by0arqYNOT+W8eZUNY9H5OwqtXirdHXOirJfO9DaBwDoszbn8pz9OTMlt+Z82Ttk0Ti2MY67bXfkrMx5NOekxm+TdHvZ1nfYHk/VNaxKVZN3TKnfWLax2O5VZQwAMFTcDdrRqo2a2D+tdua8WcZP5xyeszVVzdJF9UETFucM9V292J5exi/lfFTG8WZreC71N5oAAJ3+ahfS+B4jAgAwBu3m7OrUm3cFAMACiwnwzYn0kef7jjh0sfr/zIjEJH0AAP5DTMxv32Gbi0l9DuodmQ0AsMxFsxYT37vEshWf55xY9ut1xOJlhHNTNYE+/v7v8nvbe2nw7l0zH8weCQDAUNE4ndEuZsflvFbGdVN3XqqWo3i2Pii7szEGAGCMDkv9d7va9jXG9e97W/shFoYFAGABPJCqLwfEI9Ba3ah9nfNizppU3Yk7e/aI8Ts+57NUnXum5O00ublzh2om5/ecy8o48lXOybNHAAAsYdGsvdFR29KqLbSuO5VdNQCAJSeanos7avEJrWnSbs7iqwftGgDAktRueq7vqC20C9LgNT2Zc22rBgCwJEUj1Ey8qTptfk3917g/Z1PfEd3i5Y9hy6oAACwK5+Q81S5OULzQsHtIRjVW0aRtaRcBAJaDF1L/m6rTaEUafBwKALBsDGuEYqmPl8v4mpwdZXxf2cYXGqKRiq8zhNVlG/9vbc7Osj8OscTJsOuMFw++KOODZXthzqU5j6XeXbsr0/D/AQAwleID9M05YV3q+ieNWrMxa25rsYbcOMUCw6Ou86acs8r4/rKNx6u15t/8XLbXNWoAAItaTPQPddOzIVWT+OOu1oel9n3OMzmbczam3lIgcUdrPnzXGMciwCGud32qvsl6V6oaznU528vv8W3Wt8oYAGDRWpV6S2bUj0ZX5mwt44dzvknVB+qjkTsz55ay/1A5Zj4caBeyPal6g/SoMo4m87bG77tKDQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAO/gVOARR5rd84FgAAAABJRU5ErkJggg==>

[image2]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAmwAAAAzCAYAAAAq0lQuAAAFXUlEQVR4Xu3dXahlZRkH8DczG610VLJACzXBIQnEj5EI5KBpBgYig18gThP0cSGaNyIqBSqUN2pqEYk36kUJIRGp48fghSSKKA6JWGpYF6GgKFhoDPk+7bVmv/PM3nPOmbPn7LNn/37wZ73vs95z9jl3D2utvd5SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAYJ85oOarubgXflFzYC7OmONqNuUiAMA0ban5d80j+cQynVJzQy5WD+XCKlio+V/Nr7rxt7v5dcMle/RYLgAATNttNY/m4jJFQ5Tnb9X8OdVXS3x+e7XvvK62VJ/JBQCAabq1ZmuqxdWow1MtXNUdz6/5Sje+vObObpw9kwur4Maye3P2+IjanixnLQDAPvfzMrwluq4Mm5VLyrCR+1LNf7txf74/PlHzzW6cTbphO6vm7zUbyvDzv7fz7MD7Ndua+foyWHtiU1uMhg0AWFOiYetvicbzbJc256Jx+VrNP8pgXfiw5pqdK0p5r+bLzbw1yYbt1LJrI/VgzR019ze1EGvaPFvz6e7c2/2iRWjYAIA1pb3CFo3KF5tz0Yw9XQZrtne1WPPZnSsG80Oaeeu5XGgs7CEbY0ESn3N0M/9BGV71a02i2ZrE7wAAmJh4hq1t2L7VnIv5j2u+XnNlN9/cnA8vl/GvBXk+F1YgN1GbR9TCqFq4t+bmbnxGGdzKDaPWj6oBAEzN7WV4S/Simn914/7Zr3B9zdndOPt1zRW52HkhF1ZgR83B3figMri9GX/faTtXlHJ62f0WaeuIMnjvXCueecs0bAAwhxZq7inDd4NFoilY6rvBpu31Mnjg/1M1n6x5p+ba5vwsNDhxhe3qZhxuKoPbuz/p5r1xDSgAsJ+Ld5Kt5N1g05T/zuPL8JZieLIMH+xfq+IFunEFLvyzO8Y8vyvupDQHAOZIbnriClWurVXn1rxSBjsafLeM/rvfyIUZFV+0AADmVG5yYv5Sqs2yeK5s1PZUs2RbLgAA8yPeWxYNWptJ3UJcWCST+hwAgP3auzVv5uIyxKszJu2nazTxBYD2mOujavmYz++pFvNRtX7cfgsVANiPxRW1Lbm4DL/NBQAAJie+iRgN2zh/644X1/yp5qFu3v7MJ5oxAAAT1D+v1ieLvTt7sV9nbPHUr+tf6HpMd5ym2E90Eqb9v0zyRb4AwJxom7jYAir8oebYMniOKvb1jA3Ow+e642qLratGNZvL9ddcKIOrj2fm4j60UFZ2axoAmEOxY0Bodw24r+a1mhNqvlDzlzJs2qZlpQ1bfFM17yvaX3U8P9XHiQ3oJ2Gl/wsAwJqUm5zYpeGyVAuH1ZzTjdsrWfnne1H/Ti6O8WIu7KV4VvCuXAQAmHVtwxXj/t1ucSWwF7dwf1dzZBms2dicW82GLX7nN2ourDm6q7XN47qy67ODAAD7hb7hio3ff9nU49ur3+/GsSY2Uu/HrTzvTbphi9efHNDMP+rmG5paGPf3AADMrL7B+VEZ3FJs9ediD85LUq2X572oX5CLnbhCt9Dk1TQ/KhY1Di27f07M40scWV4HADDz+gbnxJpb2hPV77vjH2t+U/OfmiOGp/9vXIMU9bh1uRSLXWE7u+z+OXneG1cHAJhZbYOzoxk/0Iw/rDm4mbfGNUhR35SLYyzWsIX2c05p5vF6lF40nc80cwCAuRHN0cndOB72b5una2p+2Mz3xlIatqXYXuwcAQDMoWNrHk61e9N83FW2pfp8LuylD3IBAGBe3FNzd836mp/VPLfr6SXf+tyXnsoFAAB2tTUXVln/XjYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACACfgY3H8bJHwBhjoAAAAASUVORK5CYII=>

[image3]: <data:image/png;base64,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>

[image4]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABIAAAAYCAYAAAD3Va0xAAAAvElEQVR4XmNgGAWkAmsg/o8Fw8B3LHK8SPIYQIEBoug5ELMhiQsC8SMgFkMSIwj8GCCG7YbyWYB4JRAzw1UQCZgYEM5nBOKlDKiuIwnADLoFxBJociSBXQwQgwrRJUgFf4H4EwPEMD00OaKBKxCXA3ErA8SgKajSxIEnQFyJxEdPT0QBUBppQRMj2aC1QHwEXZAB4jqQQQvQxFEAKI1EA3EVA0TxWVRpBj4gXgiVA2WRcCAWRlExCkYBlQAAUD8s5y1xrpcAAAAASUVORK5CYII=>

[image5]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABAAAAAYCAYAAADzoH0MAAAApklEQVR4XmNgGAUgwA/E/7FgkDgMMAPxBzT5eiR5MOCASoAwNmACxD1AzIQugQxgBjCiiR8B4h1oYljBUwaIAQ5IYiAbLwCxIJIYTmDKADHgNRCzAnECA8TvJIGrDBBDAoH4NxB7oUoTBoYM+AOTKADSfBddkFiQyAAxwB9dglgwnwFiAB+6BDFAjwGiuQtdghAAJVVQSlvMADFgIgNqWhgFowAvAAAVtSV4+tY2pwAAAABJRU5ErkJggg==>