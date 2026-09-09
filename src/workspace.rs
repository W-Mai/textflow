use crate::bidi::{BidiError, BidiRun, BidiText};
use crate::layout::{
    BrokenLine, GlyphRun, LayoutBuffers, LayoutError, LayoutLine, LayoutOptions, LogicalRun,
    LogicalRuns, ParagraphLayout, TextSpacing, VisualRun,
};
use crate::shaping::{CaretStop, PositionedGlyph, ShapedGlyph, Typeface};
use crate::TextFlow;
use alloc::vec::Vec;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutLimits {
    pub text_bytes: usize,
    pub runs: usize,
    pub visual_runs: usize,
    pub glyphs: usize,
    pub scratch_glyphs: usize,
    pub lines: usize,
    pub carets: usize,
}

impl LayoutLimits {
    pub fn buffer_bytes(self) -> Option<usize> {
        let run_bytes = size_of::<BidiRun>()
            .checked_add(size_of::<LogicalRun>())?
            .checked_add(size_of::<GlyphRun>())?
            .checked_mul(self.runs)?;
        let glyph_bytes = size_of::<ShapedGlyph>()
            .checked_add(size_of::<PositionedGlyph>())?
            .checked_mul(self.glyphs)?;
        run_bytes
            .checked_add(glyph_bytes)?
            .checked_add(size_of::<VisualRun>().checked_mul(self.visual_runs)?)?
            .checked_add(size_of::<ShapedGlyph>().checked_mul(self.scratch_glyphs)?)?
            .checked_add(
                size_of::<BrokenLine>()
                    .checked_add(size_of::<LayoutLine>())?
                    .checked_mul(self.lines)?,
            )?
            .checked_add(size_of::<CaretStop>().checked_mul(self.carets)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    Allocation,
    TextLimit { required: usize, limit: usize },
    DimensionOverflow,
    Bidi(BidiError),
    Layout(LayoutError),
}

impl From<BidiError> for WorkspaceError {
    fn from(error: BidiError) -> Self {
        Self::Bidi(error)
    }
}

impl From<LayoutError> for WorkspaceError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

pub struct TextWorkspace {
    limits: LayoutLimits,
    bidi: Vec<BidiRun>,
    logical: Vec<LogicalRun>,
    initial_glyphs: Vec<ShapedGlyph>,
    initial_runs: Vec<GlyphRun>,
    broken: Vec<BrokenLine>,
    scratch: Vec<ShapedGlyph>,
    glyphs: Vec<PositionedGlyph>,
    runs: Vec<VisualRun>,
    lines: Vec<LayoutLine>,
    carets: Vec<CaretStop>,
}

impl TextWorkspace {
    pub fn try_new(limits: LayoutLimits) -> Result<Self, WorkspaceError> {
        Ok(Self {
            limits,
            bidi: slots(limits.runs, BidiRun::empty())?,
            logical: slots(limits.runs, LogicalRun::empty())?,
            initial_glyphs: slots(limits.glyphs, ShapedGlyph::default())?,
            initial_runs: slots(limits.runs, GlyphRun::empty())?,
            broken: slots(limits.lines, BrokenLine::empty())?,
            scratch: slots(limits.scratch_glyphs, ShapedGlyph::default())?,
            glyphs: slots(limits.glyphs, PositionedGlyph::default())?,
            runs: slots(limits.visual_runs, VisualRun::empty())?,
            lines: slots(limits.lines, LayoutLine::empty())?,
            carets: slots(limits.carets, CaretStop::default())?,
        })
    }

    pub const fn limits(&self) -> LayoutLimits {
        self.limits
    }

    pub(crate) fn layout<'workspace>(
        &'workspace mut self,
        flow: &TextFlow<'_>,
        typefaces: &[&dyn Typeface],
    ) -> Result<ParagraphLayout<'workspace>, WorkspaceError> {
        if flow.text.len() > self.limits.text_bytes {
            return Err(WorkspaceError::TextLimit {
                required: flow.text.len(),
                limit: self.limits.text_bytes,
            });
        }
        let max_width =
            i32::try_from(flow.max_width).map_err(|_| WorkspaceError::DimensionOverflow)?;
        let line_height =
            i32::try_from(flow.line_height).map_err(|_| WorkspaceError::DimensionOverflow)?;
        let line_spacing =
            i32::try_from(flow.line_spacing).map_err(|_| WorkspaceError::DimensionOverflow)?;
        let line_advance = line_height
            .checked_add(line_spacing)
            .ok_or(WorkspaceError::DimensionOverflow)?;
        let bidi = BidiText::resolve(
            flow.text,
            0..flow.text.len(),
            flow.base_direction,
            &mut self.bidi,
        )?;
        let logical = LogicalRuns::resolve(flow.text, &bidi, typefaces, &mut self.logical)?;
        let shaped = logical.shape_into(
            flow.text,
            typefaces,
            flow.features,
            &mut self.initial_glyphs,
            &mut self.initial_runs,
        )?;
        let spacing = TextSpacing {
            letter: flow.letter_spacing,
            word: flow.word_spacing,
        };
        let broken =
            shaped.break_into(flow.text, max_width, flow.wrap, spacing, &mut self.broken)?;
        Ok(logical.layout_into(
            flow.text,
            typefaces,
            flow.features,
            &broken,
            LayoutOptions::new(line_advance)
                .with_origin(flow.origin)
                .with_spacing(spacing)
                .with_width(max_width)
                .with_alignment(flow.alignment)
                .with_direction(bidi.direction()),
            LayoutBuffers::new(
                &mut self.scratch,
                &mut self.glyphs,
                &mut self.runs,
                &mut self.lines,
                &mut self.carets,
            ),
        )?)
    }
}

fn slots<T: Clone>(length: usize, value: T) -> Result<Vec<T>, WorkspaceError> {
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(length)
        .map_err(|_| WorkspaceError::Allocation)?;
    slots.resize(length, value);
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaping::{
        FlowPoint, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource, SimpleTypeface,
    };

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

    fn limits() -> LayoutLimits {
        LayoutLimits {
            text_bytes: 16,
            runs: 4,
            visual_runs: 4,
            glyphs: 8,
            scratch_glyphs: 4,
            lines: 4,
            carets: 12,
        }
    }

    #[test]
    fn reuses_fixed_capacity_for_layout() {
        let limits = limits();
        assert!(limits.buffer_bytes().is_some_and(|bytes| bytes > 0));
        let mut workspace = TextWorkspace::try_new(limits).unwrap();
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab cd", 3)
            .with_line_height(10)
            .with_line_spacing(2);
        let layout = flow.layout(&typefaces, &mut workspace).unwrap();

        assert_eq!(layout.lines().len(), 2);
        assert_eq!(layout.glyphs().len(), 5);
        assert_eq!(layout.lines()[1].origin().y, 12);
    }

    #[test]
    fn rejects_text_before_using_workspace_capacity() {
        let mut workspace = TextWorkspace::try_new(LayoutLimits {
            text_bytes: 2,
            ..limits()
        })
        .unwrap();
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("abc", 3).with_line_height(10);

        assert_eq!(
            flow.layout(&typefaces, &mut workspace).err().unwrap(),
            WorkspaceError::TextLimit {
                required: 3,
                limit: 2,
            }
        );
    }

    #[test]
    fn rejects_dimensions_outside_layout_coordinates() {
        let mut workspace = TextWorkspace::try_new(limits()).unwrap();
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("a", usize::MAX).with_line_height(10);

        assert_eq!(
            flow.layout(&typefaces, &mut workspace).err().unwrap(),
            WorkspaceError::DimensionOverflow
        );
    }
}
