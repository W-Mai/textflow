# TextFlow playground

`tools/build-site.sh` builds the WebAssembly demo, extracts the API index from the current Rust sources, and assembles `playground/dist/`.

Building requires `wasm-pack` and the `wasm32-unknown-unknown` Rust target. Python 3 serves the static output; `tools/check-playground.sh` also requires Node.js.

```sh
tools/build-site.sh
python3 -m http.server 8137 --directory playground/dist
```

The core crate remains independent of the playground crate. The browser runs nine scenes through the TextFlow API and reports unsupported input or capacity failures directly. The glyph adapter is synthetic; its displayed outlines come from browser fonts, while positions, runs, and carets come from TextFlow.

The Baselines scene supports up to 512 hand-drawn anchors, smooths the path, and samples animated glyph placement along it. The yuuu example samples the complete portrait curve at 1024 intervals and repeats Homer's Greek text until it fills the route. Buttons above the Rust API switch examples while preserving each text and path. Font size is measured in canvas pixels in both examples; curve fitting does not rescale glyphs. Spacing, alignment, overflow, smoothing, and motion are adjustable. `COPY POINTS` exports the smoothed baseline as a static Rust `FlowPoint` slice without animation displacement.

Scaffold examples depend on the released `textflow-rs` crate version selected at site build time. The page background carries small multilingual text on gently changing paths; the paths themselves are not drawn.

`tools/check-playground.sh` runs the root crate, extractor, scaffold compile matrix, WebAssembly smoke test, and JavaScript syntax checks.
