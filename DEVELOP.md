# Developer Guide

## Kitty Protocol Image and Popup Z-Order Issue

When building a Terminal UI (TUI) application like `retrofits` that uses `ratatui` alongside high-fidelity graphics protocols (like Kitty), you may run into rendering conflicts between the image and standard text widgets (like Popups).

The `ratatui-image` library implements the Kitty graphics protocol by utilizing **Unicode Placeholders**. This works by emitting the Unicode Private Use Area (PUA) base character `\u{10EEEE}` followed by specific combining characters. When the terminal emulator sees this sequence, it knows to replace that rectangular text region with the corresponding image pixels.

However, this architecture introduces two major challenges for rendering `ratatui` popups over the image:

### 1. Z-Index and Overlapping
By default, the Kitty protocol draws images with `z=0`. According to the Kitty documentation, a `z-index` of 0 causes the image to be drawn **over** any text or background cells that share the same area.

When Ratatui attempts to render a popup in the middle of the screen, it outputs text characters (like ` ` from the `Clear` widget, or border blocks). However, because `z=0`, Kitty still draws the image data right on top of Ratatui's popup blocks, making the popup appear as though it is rendering *behind* the image.

### 2. The "Tofu Characters" Problem
A seemingly logical fix is to alter the Kitty protocol transmission sequence to use a negative z-index (e.g., `z=-1` or `z=-1073741825`), which instructs Kitty to draw the image **underneath** the text.

While this correctly surfaces the popup text, it also brings the `\u{10EEEE}` Unicode placeholders to the top of the z-stack. Since `\u{10EEEE}` is a PUA character, many terminal fonts lack a designated invisible glyph for it. As a result, the terminal draws visible "tofu" boxes () over the entire image grid.

### The Solution: Graceful Degradation
Because `ratatui-image` stores the placeholder sequence entirely in `Cell 0` of the row (meaning `ratatui`'s diffing engine won't naturally "punch a hole" in the sequence when overwriting `Cell 10`), there is no reliable way to natively layer Kitty images and Ratatui widgets without visual artifacts or invasive `ratatui-image` forks.

To solve this in `retrofits`, we implemented **graceful degradation**.
Whenever a popup or help menu is activated (`app.input_mode != InputMode::Normal`), the application dynamically queues a new render request that forces the `ProtocolType` to `Halfblocks`.

The `Halfblocks` protocol natively renders the image using standard terminal text characters. This ensures:
1. The Ratatui popup borders and text are drawn flawlessly on top of the image.
2. The underlying image remains visible (albeit at a lower blocky resolution) for context.
3. Once the popup is dismissed, the state returns to `InputMode::Normal`, and the high-fidelity Kitty protocol image is instantly restored.
