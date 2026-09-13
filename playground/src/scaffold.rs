use serde::Serialize;

include!(concat!(env!("OUT_DIR"), "/feature_graph.rs"));

#[derive(Serialize)]
pub struct File {
    pub path: &'static str,
    pub content: String,
}

#[derive(Serialize)]
pub struct Feature {
    pub name: &'static str,
    pub requires: &'static [&'static str],
}

#[derive(Serialize)]
pub struct Catalog {
    pub package: &'static str,
    pub version: &'static str,
    pub source_rev: &'static str,
    pub features: Vec<Feature>,
}

pub fn catalog() -> Catalog {
    Catalog {
        package: "textflow-rs",
        version: CRATE_VERSION,
        source_rev: SOURCE_REV,
        features: FEATURE_GRAPH
            .iter()
            .filter(|(name, _)| *name != "default")
            .map(|(name, requires)| Feature { name, requires })
            .collect(),
    }
}

pub fn files(selected: &[String]) -> Result<Vec<File>, String> {
    let mut direct = Vec::new();
    for (name, _) in FEATURE_GRAPH.iter().filter(|(name, _)| *name != "default") {
        if selected.iter().any(|item| item == name) {
            direct.push(*name);
        }
    }
    for name in selected {
        if !FEATURE_GRAPH.iter().any(|(known, _)| known == name) || name == "default" {
            return Err(format!("Unknown feature: {name}"));
        }
    }
    let closure = closure(&direct);
    let features = direct
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        "[package]\nname = \"textflow-demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ntextflow-rs = {{ git = \"https://github.com/W-Mai/textflow\", rev = \"{SOURCE_REV}\", features = [{features}] }}\n"
    );
    let readme = format!(
        "# TextFlow demo\n\nRun `cargo run` to inspect the selected TextFlow features.\n\nThe dependency is pinned to source revision `{SOURCE_REV}`.\n"
    );
    Ok(vec![
        File {
            path: "Cargo.toml",
            content: manifest,
        },
        File {
            path: "src/main.rs",
            content: main_source(&closure),
        },
        File {
            path: "README.md",
            content: readme,
        },
    ])
}

fn closure(direct: &[&'static str]) -> Vec<&'static str> {
    let mut result = Vec::new();
    let mut pending: Vec<&'static str> = direct.to_vec();
    while let Some(name) = pending.pop() {
        if result.contains(&name) {
            continue;
        }
        result.push(name);
        if let Some((_, requires)) = FEATURE_GRAPH
            .iter()
            .find(|(candidate, _)| *candidate == name)
        {
            pending.extend(requires.iter().copied());
        }
    }
    result
}

fn has(features: &[&str], name: &str) -> bool {
    features.contains(&name)
}

fn main_source(features: &[&str]) -> String {
    let mut source = String::from("use textflow::TextFlow;\n");
    if has(features, "unicode") {
        source.push_str("use textflow::unicode::{graphemes, line_breaks, script_runs};\n");
    }
    if has(features, "bidi") {
        source.push_str("use textflow::bidi::{BaseDirection, BidiRun, BidiText};\n");
    }
    if has(features, "shaping") {
        source.push_str("use textflow::shaping::{FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource, SimpleTypeface};\nuse textflow::LayoutScratch;\n");
        source.push_str(DEMO_FONT);
    }
    if has(features, "complex-shaping") {
        source.push_str("use textflow::shaping::{GlyphBuffer, LookupRequest, LookupStatus, ScriptProvider, ScriptTypeface, ShapeError, ShapingData};\n");
        source.push_str(SHAPING_DATA);
    }
    if has(features, "alloc") {
        source.push_str("use textflow::workspace::{LayoutLimits, LayoutOutput, TextWorkspace};\nuse textflow::layout::{LayoutLine, VisualRun};\nuse textflow::shaping::{PositionedGlyph, CaretStop};\n");
    }
    source.push_str("\nfn main() {\n    let text = \"The harbor lights came on before the rain.\";\n    for line in TextFlow::new(text, 18) {\n        println!(\"{}: {}\", line.width(), line.text());\n    }\n");
    if has(features, "unicode") {
        source.push_str("    println!(\"graphemes: {}\", graphemes(text).count());\n    println!(\"breaks: {}\", line_breaks(text).count());\n    println!(\"script runs: {}\", script_runs(text).count());\n");
    }
    if has(features, "bidi") {
        source.push_str("    let mut runs = core::array::from_fn::<_, 32, _>(|_| BidiRun::empty());\n    let bidi = BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut runs).unwrap();\n    println!(\"bidi runs: {}\", bidi.logical_runs().len());\n");
    }
    if has(features, "shaping") {
        source.push_str("    let font = DemoFont;\n    let face = SimpleTypeface::new(&font);\n    let mut scratch = LayoutScratch::<32, 128, 16, 256>::new();\n    let layout = TextFlow::new(text, 9_000).layout_with_scratch(&[&face], &mut scratch).unwrap();\n    println!(\"glyphs: {}\", layout.glyphs().len());\n");
    }
    if has(features, "alloc") {
        source.push_str("    let limits = LayoutLimits { text_bytes: 4096, runs: 32, glyphs: 128, scratch_glyphs: 128, lines: 16, memory_bytes: 65_536 };\n    let mut workspace = TextWorkspace::new(limits);\n    let mut glyphs = [PositionedGlyph::default(); 128];\n    let mut visual = [VisualRun::empty(); 32];\n    let mut lines = [LayoutLine::empty(); 16];\n    let mut carets = [CaretStop::default(); 256];\n    let mut output = LayoutOutput::new(&mut glyphs, &mut visual, &mut lines, &mut carets);\n    let layout = TextFlow::new(text, 9_000).layout_into(&[&face], &mut workspace, &mut output).unwrap();\n    println!(\"workspace glyphs: {}\", layout.glyphs().len());\n");
    }
    if has(features, "complex-shaping") {
        source.push_str("    let providers: [&dyn ScriptProvider; 0] = [];\n    let _complex = ScriptTypeface::new(&font, &DemoData).with_scripts(&providers);\n");
    }
    for (feature, provider) in [
        ("script-arabic", "ARABIC"),
        ("script-thai", "THAI"),
        ("script-devanagari", "DEVANAGARI"),
    ] {
        if has(features, feature) {
            source.push_str(&format!(
                "    println!(\"{feature}: {{:?}}\", textflow::scripts::{provider}.script());\n"
            ));
        }
    }
    source.push_str("}\n");
    source
}

const DEMO_FONT: &str = r#"
struct DemoFont;

impl GlyphSource for DemoFont {
    fn id(&self) -> FontId { FontId::new(1) }
    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        Ok(FontMetrics { units_per_em: 1000, ascender: 800, descender: -200, line_gap: 0 })
    }
    fn glyph_for(&self, ch: char) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(u16::try_from(ch as u32).ok().map(GlyphId::new))
    }
    fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
        Ok(FlowPoint { x: 500, y: 0 })
    }
}
"#;

const SHAPING_DATA: &str = r#"
struct DemoData;

impl ShapingData for DemoData {
    fn substitute(&self, _request: LookupRequest<'_, '_>, _glyphs: &mut GlyphBuffer<'_>) -> Result<LookupStatus, ShapeError> {
        Ok(LookupStatus::NotFound)
    }
    fn position(&self, _request: LookupRequest<'_, '_>, _glyphs: &mut GlyphBuffer<'_>) -> Result<LookupStatus, ShapeError> {
        Ok(LookupStatus::NotFound)
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_closure_comes_from_manifest() {
        let features = closure(&["script-devanagari"]);
        for expected in [
            "script-devanagari",
            "complex-shaping",
            "shaping",
            "bidi",
            "unicode",
        ] {
            assert!(has(&features, expected));
        }
    }

    #[test]
    fn generated_dependency_is_pinned() {
        let files = files(&["alloc".into()]).unwrap();
        assert!(files[0].content.contains(SOURCE_REV));
        assert!(files[1].content.contains("layout_into"));
    }
}
