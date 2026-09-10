<div align="center">
  <img src="assets/textflow-logo.svg" alt="textflow-rs" width="640">

  <p><strong>Bounded Unicode text layout and shaping for Rust</strong></p>

  <p><code>no_std</code> by default · caller-owned memory · renderer-neutral</p>

  <p>
    <a href="https://crates.io/crates/textflow-rs"><img src="https://img.shields.io/crates/v/textflow-rs?style=flat-square&amp;logo=rust&amp;color=ce422b" alt="Crates.io"></a>
    <a href="https://docs.rs/textflow-rs"><img src="https://img.shields.io/docsrs/textflow-rs?style=flat-square&amp;color=3949b8" alt="docs.rs"></a>
    <a href="LICENSE"><img src="https://img.shields.io/crates/l/textflow-rs?style=flat-square&amp;color=00a38c" alt="License"></a>
    <a href="#feature-flags"><img src="https://img.shields.io/badge/no__std-default-3949b8?style=flat-square" alt="no_std by default"></a>
    <a href="Cargo.toml"><img src="https://img.shields.io/badge/MSRV-1.85-2479c4?style=flat-square" alt="MSRV 1.85"></a>
  </p>

  <p>
    <a href="#demo">Demo</a>
    · <a href="https://docs.rs/textflow-rs">API</a>
    · <a href="#embedded-footprint">Embedded</a>
    · <a href="#used-by">Used by</a>
    · <a href="#feature-flags">Features</a>
    · <a href="https://crates.io/crates/textflow-rs">Crate</a>
  </p>
</div>

---

`TextFlow` provides line breaking, bidirectional text, glyph shaping, font fallback, visual runs, and caret positioning. Caller-owned buffers make memory use explicit, while the optional `alloc` feature provides a reusable fixed-capacity workspace over the same algorithms.

<p align="center">
  <img src="assets/textflow-architecture.svg" alt="TextFlow architecture: UTF-8 input passes through Unicode analysis, bidirectional resolution, typeface shaping, and line layout into positioned glyphs" width="1200">
</p>

## Used by

[mirui](https://github.com/W-Mai/mirui) uses TextFlow for bounded Unicode analysis, shaping, line layout, and caret placement with MIRX and OpenType font sources across software, GPU, and web renderers.

## Demo

```rust
use textflow::TextFlow;

let text = "A small UI can still set type well.";
let lines = TextFlow::new(text, 12)
    .map(|line| line.text())
    .collect::<Vec<_>>();

assert_eq!(lines, ["A small UI", "can still", "set type", "well."]);
```

## Text layout and shaping

- `#![no_std]` by default with a `core`-only runtime.
- Caller-owned buffers with explicit capacity and unsupported-text errors.
- Format-neutral `Typeface` and `GlyphSource` interfaces for TTF, OpenType, MIRX, flash-backed assets, and application-specific font stores.
- Stable glyph IDs, UTF-8 cluster ranges, visual bidi runs, safe line boundaries, and caret positions.
- Optional reusable heap workspace with admission limits and no growth during layout.

## Embedded footprint

The [ESP32-C3 demo](examples/esp32c3/) runs bidirectional resolution, font selection, shaping, wrapping, visual reordering, glyph positioning, and caret generation without an allocator. One fixed-capacity `LayoutScratch` value owns all reusable working storage.

Measured with Rust 1.97.0, `esp-hal` 1.1.0, size optimization, LTO, one codegen unit, and aborting panics:

| ESP32-C3 release image | App partition | Static RAM (`.data + .bss`) |
| --- | ---: | ---: |
| ESP-HAL baseline | 73,536 B | 808 B |
| Full TextFlow pipeline | 87,056 B | 824 B |
| TextFlow delta | **13,520 B** | **16 B** |

The demo's main stack frame is 2,272 B, including a 2,016 B `LayoutScratch`. Heap usage is 0 B. Run `examples/esp32c3/measure.sh` to rebuild both images and reproduce the Flash and static RAM comparison.

## Feature flags

| Feature | Adds |
| --- | --- |
| `unicode` | Bounded grapheme, line-break, and script classification |
| `bidi` | Caller-buffer bidirectional paragraph resolution |
| `shaping` | Format-neutral shaping, fallback, line selection, and positioned output |
| `complex-shaping` | Bounded script providers with borrowed substitution and positioning data |
| `script-arabic` | Arabic joining forms, ligatures, cursive attachment, and mark positioning |
| `script-thai` | Thai cluster substitution and mark positioning |
| `script-devanagari` | Devanagari conjunct forms, pre-base matra reordering, and mark positioning |
| `alloc` | Reusable owning workspace for the shaping pipeline |

Default features are empty. The current Unicode and shaping implementation intentionally supports a defined subset and returns capability errors for unsupported scripts, controls, clusters, and font features.

## Font integration

A format adapter supplies `Typeface` for complete shaping or `GlyphSource` for scalar cmap, advance, `.notdef`, and pair-kerning access, together with its parser, table cache, raster lookup, storage access, and diagnostics. The `complex-shaping` feature adds `ScriptTypeface`, `ScriptProvider`, and `ShapingData` for sharing script behavior across borrowed font formats. `script-arabic` and `script-thai` expose the allocation-free `scripts::ARABIC` and `scripts::THAI` providers.

## Documentation

- [API documentation](https://docs.rs/textflow-rs)
- [Source repository](https://github.com/W-Mai/textflow)

## License

[MIT](LICENSE)
