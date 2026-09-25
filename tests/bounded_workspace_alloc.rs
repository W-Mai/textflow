#![cfg(feature = "alloc")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use textflow::layout::{LayoutLine, VisualRun};
use textflow::shaping::{
    CaretStop, FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource,
    PositionedGlyph, SimpleTypeface, Typeface,
};
use textflow::workspace::{LayoutLimits, LayoutOutput, TextWorkspace, WorkspaceCapacity};
use textflow::TextFlow;

std::thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountedAllocator;

unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let _ = COUNTING.try_with(|enabled| {
                if enabled.get() {
                    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, size) };
        if !resized.is_null() {
            let _ = COUNTING.try_with(|enabled| {
                if enabled.get() {
                    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
                }
            });
        }
        resized
    }
}

#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

struct Source;

impl GlyphSource for Source {
    fn id(&self) -> FontId {
        FontId::new(1)
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        Ok(FontMetrics::default())
    }

    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(Some(GlyphId::new(character as u16)))
    }

    fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
        Ok(FlowPoint { x: 1, y: 0 })
    }
}

#[test]
fn prepared_workspace_does_not_allocate_for_first_or_changed_text() {
    let limits = LayoutLimits {
        text_bytes: 16,
        runs: 8,
        glyphs: 16,
        scratch_glyphs: 8,
        lines: 8,
        memory_bytes: 4096,
    };
    let capacity = WorkspaceCapacity {
        runs: 8,
        glyphs: 16,
        scratch_glyphs: 8,
        lines: 8,
    };
    let mut workspace = TextWorkspace::try_new_bounded(limits, capacity).unwrap();
    let source = Source;
    let typeface = SimpleTypeface::new(&source);
    let typefaces: [&dyn Typeface; 1] = [&typeface];
    let mut glyphs = [PositionedGlyph::default(); 16];
    let mut runs = [VisualRun::empty(); 8];
    let mut lines = [LayoutLine::empty(); 8];
    let mut carets = [CaretStop::default(); 24];

    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|enabled| enabled.set(true));
    for index in 0..20_000 {
        let text = if index % 2 == 0 { "ab cd" } else { "99 ms" };
        let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);
        let layout = TextFlow::new(text, 3)
            .with_line_height(10)
            .layout_into(&typefaces, &mut workspace, &mut output)
            .unwrap();
        assert_eq!(layout.glyphs().len(), 5);
    }
    COUNTING.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
