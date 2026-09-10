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
/// Runtime limits for lazily allocated private pipeline storage.
pub struct LayoutLimits {
    pub text_bytes: usize,
    pub runs: usize,
    pub glyphs: usize,
    pub scratch_glyphs: usize,
    pub lines: usize,
    pub memory_bytes: usize,
}

impl LayoutLimits {
    /// Element storage for private buffers at their limits.
    ///
    /// Final output and allocator bookkeeping are not included.
    pub fn buffer_bytes(self) -> Option<usize> {
        let run_bytes = size_of::<BidiRun>()
            .checked_add(size_of::<LogicalRun>())?
            .checked_add(size_of::<GlyphRun>())?
            .checked_mul(self.runs)?;
        let glyph_bytes = size_of::<ShapedGlyph>().checked_mul(self.glyphs)?;
        run_bytes
            .checked_add(glyph_bytes)?
            .checked_add(size_of::<ShapedGlyph>().checked_mul(self.scratch_glyphs)?)?
            .checked_add(size_of::<BrokenLine>().checked_mul(self.lines)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Failure to admit, allocate, or lay out a paragraph with a workspace.
pub enum WorkspaceError {
    Allocation,
    MemoryLimit { required: usize, limit: usize },
    TextLimit { required: usize, limit: usize },
    RunLimit { required: usize, limit: usize },
    GlyphLimit { required: usize, limit: usize },
    ScratchLimit { required: usize, limit: usize },
    LineLimit { required: usize, limit: usize },
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

/// Caller-owned storage for final paragraph products.
pub struct LayoutOutput<'a> {
    glyphs: &'a mut [PositionedGlyph],
    runs: &'a mut [VisualRun],
    lines: &'a mut [LayoutLine],
    carets: &'a mut [CaretStop],
}

impl<'a> LayoutOutput<'a> {
    /// Combines final glyph, run, line, and caret slices into one output target.
    pub fn new(
        glyphs: &'a mut [PositionedGlyph],
        runs: &'a mut [VisualRun],
        lines: &'a mut [LayoutLine],
        carets: &'a mut [CaretStop],
    ) -> Self {
        Self {
            glyphs,
            runs,
            lines,
            carets,
        }
    }

    pub(crate) fn layout(&self, result: LayoutResult) -> ParagraphLayout<'_> {
        ParagraphLayout::from_parts(
            &self.lines[..result.lines],
            &self.runs[..result.runs],
            &self.glyphs[..result.glyphs],
            &self.carets[..result.carets],
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LayoutResult {
    lines: usize,
    runs: usize,
    glyphs: usize,
    carets: usize,
}

/// Reusable heap storage for private paragraph-layout intermediates.
pub struct TextWorkspace {
    limits: LayoutLimits,
    bidi: Vec<BidiRun>,
    logical: Vec<LogicalRun>,
    initial_glyphs: Vec<ShapedGlyph>,
    initial_runs: Vec<GlyphRun>,
    broken: Vec<BrokenLine>,
    scratch: Vec<ShapedGlyph>,
}

impl TextWorkspace {
    /// Creates an empty workspace that grows within `limits` on demand.
    pub const fn new(limits: LayoutLimits) -> Self {
        Self {
            limits,
            bidi: Vec::new(),
            logical: Vec::new(),
            initial_glyphs: Vec::new(),
            initial_runs: Vec::new(),
            broken: Vec::new(),
            scratch: Vec::new(),
        }
    }

    pub const fn limits(&self) -> LayoutLimits {
        self.limits
    }

    /// Updates the private-buffer byte limit without releasing retained capacity.
    pub fn set_memory_limit(&mut self, memory_bytes: usize) -> Result<(), WorkspaceError> {
        let required = self.resident_bytes();
        if required > memory_bytes {
            return Err(WorkspaceError::MemoryLimit {
                required,
                limit: memory_bytes,
            });
        }
        self.limits.memory_bytes = memory_bytes;
        Ok(())
    }

    pub(crate) fn layout_into(
        &mut self,
        flow: &TextFlow<'_>,
        typefaces: &[&dyn Typeface],
        output: &mut LayoutOutput<'_>,
    ) -> Result<LayoutResult, WorkspaceError> {
        if flow.text.len() > self.limits.text_bytes {
            return Err(WorkspaceError::TextLimit {
                required: flow.text.len(),
                limit: self.limits.text_bytes,
            });
        }
        self.prepare(flow.text)?;
        loop {
            match self.try_layout(flow, typefaces, output) {
                Ok(result) => return Ok(result),
                Err(AttemptError::Grow(kind, required)) => self.grow(kind, required)?,
                Err(AttemptError::Public(error)) => return Err(error),
            }
        }
    }

    /// Returns allocated element capacity for private pipeline buffers.
    pub fn resident_bytes(&self) -> usize {
        vec_bytes(&self.bidi)
            .saturating_add(vec_bytes(&self.logical))
            .saturating_add(vec_bytes(&self.initial_glyphs))
            .saturating_add(vec_bytes(&self.initial_runs))
            .saturating_add(vec_bytes(&self.broken))
            .saturating_add(vec_bytes(&self.scratch))
    }

    fn try_layout(
        &mut self,
        flow: &TextFlow<'_>,
        typefaces: &[&dyn Typeface],
        output: &mut LayoutOutput<'_>,
    ) -> Result<LayoutResult, AttemptError> {
        let max_width =
            i32::try_from(flow.max_width).map_err(|_| WorkspaceError::DimensionOverflow)?;
        let width = i32::try_from(flow.width).map_err(|_| WorkspaceError::DimensionOverflow)?;
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
        )
        .map_err(|error| match error {
            BidiError::InsufficientCapacity { required } => {
                AttemptError::Grow(BufferKind::Runs, required)
            }
            error => AttemptError::Public(WorkspaceError::Bidi(error)),
        })?;
        let logical = LogicalRuns::resolve(flow.text, &bidi, typefaces, &mut self.logical)
            .map_err(map_logical)?;
        let shaped = logical
            .shape_into(
                flow.text,
                typefaces,
                flow.features,
                &mut self.initial_glyphs,
                &mut self.initial_runs,
            )
            .map_err(map_shape)?;
        let spacing = TextSpacing {
            letter: flow.letter_spacing,
            word: flow.word_spacing,
        };
        let broken = shaped
            .break_into_with_provider(
                flow.text,
                max_width,
                flow.wrap,
                spacing,
                flow.line_break_provider,
                &mut self.broken,
            )
            .map_err(map_break)?;
        let layout = logical
            .layout_into(
                flow.text,
                typefaces,
                flow.features,
                &broken,
                LayoutOptions::new(line_advance)
                    .with_origin(flow.origin)
                    .with_spacing(spacing)
                    .with_width(width)
                    .with_alignment(flow.alignment)
                    .with_direction(bidi.direction())
                    .with_max_lines(flow.max_lines)
                    .with_overflow(flow.overflow),
                LayoutBuffers::new(
                    &mut self.scratch,
                    &mut *output.glyphs,
                    &mut *output.runs,
                    &mut *output.lines,
                    &mut *output.carets,
                ),
            )
            .map_err(|error| match error {
                LayoutError::InsufficientScratchCapacity { minimum } => {
                    AttemptError::Grow(BufferKind::Scratch, minimum)
                }
                error => AttemptError::Public(WorkspaceError::Layout(error)),
            })?;
        Ok(LayoutResult {
            lines: layout.lines().len(),
            runs: layout.runs().len(),
            glyphs: layout.glyphs().len(),
            carets: layout.carets().len(),
        })
    }

    fn prepare(&mut self, text: &str) -> Result<(), WorkspaceError> {
        let glyphs = text.chars().count();
        let initial = glyphs.min(self.limits.glyphs);
        self.grow(BufferKind::Glyphs, initial)
    }

    fn grow(&mut self, kind: BufferKind, required: usize) -> Result<(), WorkspaceError> {
        let limit = match kind {
            BufferKind::Runs => self.limits.runs,
            BufferKind::Glyphs => self.limits.glyphs,
            BufferKind::Scratch => self.limits.scratch_glyphs,
            BufferKind::Lines => self.limits.lines,
        };
        if required > limit {
            return Err(kind.limit_error(required, limit));
        }
        let projected = self
            .projected_bytes(kind, required)
            .ok_or(WorkspaceError::DimensionOverflow)?;
        if projected > self.limits.memory_bytes {
            return Err(WorkspaceError::MemoryLimit {
                required: projected,
                limit: self.limits.memory_bytes,
            });
        }
        match kind {
            BufferKind::Runs => {
                resize_slots(&mut self.bidi, required, BidiRun::empty())?;
                resize_slots(&mut self.logical, required, LogicalRun::empty())?;
                resize_slots(&mut self.initial_runs, required, GlyphRun::empty())
            }
            BufferKind::Glyphs => {
                resize_slots(&mut self.initial_glyphs, required, ShapedGlyph::default())
            }
            BufferKind::Scratch => {
                resize_slots(&mut self.scratch, required, ShapedGlyph::default())
            }
            BufferKind::Lines => resize_slots(&mut self.broken, required, BrokenLine::empty()),
        }
    }

    fn projected_bytes(&self, kind: BufferKind, required: usize) -> Option<usize> {
        let added = match kind {
            BufferKind::Runs => additional_bytes(&self.bidi, required)?
                .checked_add(additional_bytes(&self.logical, required)?)?
                .checked_add(additional_bytes(&self.initial_runs, required)?)?,
            BufferKind::Glyphs => additional_bytes(&self.initial_glyphs, required)?,
            BufferKind::Scratch => additional_bytes(&self.scratch, required)?,
            BufferKind::Lines => additional_bytes(&self.broken, required)?,
        };
        self.resident_bytes().checked_add(added)
    }
}

#[derive(Clone, Copy)]
enum BufferKind {
    Runs,
    Glyphs,
    Scratch,
    Lines,
}

impl BufferKind {
    const fn limit_error(self, required: usize, limit: usize) -> WorkspaceError {
        match self {
            Self::Runs => WorkspaceError::RunLimit { required, limit },
            Self::Glyphs => WorkspaceError::GlyphLimit { required, limit },
            Self::Scratch => WorkspaceError::ScratchLimit { required, limit },
            Self::Lines => WorkspaceError::LineLimit { required, limit },
        }
    }
}

enum AttemptError {
    Grow(BufferKind, usize),
    Public(WorkspaceError),
}

impl From<WorkspaceError> for AttemptError {
    fn from(error: WorkspaceError) -> Self {
        Self::Public(error)
    }
}

fn map_logical(error: LayoutError) -> AttemptError {
    match error {
        LayoutError::InsufficientRunCapacity { required } => {
            AttemptError::Grow(BufferKind::Runs, required)
        }
        error => AttemptError::Public(WorkspaceError::Layout(error)),
    }
}

fn map_shape(error: LayoutError) -> AttemptError {
    match error {
        LayoutError::InsufficientRunCapacity { required } => {
            AttemptError::Grow(BufferKind::Runs, required)
        }
        LayoutError::InsufficientGlyphCapacity { minimum } => {
            AttemptError::Grow(BufferKind::Glyphs, minimum)
        }
        error => AttemptError::Public(WorkspaceError::Layout(error)),
    }
}

fn map_break(error: LayoutError) -> AttemptError {
    match error {
        LayoutError::InsufficientLineCapacity { required } => {
            AttemptError::Grow(BufferKind::Lines, required)
        }
        error => AttemptError::Public(WorkspaceError::Layout(error)),
    }
}

fn resize_slots<T: Clone>(
    slots: &mut Vec<T>,
    length: usize,
    value: T,
) -> Result<(), WorkspaceError> {
    if length <= slots.len() {
        return Ok(());
    }
    slots
        .try_reserve_exact(length - slots.len())
        .map_err(|_| WorkspaceError::Allocation)?;
    slots.resize(length, value);
    Ok(())
}

fn vec_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(size_of::<T>())
}

fn additional_bytes<T>(values: &Vec<T>, required: usize) -> Option<usize> {
    required
        .saturating_sub(values.capacity())
        .checked_mul(size_of::<T>())
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
            glyphs: 8,
            scratch_glyphs: 4,
            lines: 4,
            memory_bytes: usize::MAX,
        }
    }

    struct OutputStorage {
        glyphs: [PositionedGlyph; 8],
        runs: [VisualRun; 4],
        lines: [LayoutLine; 4],
        carets: [CaretStop; 12],
    }

    impl OutputStorage {
        fn new() -> Self {
            Self {
                glyphs: [PositionedGlyph::default(); 8],
                runs: [VisualRun::empty(); 4],
                lines: [LayoutLine::empty(); 4],
                carets: [CaretStop::default(); 12],
            }
        }

        fn with_layout<R>(
            &mut self,
            flow: &TextFlow<'_>,
            typefaces: &[&dyn Typeface],
            workspace: &mut TextWorkspace,
            inspect: impl FnOnce(ParagraphLayout<'_>) -> R,
        ) -> Result<R, WorkspaceError> {
            let mut output = LayoutOutput::new(
                &mut self.glyphs,
                &mut self.runs,
                &mut self.lines,
                &mut self.carets,
            );
            let layout = flow.layout_into(typefaces, workspace, &mut output)?;
            Ok(inspect(layout))
        }

        fn error(
            &mut self,
            flow: &TextFlow<'_>,
            typefaces: &[&dyn Typeface],
            workspace: &mut TextWorkspace,
        ) -> WorkspaceError {
            let mut output = LayoutOutput::new(
                &mut self.glyphs,
                &mut self.runs,
                &mut self.lines,
                &mut self.carets,
            );
            match flow.layout_into(typefaces, workspace, &mut output) {
                Ok(_) => panic!("layout unexpectedly succeeded"),
                Err(error) => error,
            }
        }
    }

    #[test]
    fn grows_private_buffers_lazily_and_reuses_them() {
        let limits = limits();
        let maximum = limits.buffer_bytes().unwrap();
        let mut workspace = TextWorkspace::new(limits);
        assert_eq!(workspace.resident_bytes(), 0);
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let short = TextFlow::new("a", 3).with_line_height(10);
        let mut output = OutputStorage::new();
        output
            .with_layout(&short, &typefaces, &mut workspace, |_| {})
            .unwrap();
        let short_resident = workspace.resident_bytes();
        assert!(short_resident > 0 && short_resident < maximum);

        let flow = TextFlow::new("ab cd", 3)
            .with_line_height(10)
            .with_line_spacing(2);
        output
            .with_layout(&flow, &typefaces, &mut workspace, |layout| {
                assert_eq!(layout.lines().len(), 2);
                assert_eq!(layout.glyphs().len(), 5);
                assert_eq!(layout.lines()[1].origin().y, 12);
            })
            .unwrap();
        let resident = workspace.resident_bytes();
        assert!(resident > short_resident);
        output
            .with_layout(&flow, &typefaces, &mut workspace, |_| {})
            .unwrap();
        assert_eq!(workspace.resident_bytes(), resident);
    }

    #[test]
    fn empty_paragraph_needs_no_workspace_or_output_storage() {
        let mut workspace = TextWorkspace::new(LayoutLimits {
            text_bytes: 0,
            runs: 0,
            glyphs: 0,
            scratch_glyphs: 0,
            lines: 0,
            memory_bytes: 0,
        });
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let mut glyphs = [];
        let mut runs = [];
        let mut lines = [];
        let mut carets = [];
        let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);

        let layout = TextFlow::new("", 0)
            .layout_into(&typefaces, &mut workspace, &mut output)
            .unwrap();

        assert!(layout.glyphs().is_empty());
        assert!(layout.runs().is_empty());
        assert!(layout.lines().is_empty());
        assert!(layout.carets().is_empty());
        assert_eq!(workspace.resident_bytes(), 0);
    }

    #[test]
    fn reports_caller_output_capacity_without_owning_a_second_copy() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab", 3).with_line_height(10);
        let mut glyphs = [PositionedGlyph::default(); 1];
        let mut runs = [VisualRun::empty(); 2];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 3];
        let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);

        let error = match flow.layout_into(&typefaces, &mut workspace, &mut output) {
            Ok(_) => panic!("layout unexpectedly succeeded"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            WorkspaceError::Layout(LayoutError::InsufficientPositionedCapacity { minimum: 2 })
        );
    }

    #[test]
    fn breaking_and_alignment_widths_are_independent() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab cd", 3)
            .with_width(10)
            .with_line_height(10)
            .with_alignment(crate::layout::Alignment::Center);
        let mut output = OutputStorage::new();

        output
            .with_layout(&flow, &typefaces, &mut workspace, |layout| {
                assert_eq!(layout.lines().len(), 2);
                assert_eq!(layout.lines()[0].origin().x, 3);
                assert_eq!(layout.lines()[1].origin().x, 4);
            })
            .unwrap();
    }

    #[test]
    fn memory_limit_rejects_growth_before_allocation() {
        let mut limits = limits();
        limits.memory_bytes = 0;
        let mut workspace = TextWorkspace::new(limits);
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("a", 3).with_line_height(10);
        let mut output = OutputStorage::new();

        assert!(matches!(
            output.error(&flow, &typefaces, &mut workspace),
            WorkspaceError::MemoryLimit {
                required: _,
                limit: 0
            }
        ));
        assert_eq!(workspace.resident_bytes(), 0);
    }

    #[test]
    fn retained_storage_sets_the_minimum_memory_limit() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab", 3).with_line_height(10);
        let mut output = OutputStorage::new();
        output
            .with_layout(&flow, &typefaces, &mut workspace, |_| {})
            .unwrap();
        let resident = workspace.resident_bytes();

        workspace.set_memory_limit(resident).unwrap();
        assert_eq!(workspace.limits().memory_bytes, resident);
        assert_eq!(
            workspace.set_memory_limit(resident - 1),
            Err(WorkspaceError::MemoryLimit {
                required: resident,
                limit: resident - 1,
            })
        );
        assert_eq!(workspace.limits().memory_bytes, resident);
    }

    #[test]
    fn rejects_text_before_using_workspace_capacity() {
        let mut workspace = TextWorkspace::new(LayoutLimits {
            text_bytes: 2,
            ..limits()
        });
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("abc", 3).with_line_height(10);

        let mut output = OutputStorage::new();
        assert_eq!(
            output.error(&flow, &typefaces, &mut workspace),
            WorkspaceError::TextLimit {
                required: 3,
                limit: 2,
            }
        );
    }

    #[test]
    fn rejects_dimensions_outside_layout_coordinates() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("a", usize::MAX).with_line_height(10);

        let mut output = OutputStorage::new();
        assert_eq!(
            output.error(&flow, &typefaces, &mut workspace),
            WorkspaceError::DimensionOverflow
        );
    }

    #[test]
    fn ellipsis_is_shaped_and_excluded_from_caret_navigation() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab cd", 3)
            .with_line_height(10)
            .with_max_lines(1)
            .with_overflow(crate::layout::Overflow::Ellipsis);
        let mut output = OutputStorage::new();
        output
            .with_layout(&flow, &typefaces, &mut workspace, |layout| {
                assert_eq!(layout.lines().len(), 1);
                assert_eq!(
                    layout.lines()[0].text(),
                    crate::shaping::TextRange::new(0, 2)
                );
                assert_eq!(layout.glyphs().len(), 3);
                assert_eq!(
                    layout.glyphs()[2].glyph_id(),
                    GlyphId::new('\u{2026}' as u16)
                );
                assert!(layout.runs()[1].is_synthetic());
                assert_eq!(
                    layout.runs()[1].text(),
                    crate::shaping::TextRange::new(2, 2)
                );
                assert_eq!(layout.carets().len(), 3);
                assert_eq!(layout.carets().last().unwrap().text_offset, 2);
            })
            .unwrap();
    }

    #[test]
    fn no_wrap_ellipsis_replaces_horizontal_overflow() {
        let mut workspace = TextWorkspace::new(LayoutLimits {
            scratch_glyphs: 5,
            ..limits()
        });
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("abcde", 3)
            .with_line_height(10)
            .with_wrap(crate::layout::WrapMode::NoWrap)
            .with_overflow(crate::layout::Overflow::Ellipsis);
        let mut output = OutputStorage::new();
        output
            .with_layout(&flow, &typefaces, &mut workspace, |layout| {
                assert_eq!(layout.lines().len(), 1);
                assert_eq!(
                    layout.lines()[0].text(),
                    crate::shaping::TextRange::new(0, 2)
                );
                assert_eq!(layout.lines()[0].advance(), 3);
                assert_eq!(layout.glyphs().len(), 3);
                assert_eq!(
                    layout.glyphs()[2].glyph_id(),
                    GlyphId::new('\u{2026}' as u16)
                );
            })
            .unwrap();
    }

    #[test]
    fn rtl_ellipsis_occupies_the_visual_start() {
        let mut workspace = TextWorkspace::new(limits());
        let source = Source;
        let typeface = SimpleTypeface::new(&source);
        let typefaces: [&dyn Typeface; 1] = [&typeface];
        let flow = TextFlow::new("ab cd", 3)
            .with_line_height(10)
            .with_direction(crate::bidi::BaseDirection::RightToLeft)
            .with_max_lines(1)
            .with_overflow(crate::layout::Overflow::Ellipsis);
        let mut output = OutputStorage::new();
        output
            .with_layout(&flow, &typefaces, &mut workspace, |layout| {
                assert!(layout.runs()[0].is_synthetic());
                assert_eq!(
                    layout.glyphs()[0].glyph_id(),
                    GlyphId::new('\u{2026}' as u16)
                );
                assert_eq!(layout.glyphs()[0].origin.x, 0);
                assert_eq!(layout.glyphs()[1].origin.x, 1);
            })
            .unwrap();
    }
}
