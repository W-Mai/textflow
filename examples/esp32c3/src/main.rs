#![no_std]
#![no_main]

use core::hint::black_box;
use textflow::bidi::{BaseDirection, BidiRun, BidiText};
use textflow::layout::{
    BrokenLine, GlyphRun, LayoutBuffers, LayoutLine, LayoutOptions, LogicalRun, LogicalRuns,
    TextSpacing, VisualRun, WrapMode,
};
use textflow::shaping::{
    CaretStop, FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource,
    PositionedGlyph, ShapedGlyph, SimpleTypeface, Typeface,
};

esp_bootloader_esp_idf::esp_app_desc!();

struct DemoFont;

impl GlyphSource for DemoFont {
    fn id(&self) -> FontId {
        FontId::new(1)
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        Ok(FontMetrics {
            units_per_em: 16,
            ascender: 12,
            descender: -4,
            line_gap: 2,
        })
    }

    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(Some(GlyphId::new(character as u32 as u16)))
    }

    fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
        Ok(FlowPoint { x: 8, y: 0 })
    }
}

fn layout_once() -> (usize, usize, usize, usize) {
    const TEXT: &str = "Text · אבג";
    let font = DemoFont;
    let face = SimpleTypeface::new(&font);
    let faces: [&dyn Typeface; 1] = [&face];

    let mut bidi_storage = [const { BidiRun::empty() }; 4];
    let bidi =
        BidiText::resolve(TEXT, 0..TEXT.len(), BaseDirection::Auto, &mut bidi_storage).unwrap();

    let mut logical_storage = [LogicalRun::empty(); 8];
    let logical = LogicalRuns::resolve(TEXT, &bidi, &faces, &mut logical_storage).unwrap();
    let mut shaped_storage = [ShapedGlyph::default(); 12];
    let mut glyph_run_storage = [GlyphRun::empty(); 8];
    let mut broken_storage = [BrokenLine::empty(); 4];
    let broken = {
        let shaped = logical
            .shape_into(
                TEXT,
                &faces,
                &[],
                &mut shaped_storage,
                &mut glyph_run_storage,
            )
            .unwrap();
        shaped
            .break_into(
                TEXT,
                72,
                WrapMode::Word,
                TextSpacing::default(),
                &mut broken_storage,
            )
            .unwrap()
    };

    let mut scratch = [ShapedGlyph::default(); 12];
    let mut positioned = [PositionedGlyph::default(); 12];
    let mut visual_runs = [VisualRun::empty(); 8];
    let mut lines = [LayoutLine::empty(); 4];
    let mut carets = [CaretStop::default(); 20];
    let layout = logical
        .layout_into(
            TEXT,
            &faces,
            &[],
            &broken,
            LayoutOptions::new(18),
            LayoutBuffers::new(
                &mut scratch,
                &mut positioned,
                &mut visual_runs,
                &mut lines,
                &mut carets,
            ),
        )
        .unwrap();

    (
        layout.lines().len(),
        layout.runs().len(),
        layout.glyphs().len(),
        layout.carets().len(),
    )
}

#[esp_hal::main]
fn main() -> ! {
    let _peripherals = esp_hal::init(esp_hal::Config::default());
    black_box(layout_once());
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
