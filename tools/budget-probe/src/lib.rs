use textflow::TextFlow;

const LATIN: &str =
    "The quick brown fox jumps over the lazy dog. The harbor lights came on before the rain.";
#[cfg(feature = "bidi")]
const BIDI: &str = "שלום, hello · مرحبا";
#[cfg(feature = "script-arabic")]
const ARABIC: &str = "مرحبا بالعالم";
#[cfg(feature = "script-thai")]
const THAI: &str = "ภาษาไทย";
#[cfg(feature = "script-devanagari")]
const DEVANAGARI: &str = "नमस्ते दुनिया";

#[no_mangle]
pub extern "C" fn latin_chars() -> usize {
    LATIN.chars().count()
}
#[cfg(feature = "bidi")]
#[no_mangle]
pub extern "C" fn bidi_chars() -> usize {
    BIDI.chars().count()
}
#[cfg(feature = "script-arabic")]
#[no_mangle]
pub extern "C" fn arabic_chars() -> usize {
    ARABIC.chars().count()
}
#[cfg(feature = "script-thai")]
#[no_mangle]
pub extern "C" fn thai_chars() -> usize {
    THAI.chars().count()
}
#[cfg(feature = "script-devanagari")]
#[no_mangle]
pub extern "C" fn devanagari_chars() -> usize {
    DEVANAGARI.chars().count()
}

#[no_mangle]
pub extern "C" fn run_core() -> usize {
    TextFlow::new(LATIN, 18).map(|line| line.width()).sum()
}

#[cfg(feature = "unicode")]
#[no_mangle]
pub extern "C" fn run_unicode() -> usize {
    use textflow::unicode::{graphemes, line_breaks, script_runs};
    graphemes(LATIN).count() + line_breaks(LATIN).count() + script_runs(LATIN).count()
}

#[cfg(feature = "bidi")]
#[no_mangle]
pub extern "C" fn run_bidi() -> usize {
    use textflow::bidi::{BaseDirection, BidiRun, BidiText};
    let mut logical = core::array::from_fn::<_, 16, _>(|_| BidiRun::empty());
    let mut visual = core::array::from_fn::<_, 16, _>(|_| BidiRun::empty());
    let paragraph = BidiText::resolve(BIDI, 0..BIDI.len(), BaseDirection::Auto, &mut logical)
        .expect("bidi fixture");
    paragraph
        .visual_runs_into(&mut visual)
        .expect("visual runs")
        .len()
}

#[cfg(feature = "shaping")]
mod shape {
    use textflow::shaping::{
        FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource,
    };

    pub struct DemoFont;

    impl GlyphSource for DemoFont {
        fn id(&self) -> FontId {
            FontId::new(1)
        }
        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics {
                units_per_em: 1000,
                ascender: 800,
                descender: -200,
                line_gap: 0,
            })
        }
        fn glyph_for(&self, ch: char) -> Result<Option<GlyphId>, FontAccessError> {
            Ok(u16::try_from(ch as u32).ok().map(GlyphId::new))
        }
        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint { x: 500, y: 0 })
        }
    }
}

#[cfg(feature = "shaping")]
#[no_mangle]
pub extern "C" fn run_shaping() -> usize {
    use textflow::shaping::SimpleTypeface;
    use textflow::LayoutScratch;
    let font = shape::DemoFont;
    let face = SimpleTypeface::new(&font);
    let mut scratch = LayoutScratch::<16, 128, 16, 256>::new();
    TextFlow::new(LATIN, 4800)
        .layout_with_scratch(&[&face], &mut scratch)
        .expect("shaping fixture")
        .glyphs()
        .len()
}

#[cfg(feature = "alloc")]
#[no_mangle]
pub extern "C" fn run_workspace() -> usize {
    use textflow::layout::{LayoutLine, VisualRun};
    use textflow::shaping::{CaretStop, PositionedGlyph, SimpleTypeface};
    use textflow::workspace::{LayoutLimits, LayoutOutput, TextWorkspace};
    let font = shape::DemoFont;
    let face = SimpleTypeface::new(&font);
    let mut workspace = TextWorkspace::new(LayoutLimits {
        text_bytes: 4096,
        runs: 32,
        glyphs: 128,
        scratch_glyphs: 128,
        lines: 16,
        memory_bytes: 65_536,
    });
    let mut glyphs = [PositionedGlyph::default(); 128];
    let mut runs = [VisualRun::empty(); 32];
    let mut lines = [LayoutLine::empty(); 16];
    let mut carets = [CaretStop::default(); 256];
    let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);
    TextFlow::new(LATIN, 4800)
        .layout_into(&[&face], &mut workspace, &mut output)
        .expect("workspace fixture")
        .glyphs()
        .len()
}

#[cfg(feature = "complex-shaping")]
mod complex {
    use textflow::shaping::{GlyphBuffer, LookupRequest, LookupStatus, ShapeError, ShapingData};

    pub struct DemoData;

    impl ShapingData for DemoData {
        fn substitute(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }
        fn position(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }
    }
}

#[cfg(feature = "complex-shaping")]
#[no_mangle]
pub extern "C" fn run_complex() -> usize {
    use textflow::shaping::{ScriptProvider, ScriptTypeface};
    use textflow::LayoutScratch;
    let font = shape::DemoFont;
    let providers: [&dyn ScriptProvider; 0] = [];
    let face = ScriptTypeface::new(&font, &complex::DemoData).with_scripts(&providers);
    let mut scratch = LayoutScratch::<16, 128, 16, 256>::new();
    TextFlow::new(LATIN, 4800)
        .layout_with_scratch(&[&face], &mut scratch)
        .expect("complex fixture")
        .glyphs()
        .len()
}

#[cfg(feature = "script-arabic")]
#[no_mangle]
pub extern "C" fn run_arabic() -> usize {
    use textflow::shaping::{ScriptProvider, ScriptTypeface};
    use textflow::LayoutScratch;
    let font = shape::DemoFont;
    let providers: [&dyn ScriptProvider; 1] = [&textflow::scripts::ARABIC];
    let face = ScriptTypeface::new(&font, &complex::DemoData).with_scripts(&providers);
    let mut scratch = LayoutScratch::<16, 128, 16, 256>::new();
    TextFlow::new(ARABIC, 4800)
        .layout_with_scratch(&[&face], &mut scratch)
        .map_or(0, |layout| layout.glyphs().len())
}

#[cfg(feature = "script-thai")]
#[no_mangle]
pub extern "C" fn run_thai() -> usize {
    use textflow::shaping::{ScriptProvider, ScriptTypeface};
    use textflow::LayoutScratch;
    let font = shape::DemoFont;
    let providers: [&dyn ScriptProvider; 1] = [&textflow::scripts::THAI];
    let face = ScriptTypeface::new(&font, &complex::DemoData).with_scripts(&providers);
    let mut scratch = LayoutScratch::<16, 128, 16, 256>::new();
    TextFlow::new(THAI, 4800)
        .layout_with_scratch(&[&face], &mut scratch)
        .map_or(0, |layout| layout.glyphs().len())
}

#[cfg(feature = "script-devanagari")]
#[no_mangle]
pub extern "C" fn run_devanagari() -> usize {
    use textflow::shaping::{ScriptProvider, ScriptTypeface};
    use textflow::LayoutScratch;
    let font = shape::DemoFont;
    let providers: [&dyn ScriptProvider; 1] = [&textflow::scripts::DEVANAGARI];
    let face = ScriptTypeface::new(&font, &complex::DemoData).with_scripts(&providers);
    let mut scratch = LayoutScratch::<16, 128, 16, 256>::new();
    TextFlow::new(DEVANAGARI, 4800)
        .layout_with_scratch(&[&face], &mut scratch)
        .map_or(0, |layout| layout.glyphs().len())
}
