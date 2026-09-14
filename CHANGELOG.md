# Changelog

## [Unreleased]

### Added

- Free draw opens with a spring-timed near-square heart; its guide clears behind the tip unless Show curve is enabled.
- A Playground budget panel reports feature-specific reference WASM sizes, memory capacities, metered work, and browser-measured throughput.

### Fixed

- Overlong RTL lines retain their logical start at the right viewport edge when clipped; unexpanded justified lines also honor that edge.
- API atlas method searches include only methods available under the selected features and open directly on matching methods.

### Changed

- Baseline and atmospheric motion follow display refresh timing; repeated frames reuse glyph metrics and canvas backing storage.
- Clicking the yuuu canvas briefly highlights Free draw without switching examples.
- The Playground opens on the yuuu Baselines example.
- Free draw hides its baseline guide by default and exposes a Show curve control.
- Site guides cover font adapters, positioned glyphs, bidi overflow, baseline placement, and bounded memory; the README includes a concise coordinate example.
- Scaffold Rust files and API atlas code examples and signatures use syntax highlighting.
- Scaffold manifest and README previews use TOML and Markdown syntax highlighting.

## [0.3.1] - 2026-09-14

### Added

- Baselines accepts up to 512 hand-drawn anchors and places repeated Greek text along the complete yuuu portrait curve.
- A browser Playground runs the TextFlow WebAssembly engine with interactive layout, script, memory, and baseline scenes.
- Feature-aware example scaffolding and a source-derived API atlas accompany the Playground.
- The Baselines scene places text and carets on a hand-drawn curve.
- A Trunk-built Playground site is configured for GitHub Pages.
- The Baselines scene smooths drawn paths, animates glyph placement, and exposes path and text controls.
- Baseline curves can be copied as static Rust `FlowPoint` slices.
- A typographic field behind the page content lets the pointer displace individual multilingual glyphs with spring return.
- Width-constrained scenes provide a draggable viewport boundary and Line flow bars on the same column scale.
- The Bidi scene shows source and screen run order with matched colors and numbers.

### Fixed

- Narrow CJK text wraps in the Playground, and baseline guides follow the placement geometry.
- Line-flow widths are distinguished from rendered text bounds.
- Baseline font size no longer scales the drawn curve; clipped layout output respects the viewport width.
- Desktop panels fit the viewport with independent scrolling for longer content.
- Baseline bounds follow glyph rotation and ink dimensions; short paths apply clip or ellipsis overflow.
- Scaffold examples use the released crate version, and the multilingual page background renders text without visible guide curves.
- Baseline examples keep the selected font size in canvas pixels while fitting curves to the stage.
- Clip hides incomplete glyph clusters at the viewport edge and reports the number of visible glyphs.

### Changed

- The crate homepage points to the browser Playground, and the published package excludes development tooling.

## [0.3.0] - 2026-09-13

### Added

- Borrowed per-line width providers constrain wrapping, alignment, justification, and ellipsis without allocating an intermediate width table.
- `BaselineProvider` places independent paragraph lines through indexed borrowed cursors without materializing a baseline array.
- Allocation-free line and borrowed-polyline baseline cursors with forward sampling, normalized fixed-point tangents, and checked coordinate bounds.
- `ParagraphLayout::place_on` produces caller-owned glyph and optional caret frame sidecars after capacity and geometry preflight.
- `ParagraphLayout::from_slices` validates and borrows retained layout products without copying them.
- `BaselinePlacement::place_carets_into` writes oriented caret frames without requiring a glyph-frame buffer.

### Changed

- Finite `max_lines` layout records truncation without requesting hidden line widths or broken-line storage beyond the configured limit.

## [0.2.4] - 2026-09-11

### Changed

- `TextFlow::layout_with_scratch` runs the complete allocation-free paragraph pipeline with one reusable `LayoutScratch` value.
- Logical-run, initial-shaping, and line-breaking records are internal implementation details.
- The optional `TextWorkspace` grows only private pipeline buffers within runtime limits and writes final glyphs, runs, lines, and carets directly into caller-owned slices.
- Final alignment and overflow width can differ from breaking width or remain unbounded.
- Workspace growth observes an adjustable private-memory byte limit.

## [0.2.3] - 2026-09-10

### Added

- Optional allocation-free Devanagari shaping with conjunct forms, pre-base matra reordering, mark positioning, and linked grapheme clusters.

## [0.2.2] - 2026-09-10

### Fixed

- Arabic combining marks retain transparent joining masks when grapheme clusters are merged.

## [0.2.1] - 2026-09-10

### Added

- Optional allocation-free Arabic and Thai shaping providers with contextual substitution and mark positioning stages.

## [0.2.0] - 2026-09-10

### Added

- Thai script and combining-mark classification.
- Application-provided line-break opportunities through borrowed `LineBreakProvider` values.
- Strict word wrapping with explicit `WordOrGrapheme` fallback.
- Optional bounded shaping engine with separate font-data and script-provider interfaces.

## [0.1.0] - 2026-09-09

### Added

- Bounded `no_std` line layout with borrowed results.
- Caller-buffer Unicode analysis, bidirectional resolution, glyph shaping, line breaking, visual runs, and caret positioning.
- Format-neutral `Typeface` and `GlyphSource` interfaces with stable font and glyph identities.
- Optional reusable `alloc` workspace with fixed admission limits.
- Explicit capacity, font access, and unsupported-text errors.

[Unreleased]: https://github.com/W-Mai/textflow/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/W-Mai/textflow/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/W-Mai/textflow/compare/v0.2.4...v0.3.0
[0.2.4]: https://github.com/W-Mai/textflow/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/W-Mai/textflow/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/W-Mai/textflow/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/W-Mai/textflow/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/W-Mai/textflow/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/W-Mai/textflow/releases/tag/v0.1.0
