# Changelog

## [Unreleased]

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

[Unreleased]: https://github.com/W-Mai/textflow/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/W-Mai/textflow/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/W-Mai/textflow/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/W-Mai/textflow/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/W-Mai/textflow/releases/tag/v0.1.0
