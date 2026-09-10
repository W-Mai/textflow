#![no_std]
#![no_main]

use core::hint::black_box;
use textflow::bidi::BaseDirection;
use textflow::shaping::{
    FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource, SimpleTypeface, Typeface,
};
use textflow::{LayoutScratch, TextFlow, WrapMode};

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

    let mut scratch = LayoutScratch::<4, 12, 4, 20>::new();
    let layout = TextFlow::new(TEXT, 72)
        .with_line_height(18)
        .with_direction(BaseDirection::Auto)
        .with_wrap(WrapMode::Word)
        .layout_with_scratch(&faces, &mut scratch)
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
