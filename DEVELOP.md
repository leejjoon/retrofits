# Development Notes

## Known Issues and Fixes

### Kitty Protocol Popup Window Rendering Bug

**Issue:**
When running the application with the Kitty protocol, opening and then closing a popup window (such as the Summary, Help, or Manual Cut Entry window) would result in the popup window partially remaining on the screen. The background image was not fully restored. However, if the color map was changed while the popup was open, the image would be fully restored upon closing the popup.

**Cause:**
The application uses a `Clear` widget to blank out the area behind popups before drawing them. When the user closes the popup (by changing the internal `InputMode` state back to `Normal`), the application stops rendering the `Clear` widget and the popup text. However, because the terminal's text cells were cleared, and the underlying `ratatui-image` image was not told to re-encode and redraw its content over those cleared cells, the terminal displayed a mix of the cleared area and whatever remaining text/image fragments were left behind.

Changing the color while the popup was open triggered an explicit call to `self.queue_render()`, which told the background rendering thread to generate a new image state, effectively masking the bug once the popup was closed.

**Fix:**
To resolve this, explicit calls to `self.queue_render()` were added in `src/app.rs` within the key handlers (`handle_summary_key`, `handle_help_key`, and `handle_input_key`) when the `InputMode` state is changed back to `Normal`. This ensures that every time a popup window is closed, a new render request is queued, forcing the image to be fully redrawn on the terminal and overwriting the stale cleared cells.