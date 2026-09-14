# TextFlow playground

`tools/build-site.sh` builds the WebAssembly demo, extracts the API index from the current Rust sources, and assembles `playground/dist/`.

Building requires `wasm-pack` and the `wasm32-unknown-unknown` Rust target. Python 3 serves the static output; `tools/check-playground.sh` also requires Node.js.

```sh
tools/build-site.sh
python3 -m http.server 8137 --directory playground/dist
```

The core crate remains independent of the playground crate. The browser runs nine scenes through the TextFlow API and reports unsupported input or capacity failures directly. Glyph layout uses the bundled Lato Regular font for both canvas ink and its extracted advances, pair kerning, and fi/fl ligatures. Other layout scenes retain the synthetic demo adapter. The bundled font is licensed under the SIL Open Font License in [web/Lato-OFL.txt](web/Lato-OFL.txt).

Scenes constrained by viewport width show a draggable right boundary on the output canvas, synchronized with the Viewport width control. Line flow bars use the same column scale as the boundary, so each endpoint shows its line width against the available columns. Clip hides incomplete glyph clusters at that boundary and reports visible versus laid-out glyph counts. The Bidi scene uses numbered, color-matched runs to show how a right-to-left paragraph changes their screen positions.

The Baselines scene supports up to 512 hand-drawn anchors, smooths the path, and samples animated glyph placement along it. The yuuu example samples the complete portrait curve at 1024 intervals and repeats Homer's Greek text until it fills the route. Buttons above the Rust API switch examples while preserving each text and path. Font size is measured in canvas pixels in both examples; curve fitting does not rescale glyphs. Spacing, alignment, overflow, smoothing, and motion are adjustable. `COPY POINTS` exports the smoothed baseline as a static Rust `FlowPoint` slice without animation displacement.

Scaffold examples depend on the released `textflow-rs` crate version selected at site build time. A typographic field behind the page content arranges Chinese, English, and Japanese fragments in straight rows. The pointer pushes individual glyphs aside; they spring back after it moves away. Reduced-motion settings keep the field still.

`tools/check-playground.sh` runs the root crate, extractor, scaffold compile matrix, WebAssembly smoke test, and JavaScript syntax checks.
