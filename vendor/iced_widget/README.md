# Local iced_widget patch

This directory contains the published `iced_widget` 0.14.2 sources under the MIT license. Upstream
commit: `54abf81d13fec06d4d9ac754b03ce2c3313e8a1f` in <https://github.com/iced-rs/iced>.

The local patch changes scrollbar geometry in `src/scrollable.rs`:

- Keep both handles at least 28 logical pixels long when the rail permits.
- Cap the minimum at half the rail length for very small panels.
- Map offsets onto the remaining handle travel, including the final scroll position.
- Ignore empty handles and avoid division by zero during dragging.

The published widget has a fixed two-pixel minimum and provides no public setting for handle length.
Cargo uses this directory through the root patch table. Application interaction tests verify
grabbing and dragging on both axes.

The patch also adds `text_editor::Content::with_editor`, a read-only layout inspection callback. The
editor gutter uses visible layout runs to align line numbers with text during scrolling. The public
content API otherwise exposes no visible-line positions. Application tests verify the gutter,
scrolling, and its visibility preference.

Disabled text fields and multiline editors reject focus operations. Tab skips these controls. The
dialog interaction tests cover both widget types.

On macOS, both text widgets use Command combinations for shortcuts and suppress their text. The
single-line field reads modifiers from each key press, including after focus changes.

The Markdown parser preserves the contents of indented code blocks. Upstream only collects code from
fenced blocks. IWE uses the parser without its HTML output and command-line dependencies.

File editors can adopt an initialized native editor. IWE supplies finite layout bounds before
loading source lines so glyph layout starts with the visible text.

Keep other source files identical to the published crate. Remove each patch when upstream provides
equivalent behavior.
