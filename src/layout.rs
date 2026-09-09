use crate::bidi::{BidiText, Direction};
use crate::shaping::{
    CaretStop, FlowPoint, FontAccessError, FontFeature, FontId, LineEdges, PositionError,
    PositionedGlyph, ShapeError, ShapeRequest, ShapedGlyph, ShapedRun, TextRange, Typeface,
};
use crate::unicode::{graphemes, line_breaks, script_runs, LineBreakKind, LineBreaks, Script};
use core::iter::Peekable;

use crate::buffer::SliceWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalRun {
    text: TextRange,
    typeface: u16,
    script: Script,
    bidi_level: u8,
}

impl LogicalRun {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            typeface: 0,
            script: Script::Common,
            bidi_level: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn typeface_index(self) -> usize {
        self.typeface as usize
    }

    pub const fn script(self) -> Script {
        self.script
    }

    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    NoTypeface,
    TooManyTypefaces,
    InsufficientRunCapacity { required: usize },
    InsufficientGlyphCapacity { minimum: usize },
    InsufficientLineCapacity { required: usize },
    InsufficientScratchCapacity { minimum: usize },
    InsufficientPositionedCapacity { minimum: usize },
    InsufficientCaretCapacity { minimum: usize },
    InvalidRun,
    InvalidTypefaceOutput { run: usize },
    InvalidWidth,
    InvalidLineHeight,
    CoordinateOverflow,
    Font(FontAccessError),
    Shape { run: usize, error: ShapeError },
}

impl From<FontAccessError> for LayoutError {
    fn from(error: FontAccessError) -> Self {
        Self::Font(error)
    }
}

pub struct LogicalRuns<'a> {
    text: TextRange,
    runs: &'a [LogicalRun],
}

impl<'a> LogicalRuns<'a> {
    pub fn resolve(
        text: &str,
        bidi: &BidiText<'_>,
        typefaces: &[&dyn Typeface],
        output: &'a mut [LogicalRun],
    ) -> Result<Self, LayoutError> {
        if typefaces.is_empty() {
            return Err(LayoutError::NoTypeface);
        }
        if typefaces.len() > u16::MAX as usize {
            return Err(LayoutError::TooManyTypefaces);
        }
        let paragraph = bidi.text();
        if paragraph.end > u32::MAX as usize {
            return Err(LayoutError::InvalidRun);
        }
        let mut writer = LogicalRunWriter::new(output);
        for bidi_run in bidi.logical_runs() {
            let bidi_start = bidi_run.text.start;
            let bidi_end = bidi_run.text.end;
            if bidi_start > bidi_end
                || bidi_end > text.len()
                || !text.is_char_boundary(bidi_start)
                || !text.is_char_boundary(bidi_end)
            {
                return Err(LayoutError::InvalidRun);
            }
            for script_run in script_runs(&text[bidi_start..bidi_end]) {
                let script_start = bidi_start + script_run.text.start;
                let script_end = bidi_start + script_run.text.end;
                for cluster in graphemes(&text[script_start..script_end]) {
                    let start = script_start + cluster.range.start;
                    let end = script_start + cluster.range.end;
                    let typeface = select_typeface(cluster.text, typefaces)?;
                    writer.push(LogicalRun {
                        text: TextRange::new(start as u32, end as u32),
                        typeface,
                        script: script_run.script,
                        bidi_level: bidi_run.level,
                    });
                }
            }
        }
        let runs = writer.finish()?;
        Ok(Self {
            text: TextRange::new(paragraph.start as u32, paragraph.end as u32),
            runs,
        })
    }

    pub const fn text(&self) -> TextRange {
        self.text
    }

    pub const fn runs(&self) -> &[LogicalRun] {
        self.runs
    }

    pub fn shape_into<'output>(
        &self,
        text: &str,
        typefaces: &[&dyn Typeface],
        features: &[FontFeature],
        glyphs: &'output mut [ShapedGlyph],
        runs: &'output mut [GlyphRun],
    ) -> Result<ShapedText<'output>, LayoutError> {
        if runs.len() < self.runs.len() {
            return Err(LayoutError::InsufficientRunCapacity {
                required: self.runs.len(),
            });
        }
        let mut glyph_count = 0;
        for (run_index, logical) in self.runs.iter().enumerate() {
            let Some(typeface) = typefaces.get(logical.typeface_index()) else {
                return Err(LayoutError::InvalidRun);
            };
            let range = logical.text.start as usize..logical.text.end as usize;
            let request = ShapeRequest::new(
                text,
                range,
                crate::bidi::Direction::from_level(logical.bidi_level),
                logical.script,
            )
            .with_features(features);
            let count = match typeface.shape_into(&request, &mut glyphs[glyph_count..]) {
                Ok(count) => count,
                Err(ShapeError::InsufficientCapacity { required }) => {
                    return Err(LayoutError::InsufficientGlyphCapacity {
                        minimum: glyph_count.saturating_add(required),
                    });
                }
                Err(error) => {
                    return Err(LayoutError::Shape {
                        run: run_index,
                        error,
                    });
                }
            };
            let end = glyph_count
                .checked_add(count)
                .ok_or(LayoutError::InvalidRun)?;
            if end > glyphs.len()
                || end > u32::MAX as usize
                || !valid_shaped_output(&request, &glyphs[glyph_count..end])
            {
                return Err(LayoutError::InvalidTypefaceOutput { run: run_index });
            }
            runs[run_index] = GlyphRun {
                text: logical.text,
                glyphs: TextRange::new(glyph_count as u32, end as u32),
                font_id: typeface.id(),
                bidi_level: logical.bidi_level,
            };
            glyph_count = end;
        }
        Ok(ShapedText {
            text: self.text,
            glyphs: &glyphs[..glyph_count],
            runs: &runs[..self.runs.len()],
        })
    }

    pub fn layout_into<'output>(
        &self,
        text: &str,
        typefaces: &[&dyn Typeface],
        features: &[FontFeature],
        broken: &BrokenLines<'_, '_>,
        options: LayoutOptions,
        buffers: LayoutBuffers<'output>,
    ) -> Result<ParagraphLayout<'output>, LayoutError> {
        ParagraphBuilder::new(options, buffers).layout(
            LayoutInput {
                text,
                typefaces,
                features,
                logical: self.runs,
                shaped: broken.shaped,
            },
            broken,
        )
    }
}

#[derive(Clone, Copy)]
struct LayoutInput<'a, 'face> {
    text: &'a str,
    typefaces: &'a [&'face dyn Typeface],
    features: &'a [FontFeature],
    logical: &'a [LogicalRun],
    shaped: &'a ShapedText<'a>,
}

#[derive(Clone, Copy)]
struct SyntheticRun {
    typeface: u16,
    font_id: FontId,
    anchor: u32,
}

struct ParagraphBuilder<'a> {
    options: LayoutOptions,
    buffers: LayoutBuffers<'a>,
    run_count: usize,
    glyph_count: usize,
    caret_count: usize,
}

impl<'output> ParagraphBuilder<'output> {
    fn new(options: LayoutOptions, buffers: LayoutBuffers<'output>) -> Self {
        Self {
            options,
            buffers,
            run_count: 0,
            glyph_count: 0,
            caret_count: 0,
        }
    }

    fn layout(
        mut self,
        input: LayoutInput<'_, '_>,
        broken: &BrokenLines<'_, '_>,
    ) -> Result<ParagraphLayout<'output>, LayoutError> {
        let line_count = broken.lines.len().min(self.options.max_lines);
        let ellipsized = self.options.overflow == Overflow::Ellipsis
            && line_count > 0
            && line_count < broken.lines.len();
        self.preflight(input.logical, broken, line_count, ellipsized)?;
        for (line_index, broken_line) in broken.lines[..line_count].iter().enumerate() {
            if ellipsized && line_index + 1 == line_count {
                self.push_ellipsized_line(input, line_index, *broken_line)?;
            } else {
                self.push_line(input, line_index, *broken_line, None)?;
            }
        }
        Ok(self.finish(line_count))
    }

    fn preflight(
        &self,
        logical: &[LogicalRun],
        broken: &BrokenLines<'_, '_>,
        line_count: usize,
        ellipsized: bool,
    ) -> Result<(), LayoutError> {
        if self.options.line_height < 0 {
            return Err(LayoutError::InvalidLineHeight);
        }
        if self.buffers.lines.len() < line_count {
            return Err(LayoutError::InsufficientLineCapacity {
                required: line_count,
            });
        }
        let required_runs = broken.lines[..line_count]
            .iter()
            .try_fold(0usize, |count, line| {
                let line_runs = logical
                    .iter()
                    .filter(|run| intersection(run.text, line.text).is_some())
                    .count();
                count
                    .checked_add(line_runs)
                    .ok_or(LayoutError::CoordinateOverflow)
            })?
            .checked_add(usize::from(ellipsized))
            .ok_or(LayoutError::CoordinateOverflow)?;
        if required_runs > u32::MAX as usize || line_count > u32::MAX as usize {
            return Err(LayoutError::CoordinateOverflow);
        }
        if self.buffers.runs.len() < required_runs {
            return Err(LayoutError::InsufficientRunCapacity {
                required: required_runs,
            });
        }
        Ok(())
    }

    fn push_line(
        &mut self,
        input: LayoutInput<'_, '_>,
        line_index: usize,
        broken: BrokenLine,
        synthetic: Option<SyntheticRun>,
    ) -> Result<(), LayoutError> {
        let line_run_start = self.run_count;
        self.append_visual_runs(input.logical, input.typefaces, broken)?;
        reorder_visual_runs(&mut self.buffers.runs[line_run_start..self.run_count]);
        if let Some(synthetic) = synthetic {
            self.insert_synthetic_run(line_run_start, synthetic)?;
        }

        let origin = self.line_origin(line_index)?;
        let line_glyph_start = self.glyph_count;
        let line_caret_start = self.caret_count;
        let mut pen = origin;
        let line_run_end = self.run_count;
        for run_index in line_run_start..self.run_count {
            pen = self.shape_run(input, run_index, line_run_end, broken, pen)?;
        }
        let mut advance = pen
            .x
            .checked_sub(origin.x)
            .ok_or(LayoutError::CoordinateOverflow)?;
        let offset = self.alignment_offset(advance)?;
        let aligned_origin = FlowPoint {
            x: origin
                .x
                .checked_add(offset)
                .ok_or(LayoutError::CoordinateOverflow)?,
            y: origin.y,
        };
        self.translate_line(line_glyph_start, line_caret_start, offset)?;
        if self.options.alignment == Alignment::Justify && broken.end == LineEnd::Wrap {
            advance = self.justify_line(
                input.text,
                broken,
                line_glyph_start,
                line_caret_start,
                advance,
            )?;
        }
        self.buffers.lines[line_index] = LayoutLine {
            text: broken.text,
            runs: TextRange::new(line_run_start as u32, self.run_count as u32),
            glyphs: TextRange::new(line_glyph_start as u32, self.glyph_count as u32),
            carets: TextRange::new(line_caret_start as u32, self.caret_count as u32),
            origin: aligned_origin,
            advance,
        };
        Ok(())
    }

    fn push_ellipsized_line(
        &mut self,
        input: LayoutInput<'_, '_>,
        line_index: usize,
        line: BrokenLine,
    ) -> Result<(), LayoutError> {
        const ELLIPSIS: &str = "\u{2026}";

        let typeface = select_typeface(ELLIPSIS, input.typefaces)?;
        let face = input
            .typefaces
            .get(typeface as usize)
            .ok_or(LayoutError::InvalidRun)?;
        let direction = self.options.direction;
        let request = ShapeRequest::new(ELLIPSIS, 0..ELLIPSIS.len(), direction, Script::Common)
            .with_features(input.features)
            .with_line_edges(LineEdges::BOTH);
        let count = match face.shape_into(&request, self.buffers.scratch) {
            Ok(count) => count,
            Err(ShapeError::InsufficientCapacity { required }) => {
                return Err(LayoutError::InsufficientScratchCapacity { minimum: required });
            }
            Err(error) => {
                return Err(LayoutError::Shape {
                    run: self.run_count,
                    error,
                });
            }
        };
        if count > self.buffers.scratch.len()
            || !valid_shaped_output(&request, &self.buffers.scratch[..count])
        {
            return Err(LayoutError::InvalidTypefaceOutput {
                run: self.run_count,
            });
        }
        let suffix_advance =
            self.buffers.scratch[..count]
                .iter()
                .try_fold(0i32, |total, glyph| {
                    total
                        .checked_add(glyph.advance.x)
                        .ok_or(LayoutError::CoordinateOverflow)
                })?;
        let width = self.options.width.unwrap_or(i32::MAX);
        let content_limit = width
            .checked_sub(suffix_advance)
            .and_then(|value| value.checked_sub(self.options.spacing.letter))
            .ok_or(LayoutError::CoordinateOverflow)?
            .max(0);
        let mut cutoff = fit_prefix(
            input.shaped,
            input.text,
            line.text,
            content_limit,
            self.options.spacing,
        )?;
        loop {
            let counts = (self.run_count, self.glyph_count, self.caret_count);
            self.push_line(
                input,
                line_index,
                BrokenLine {
                    text: TextRange::new(line.text.start, cutoff),
                    advance: 0,
                    end: LineEnd::Paragraph,
                },
                Some(SyntheticRun {
                    typeface,
                    font_id: face.id(),
                    anchor: cutoff,
                }),
            )?;
            if self.buffers.lines[line_index].advance <= width || cutoff == line.text.start {
                return Ok(());
            }
            (self.run_count, self.glyph_count, self.caret_count) = counts;
            cutoff = previous_cluster_start(input.shaped, line.text, cutoff)?;
        }
    }

    fn insert_synthetic_run(
        &mut self,
        line_run_start: usize,
        synthetic: SyntheticRun,
    ) -> Result<(), LayoutError> {
        if self.run_count >= self.buffers.runs.len() {
            return Err(LayoutError::InsufficientRunCapacity {
                required: self.run_count.saturating_add(1),
            });
        }
        let index = match self.options.direction {
            Direction::LeftToRight => self.run_count,
            Direction::RightToLeft => line_run_start,
        };
        self.buffers
            .runs
            .copy_within(index..self.run_count, index + 1);
        let face = synthetic.typeface;
        self.buffers.runs[index] = VisualRun {
            text: TextRange::new(synthetic.anchor, synthetic.anchor),
            glyphs: TextRange::new(0, 0),
            font_id: synthetic.font_id,
            typeface_index: face,
            script: Script::Common,
            bidi_level: match self.options.direction {
                Direction::LeftToRight => 0,
                Direction::RightToLeft => 1,
            },
            synthetic: true,
        };
        self.run_count += 1;
        Ok(())
    }

    fn alignment_offset(&self, advance: i32) -> Result<i32, LayoutError> {
        let Some(width) = self.options.width else {
            return Ok(0);
        };
        let remaining = width.saturating_sub(advance).max(0);
        Ok(match (self.options.alignment, self.options.direction) {
            (Alignment::Start, Direction::LeftToRight)
            | (Alignment::End, Direction::RightToLeft)
            | (Alignment::Justify, _) => 0,
            (Alignment::Start, Direction::RightToLeft)
            | (Alignment::End, Direction::LeftToRight) => remaining,
            (Alignment::Center, _) => remaining / 2,
        })
    }

    fn translate_line(
        &mut self,
        glyph_start: usize,
        caret_start: usize,
        offset: i32,
    ) -> Result<(), LayoutError> {
        if offset == 0 {
            return Ok(());
        }
        for glyph in &mut self.buffers.glyphs[glyph_start..self.glyph_count] {
            glyph.origin.x = glyph
                .origin
                .x
                .checked_add(offset)
                .ok_or(LayoutError::CoordinateOverflow)?;
        }
        for caret in &mut self.buffers.carets[caret_start..self.caret_count] {
            caret.position.x = caret
                .position
                .x
                .checked_add(offset)
                .ok_or(LayoutError::CoordinateOverflow)?;
        }
        Ok(())
    }

    fn justify_line(
        &mut self,
        text: &str,
        line: BrokenLine,
        glyph_start: usize,
        caret_start: usize,
        advance: i32,
    ) -> Result<i32, LayoutError> {
        let Some(width) = self.options.width else {
            return Ok(advance);
        };
        let remaining = width.saturating_sub(advance);
        if remaining <= 0 {
            return Ok(advance);
        }
        let trimmed_end = trailing_content_end(text, line.text)?;
        let glyphs = &self.buffers.glyphs[glyph_start..self.glyph_count];
        let gaps = glyphs
            .iter()
            .enumerate()
            .filter(|(index, glyph)| {
                glyphs
                    .get(index + 1)
                    .is_none_or(|next| next.cluster != glyph.cluster)
                    && justification_gap(text, glyph.cluster, trimmed_end)
            })
            .count();
        if gaps == 0 {
            return Ok(advance);
        }
        let gaps = i32::try_from(gaps).map_err(|_| LayoutError::CoordinateOverflow)?;
        let each = remaining / gaps;
        let mut remainder = remaining % gaps;
        let mut shift = 0i32;
        let (glyphs, carets) = (&mut self.buffers.glyphs, &mut self.buffers.carets);
        let glyphs = &mut glyphs[glyph_start..self.glyph_count];
        let carets = &mut carets[caret_start..self.caret_count];
        for index in 0..glyphs.len() {
            glyphs[index].origin.x = glyphs[index]
                .origin
                .x
                .checked_add(shift)
                .ok_or(LayoutError::CoordinateOverflow)?;
            let cluster_end = glyphs
                .get(index + 1)
                .is_none_or(|next| next.cluster != glyphs[index].cluster);
            if cluster_end && justification_gap(text, glyphs[index].cluster, trimmed_end) {
                let extra = each + i32::from(remainder > 0);
                remainder = remainder.saturating_sub(1);
                let boundary = glyphs[index]
                    .origin
                    .x
                    .checked_add(glyphs[index].advance.x)
                    .ok_or(LayoutError::CoordinateOverflow)?;
                glyphs[index].advance.x = glyphs[index]
                    .advance
                    .x
                    .checked_add(extra)
                    .ok_or(LayoutError::CoordinateOverflow)?;
                shift = shift
                    .checked_add(extra)
                    .ok_or(LayoutError::CoordinateOverflow)?;
                for caret in carets
                    .iter_mut()
                    .filter(|caret| caret.position.x >= boundary)
                {
                    caret.position.x = caret
                        .position
                        .x
                        .checked_add(extra)
                        .ok_or(LayoutError::CoordinateOverflow)?;
                }
            }
        }
        advance
            .checked_add(shift)
            .ok_or(LayoutError::CoordinateOverflow)
    }

    fn append_visual_runs(
        &mut self,
        logical: &[LogicalRun],
        typefaces: &[&dyn Typeface],
        broken: BrokenLine,
    ) -> Result<(), LayoutError> {
        for logical in logical {
            let Some(text) = intersection(logical.text, broken.text) else {
                continue;
            };
            let Some(typeface) = typefaces.get(logical.typeface_index()) else {
                return Err(LayoutError::InvalidRun);
            };
            self.buffers.runs[self.run_count] = VisualRun {
                text,
                glyphs: TextRange::new(0, 0),
                font_id: typeface.id(),
                typeface_index: logical.typeface,
                script: logical.script,
                bidi_level: logical.bidi_level,
                synthetic: false,
            };
            self.run_count += 1;
        }
        Ok(())
    }

    fn line_origin(&self, line_index: usize) -> Result<FlowPoint, LayoutError> {
        let line = i32::try_from(line_index).map_err(|_| LayoutError::CoordinateOverflow)?;
        let y = self
            .options
            .line_height
            .checked_mul(line)
            .and_then(|offset| self.options.origin.y.checked_add(offset))
            .ok_or(LayoutError::CoordinateOverflow)?;
        Ok(FlowPoint {
            x: self.options.origin.x,
            y,
        })
    }

    fn shape_run(
        &mut self,
        input: LayoutInput<'_, '_>,
        run_index: usize,
        line_run_end: usize,
        line: BrokenLine,
        origin: FlowPoint,
    ) -> Result<FlowPoint, LayoutError> {
        let mut visual = self.buffers.runs[run_index];
        let Some(typeface) = input.typefaces.get(visual.typeface_index as usize) else {
            return Err(LayoutError::InvalidRun);
        };
        const ELLIPSIS: &str = "\u{2026}";
        let (text, range, edges) = if visual.synthetic {
            (ELLIPSIS, 0..ELLIPSIS.len(), LineEdges::BOTH)
        } else {
            (
                input.text,
                visual.text.start as usize..visual.text.end as usize,
                line_edges(visual.text, line.text),
            )
        };
        let request = ShapeRequest::new(
            text,
            range,
            crate::bidi::Direction::from_level(visual.bidi_level),
            visual.script,
        )
        .with_features(input.features)
        .with_line_edges(edges);
        let count = match typeface.shape_into(&request, self.buffers.scratch) {
            Ok(count) => count,
            Err(ShapeError::InsufficientCapacity { required }) => {
                return Err(LayoutError::InsufficientScratchCapacity { minimum: required });
            }
            Err(error) => {
                return Err(LayoutError::Shape {
                    run: run_index,
                    error,
                });
            }
        };
        if count > self.buffers.scratch.len()
            || !valid_shaped_output(&request, &self.buffers.scratch[..count])
        {
            return Err(LayoutError::InvalidTypefaceOutput { run: run_index });
        }
        apply_spacing(
            text,
            run_index + 1 < line_run_end,
            self.options.spacing,
            &mut self.buffers.scratch[..count],
        )?;
        if visual.synthetic {
            for glyph in &mut self.buffers.scratch[..count] {
                glyph.cluster = visual.text;
            }
        }
        let glyph_end = self
            .glyph_count
            .checked_add(count)
            .ok_or(LayoutError::CoordinateOverflow)?;
        if glyph_end > u32::MAX as usize {
            return Err(LayoutError::CoordinateOverflow);
        }
        if glyph_end > self.buffers.glyphs.len() {
            return Err(LayoutError::InsufficientPositionedCapacity { minimum: glyph_end });
        }
        let shaped = ShapedRun::new(
            visual.font_id,
            visual.text,
            visual.bidi_level,
            &self.buffers.scratch[..count],
        );
        let positioned = shaped
            .position_into(
                origin,
                &mut self.buffers.glyphs[self.glyph_count..glyph_end],
            )
            .map_err(|error| position_error(error, self.glyph_count))?;
        let end = positioned.end();
        if !visual.synthetic {
            let carets = positioned
                .carets_into(&mut self.buffers.carets[self.caret_count..])
                .map_err(|error| caret_error(error, self.caret_count))?;
            self.caret_count = self
                .caret_count
                .checked_add(carets.len())
                .ok_or(LayoutError::CoordinateOverflow)?;
        }
        if self.caret_count > u32::MAX as usize {
            return Err(LayoutError::CoordinateOverflow);
        }
        visual.glyphs = TextRange::new(self.glyph_count as u32, glyph_end as u32);
        self.buffers.runs[run_index] = visual;
        self.glyph_count = glyph_end;
        Ok(end)
    }

    fn finish(self, line_count: usize) -> ParagraphLayout<'output> {
        ParagraphLayout {
            lines: &self.buffers.lines[..line_count],
            runs: &self.buffers.runs[..self.run_count],
            glyphs: &self.buffers.glyphs[..self.glyph_count],
            carets: &self.buffers.carets[..self.caret_count],
        }
    }
}

fn line_edges(run: TextRange, line: TextRange) -> LineEdges {
    match (run.start == line.start, run.end == line.end) {
        (true, true) => LineEdges::BOTH,
        (true, false) => LineEdges::START,
        (false, true) => LineEdges::END,
        (false, false) => LineEdges::NONE,
    }
}

fn spacing_after(
    text: &str,
    cluster: TextRange,
    paragraph: TextRange,
    spacing: TextSpacing,
) -> Result<i32, LayoutError> {
    if cluster.end >= paragraph.end {
        return Ok(0);
    }
    cluster_spacing(text, cluster, spacing)
}

fn apply_spacing(
    text: &str,
    has_following_run: bool,
    spacing: TextSpacing,
    glyphs: &mut [ShapedGlyph],
) -> Result<(), LayoutError> {
    let mut start = 0;
    while start < glyphs.len() {
        let cluster = glyphs[start].cluster;
        let mut end = start + 1;
        while end < glyphs.len() && glyphs[end].cluster == cluster {
            end += 1;
        }
        if end < glyphs.len() || has_following_run {
            let extra = cluster_spacing(text, cluster, spacing)?;
            glyphs[end - 1].advance.x = glyphs[end - 1]
                .advance
                .x
                .checked_add(extra)
                .ok_or(LayoutError::CoordinateOverflow)?;
        }
        start = end;
    }
    Ok(())
}

fn cluster_spacing(
    text: &str,
    cluster: TextRange,
    spacing: TextSpacing,
) -> Result<i32, LayoutError> {
    let value = text
        .get(cluster.start as usize..cluster.end as usize)
        .ok_or(LayoutError::InvalidRun)?;
    if value
        .chars()
        .any(|character| matches!(character, '\r' | '\n'))
    {
        return Ok(0);
    }
    let word = if !value.is_empty() && value.chars().all(char::is_whitespace) {
        spacing.word
    } else {
        0
    };
    spacing
        .letter
        .checked_add(word)
        .ok_or(LayoutError::CoordinateOverflow)
}

fn trailing_content_end(text: &str, line: TextRange) -> Result<u32, LayoutError> {
    let content = text
        .get(line.start as usize..line.end as usize)
        .ok_or(LayoutError::InvalidRun)?;
    let trimmed = content.trim_end_matches(char::is_whitespace);
    let end = line.start as usize + trimmed.len();
    u32::try_from(end).map_err(|_| LayoutError::CoordinateOverflow)
}

fn justification_gap(text: &str, cluster: TextRange, content_end: u32) -> bool {
    cluster.end <= content_end
        && text
            .get(cluster.start as usize..cluster.end as usize)
            .is_some_and(|value| !value.is_empty() && value.chars().all(char::is_whitespace))
}

fn fit_prefix(
    shaped: &ShapedText<'_>,
    text: &str,
    line: TextRange,
    max_width: i32,
    spacing: TextSpacing,
) -> Result<u32, LayoutError> {
    let mut glyphs = LogicalGlyphs::new(shaped).peekable();
    let mut width = 0i32;
    let mut cutoff = line.start;
    while let Some(first) = glyphs.next() {
        let cluster = first.cluster;
        let mut advance = first.advance.x;
        while let Some(next) = glyphs.peek() {
            if next.cluster != cluster {
                break;
            }
            advance = advance
                .checked_add(next.advance.x)
                .ok_or(LayoutError::CoordinateOverflow)?;
            glyphs.next();
        }
        if cluster.start < line.start || cluster.end > line.end {
            continue;
        }
        advance = advance
            .checked_add(cluster_spacing(text, cluster, spacing)?)
            .ok_or(LayoutError::CoordinateOverflow)?;
        let next = width
            .checked_add(advance)
            .ok_or(LayoutError::CoordinateOverflow)?;
        if next > max_width {
            break;
        }
        width = next;
        let content = text
            .get(cluster.start as usize..cluster.end as usize)
            .ok_or(LayoutError::InvalidRun)?;
        if !content.chars().all(char::is_whitespace) && shaped.is_safe_break(cluster.end) {
            cutoff = cluster.end;
        }
    }
    Ok(cutoff)
}

fn previous_cluster_start(
    shaped: &ShapedText<'_>,
    line: TextRange,
    cutoff: u32,
) -> Result<u32, LayoutError> {
    let mut previous = line.start;
    for glyph in LogicalGlyphs::new(shaped) {
        let cluster = glyph.cluster;
        if cluster.start < line.start || cluster.end > line.end || cluster.end >= cutoff {
            continue;
        }
        if shaped.is_safe_break(cluster.end) {
            previous = previous.max(cluster.end);
        }
    }
    Ok(previous)
}

struct LogicalRunWriter<'a> {
    output: SliceWriter<'a, LogicalRun>,
    last: Option<LogicalRun>,
}

impl<'a> LogicalRunWriter<'a> {
    fn new(output: &'a mut [LogicalRun]) -> Self {
        Self {
            output: SliceWriter::new(output),
            last: None,
        }
    }

    fn push(&mut self, run: LogicalRun) {
        if let Some(previous) = self.last.as_mut() {
            if previous.text.end == run.text.start
                && previous.typeface == run.typeface
                && previous.script == run.script
                && previous.bidi_level == run.bidi_level
            {
                previous.text.end = run.text.end;
                if let Some(stored) = self.output.last_mut() {
                    stored.text.end = run.text.end;
                }
                return;
            }
        }
        self.output.push(run);
        self.last = Some(run);
    }

    fn finish(self) -> Result<&'a [LogicalRun], LayoutError> {
        self.output
            .finish()
            .map_err(|required| LayoutError::InsufficientRunCapacity { required })
    }
}

fn select_typeface(grapheme: &str, typefaces: &[&dyn Typeface]) -> Result<u16, FontAccessError> {
    for (index, typeface) in typefaces.iter().enumerate() {
        if typeface.covers(grapheme)? {
            return Ok(index as u16);
        }
    }
    Ok(0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlyphRun {
    text: TextRange,
    glyphs: TextRange,
    font_id: FontId,
    bidi_level: u8,
}

impl GlyphRun {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            glyphs: TextRange::new(0, 0),
            font_id: FontId::new(0),
            bidi_level: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn glyphs(self) -> TextRange {
        self.glyphs
    }

    pub const fn font_id(self) -> FontId {
        self.font_id
    }

    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

pub struct ShapedText<'a> {
    text: TextRange,
    glyphs: &'a [ShapedGlyph],
    runs: &'a [GlyphRun],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WrapMode {
    NoWrap,
    #[default]
    Word,
    Grapheme,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextSpacing {
    pub letter: i32,
    pub word: i32,
}

impl ShapedText<'_> {
    pub const fn text(&self) -> TextRange {
        self.text
    }

    pub const fn glyphs(&self) -> &[ShapedGlyph] {
        self.glyphs
    }

    pub const fn runs(&self) -> &[GlyphRun] {
        self.runs
    }

    pub fn run(&self, index: usize) -> Option<ShapedRun<'_>> {
        let run = *self.runs.get(index)?;
        let glyphs = self
            .glyphs
            .get(run.glyphs.start as usize..run.glyphs.end as usize)?;
        Some(ShapedRun::new(
            run.font_id,
            run.text,
            run.bidi_level,
            glyphs,
        ))
    }

    pub fn is_safe_break(&self, offset: u32) -> bool {
        if offset == self.text.start || offset == self.text.end {
            return true;
        }
        if offset < self.text.start || offset > self.text.end {
            return false;
        }
        !self.glyphs.iter().any(|glyph| {
            glyph.cluster.start < offset && offset < glyph.cluster.end
                || glyph.cluster.start == offset && glyph.unsafe_to_break()
        })
    }

    pub fn break_into<'shaped, 'output>(
        &'shaped self,
        text: &str,
        max_width: i32,
        mode: WrapMode,
        spacing: TextSpacing,
        output: &'output mut [BrokenLine],
    ) -> Result<BrokenLines<'shaped, 'output>, LayoutError> {
        if max_width < 0 {
            return Err(LayoutError::InvalidWidth);
        }
        let start = self.text.start as usize;
        let end = self.text.end as usize;
        if start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(LayoutError::InvalidRun);
        }
        let breaks = line_breaks(&text[start..end]).peekable();
        let mut breaker = LineBreaker::new(self, self.text, max_width, mode, breaks, output);
        let mut glyphs = LogicalGlyphs::new(self).peekable();
        while let Some(first) = glyphs.next() {
            let cluster = first.cluster;
            let mut advance = first.advance.x;
            while let Some(next) = glyphs.peek() {
                if next.cluster != cluster {
                    break;
                }
                advance = advance
                    .checked_add(next.advance.x)
                    .ok_or(LayoutError::CoordinateOverflow)?;
                glyphs.next();
            }
            advance = advance
                .checked_add(spacing_after(text, cluster, self.text, spacing)?)
                .ok_or(LayoutError::CoordinateOverflow)?;
            breaker.add_cluster(cluster, advance)?;
        }
        breaker.finish()
    }
}

struct LogicalGlyphs<'a> {
    shaped: &'a ShapedText<'a>,
    run: usize,
    glyph: usize,
}

impl<'a> LogicalGlyphs<'a> {
    fn new(shaped: &'a ShapedText<'a>) -> Self {
        Self {
            shaped,
            run: 0,
            glyph: 0,
        }
    }
}

impl<'a> Iterator for LogicalGlyphs<'a> {
    type Item = &'a ShapedGlyph;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let run = self.shaped.runs.get(self.run)?;
            let start = run.glyphs.start as usize;
            let end = run.glyphs.end as usize;
            if self.glyph < end.saturating_sub(start) {
                let index = if run.bidi_level & 1 == 0 {
                    start + self.glyph
                } else {
                    end - self.glyph - 1
                };
                self.glyph += 1;
                return self.shaped.glyphs.get(index);
            }
            self.run += 1;
            self.glyph = 0;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BrokenLine {
    text: TextRange,
    advance: i32,
    end: LineEnd,
}

impl BrokenLine {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            advance: 0,
            end: LineEnd::Paragraph,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn advance(self) -> i32 {
        self.advance
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum LineEnd {
    Wrap,
    Mandatory,
    #[default]
    Paragraph,
}

pub struct BrokenLines<'shaped, 'output> {
    shaped: &'shaped ShapedText<'shaped>,
    lines: &'output [BrokenLine],
}

impl BrokenLines<'_, '_> {
    pub const fn lines(&self) -> &[BrokenLine] {
        self.lines
    }
}

struct LineBreaker<'shaped, 'glyphs, 'text, 'output> {
    shaped: &'shaped ShapedText<'glyphs>,
    paragraph: TextRange,
    max_width: i32,
    mode: WrapMode,
    breaks: Peekable<LineBreaks<'text>>,
    writer: LineWriter<'output>,
    line_start: u32,
    width: i32,
    last_allowed: Option<(u32, i32)>,
}

impl<'shaped, 'glyphs, 'text, 'output> LineBreaker<'shaped, 'glyphs, 'text, 'output> {
    fn new(
        shaped: &'shaped ShapedText<'glyphs>,
        paragraph: TextRange,
        max_width: i32,
        mode: WrapMode,
        breaks: Peekable<LineBreaks<'text>>,
        output: &'output mut [BrokenLine],
    ) -> Self {
        Self {
            shaped,
            paragraph,
            max_width,
            mode,
            breaks,
            writer: LineWriter::new(output),
            line_start: paragraph.start,
            width: 0,
            last_allowed: None,
        }
    }

    fn add_cluster(&mut self, cluster: TextRange, advance: i32) -> Result<(), LayoutError> {
        self.take_breaks_through(cluster.start)?;
        let width_before = self.width;
        self.width = self
            .width
            .checked_add(advance)
            .ok_or(LayoutError::CoordinateOverflow)?;
        if self.width > self.max_width {
            match self.mode {
                WrapMode::NoWrap => {}
                WrapMode::Word => {
                    if let Some((offset, width)) = self.last_allowed.take() {
                        self.emit(offset, width, LineEnd::Wrap)?;
                    } else if width_before > 0
                        && cluster.start > self.line_start
                        && self.shaped.is_safe_break(cluster.start)
                    {
                        self.emit(cluster.start, width_before, LineEnd::Wrap)?;
                    } else if self.line_start == cluster.start
                        && self.shaped.is_safe_break(cluster.end)
                    {
                        self.emit(cluster.end, self.width, LineEnd::Wrap)?;
                    }
                }
                WrapMode::Grapheme if width_before > 0 && cluster.start > self.line_start => {
                    self.emit(cluster.start, width_before, LineEnd::Wrap)?;
                }
                WrapMode::Grapheme if self.line_start == cluster.start => {
                    self.emit(cluster.end, self.width, LineEnd::Wrap)?;
                }
                WrapMode::Grapheme => {}
            }
        }
        self.take_breaks_through(cluster.end)
    }

    fn take_breaks_through(&mut self, offset: u32) -> Result<(), LayoutError> {
        while self
            .breaks
            .peek()
            .is_some_and(|next| self.paragraph.start as usize + next.offset <= offset as usize)
        {
            let next = self.breaks.next().unwrap();
            let global = self.paragraph.start + next.offset as u32;
            if global <= self.line_start || !self.shaped.is_safe_break(global) {
                continue;
            }
            match next.kind {
                LineBreakKind::Mandatory => {
                    let end = if global == self.paragraph.end {
                        LineEnd::Paragraph
                    } else {
                        LineEnd::Mandatory
                    };
                    self.emit(global, self.width, end)?;
                }
                LineBreakKind::Allowed
                    if self.mode == WrapMode::Word && self.width > self.max_width =>
                {
                    self.emit(global, self.width, LineEnd::Wrap)?;
                }
                LineBreakKind::Allowed if self.mode == WrapMode::Word => {
                    self.last_allowed = Some((global, self.width));
                }
                LineBreakKind::Allowed => {}
            }
        }
        Ok(())
    }

    fn emit(&mut self, end: u32, advance: i32, line_end: LineEnd) -> Result<(), LayoutError> {
        if end <= self.line_start {
            return Ok(());
        }
        self.writer.push(BrokenLine {
            text: TextRange::new(self.line_start, end),
            advance,
            end: line_end,
        });
        self.line_start = end;
        self.width = self
            .width
            .checked_sub(advance)
            .ok_or(LayoutError::CoordinateOverflow)?;
        self.last_allowed = None;
        Ok(())
    }

    fn finish(mut self) -> Result<BrokenLines<'shaped, 'output>, LayoutError> {
        self.take_breaks_through(self.paragraph.end)?;
        if self.line_start < self.paragraph.end {
            self.emit(self.paragraph.end, self.width, LineEnd::Paragraph)?;
        }
        let lines = self.writer.finish()?;
        Ok(BrokenLines {
            shaped: self.shaped,
            lines,
        })
    }
}

struct LineWriter<'a> {
    output: SliceWriter<'a, BrokenLine>,
}

impl<'a> LineWriter<'a> {
    fn new(output: &'a mut [BrokenLine]) -> Self {
        Self {
            output: SliceWriter::new(output),
        }
    }

    fn push(&mut self, line: BrokenLine) {
        self.output.push(line);
    }

    fn finish(self) -> Result<&'a [BrokenLine], LayoutError> {
        self.output
            .finish()
            .map_err(|required| LayoutError::InsufficientLineCapacity { required })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutOptions {
    pub origin: FlowPoint,
    pub line_height: i32,
    pub spacing: TextSpacing,
    pub width: Option<i32>,
    pub alignment: Alignment,
    pub direction: Direction,
    pub max_lines: usize,
    pub overflow: Overflow,
}

impl LayoutOptions {
    pub const fn new(line_height: i32) -> Self {
        Self {
            origin: FlowPoint { x: 0, y: 0 },
            line_height,
            spacing: TextSpacing { letter: 0, word: 0 },
            width: None,
            alignment: Alignment::Start,
            direction: Direction::LeftToRight,
            max_lines: usize::MAX,
            overflow: Overflow::Clip,
        }
    }

    pub const fn with_origin(mut self, origin: FlowPoint) -> Self {
        self.origin = origin;
        self
    }

    pub const fn with_spacing(mut self, spacing: TextSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub const fn with_width(mut self, width: i32) -> Self {
        self.width = Some(width);
        self
    }

    pub const fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub const fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub const fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = max_lines;
        self
    }

    pub const fn with_overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Justify,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Overflow {
    #[default]
    Clip,
    Ellipsis,
}

pub struct LayoutBuffers<'a> {
    pub scratch: &'a mut [ShapedGlyph],
    pub glyphs: &'a mut [PositionedGlyph],
    pub runs: &'a mut [VisualRun],
    pub lines: &'a mut [LayoutLine],
    pub carets: &'a mut [CaretStop],
}

impl<'a> LayoutBuffers<'a> {
    pub fn new(
        scratch: &'a mut [ShapedGlyph],
        glyphs: &'a mut [PositionedGlyph],
        runs: &'a mut [VisualRun],
        lines: &'a mut [LayoutLine],
        carets: &'a mut [CaretStop],
    ) -> Self {
        Self {
            scratch,
            glyphs,
            runs,
            lines,
            carets,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualRun {
    text: TextRange,
    glyphs: TextRange,
    font_id: FontId,
    typeface_index: u16,
    script: Script,
    bidi_level: u8,
    synthetic: bool,
}

impl VisualRun {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            glyphs: TextRange::new(0, 0),
            font_id: FontId::new(0),
            typeface_index: 0,
            script: Script::Common,
            bidi_level: 0,
            synthetic: false,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn glyphs(self) -> TextRange {
        self.glyphs
    }

    pub const fn font_id(self) -> FontId {
        self.font_id
    }

    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }

    pub const fn is_synthetic(self) -> bool {
        self.synthetic
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutLine {
    text: TextRange,
    runs: TextRange,
    glyphs: TextRange,
    carets: TextRange,
    origin: FlowPoint,
    advance: i32,
}

impl LayoutLine {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            runs: TextRange::new(0, 0),
            glyphs: TextRange::new(0, 0),
            carets: TextRange::new(0, 0),
            origin: FlowPoint { x: 0, y: 0 },
            advance: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn runs(self) -> TextRange {
        self.runs
    }

    pub const fn glyphs(self) -> TextRange {
        self.glyphs
    }

    pub const fn carets(self) -> TextRange {
        self.carets
    }

    pub const fn origin(self) -> FlowPoint {
        self.origin
    }

    pub const fn advance(self) -> i32 {
        self.advance
    }
}

pub struct ParagraphLayout<'a> {
    lines: &'a [LayoutLine],
    runs: &'a [VisualRun],
    glyphs: &'a [PositionedGlyph],
    carets: &'a [CaretStop],
}

impl ParagraphLayout<'_> {
    pub const fn lines(&self) -> &[LayoutLine] {
        self.lines
    }

    pub const fn runs(&self) -> &[VisualRun] {
        self.runs
    }

    pub const fn glyphs(&self) -> &[PositionedGlyph] {
        self.glyphs
    }

    pub const fn carets(&self) -> &[CaretStop] {
        self.carets
    }
}

fn intersection(left: TextRange, right: TextRange) -> Option<TextRange> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(TextRange::new(start, end))
}

fn valid_shaped_output(request: &ShapeRequest<'_>, glyphs: &[ShapedGlyph]) -> bool {
    let mut previous = None;
    glyphs.iter().all(|glyph| {
        let start = glyph.cluster.start as usize;
        let end = glyph.cluster.end as usize;
        let ordered = match (request.direction, previous) {
            (_, None) => true,
            (crate::bidi::Direction::LeftToRight, Some(previous)) => {
                glyph.cluster.start >= previous
            }
            (crate::bidi::Direction::RightToLeft, Some(previous)) => {
                glyph.cluster.start <= previous
            }
        };
        previous = Some(glyph.cluster.start);
        start < end
            && start >= request.range.start
            && end <= request.range.end
            && request.text.is_char_boundary(start)
            && request.text.is_char_boundary(end)
            && ordered
    })
}

fn reorder_visual_runs(runs: &mut [VisualRun]) {
    let max_level = runs.iter().map(|run| run.bidi_level).max().unwrap_or(0);
    let min_odd = runs
        .iter()
        .filter(|run| run.bidi_level & 1 == 1)
        .map(|run| run.bidi_level)
        .min();
    let Some(min_odd) = min_odd else {
        return;
    };
    for level in (min_odd..=max_level).rev() {
        let mut start = 0;
        while start < runs.len() {
            if runs[start].bidi_level < level {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < runs.len() && runs[end].bidi_level >= level {
                end += 1;
            }
            runs[start..end].reverse();
            start = end;
        }
    }
}

fn position_error(error: PositionError, current: usize) -> LayoutError {
    match error {
        PositionError::InsufficientCapacity { required } => {
            LayoutError::InsufficientPositionedCapacity {
                minimum: current.saturating_add(required),
            }
        }
        PositionError::CoordinateOverflow => LayoutError::CoordinateOverflow,
    }
}

fn caret_error(error: PositionError, current: usize) -> LayoutError {
    match error {
        PositionError::InsufficientCapacity { required } => {
            LayoutError::InsufficientCaretCapacity {
                minimum: current.saturating_add(required),
            }
        }
        PositionError::CoordinateOverflow => LayoutError::CoordinateOverflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidi::{BaseDirection, BidiRun};
    use crate::shaping::{FlowPoint, FontId, FontMetrics, GlyphId, GlyphSource, SimpleTypeface};
    use std::prelude::v1::*;

    struct Source {
        key: u64,
        ascii: bool,
    }

    impl GlyphSource for Source {
        fn id(&self) -> FontId {
            FontId::new(self.key)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics::default())
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
            let covered = character.is_ascii() == self.ascii;
            Ok(covered.then_some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint { x: 1, y: 0 })
        }
    }

    struct EdgeTypeface;

    impl Typeface for EdgeTypeface {
        fn id(&self) -> FontId {
            FontId::new(9)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics::default())
        }

        fn covers(&self, _grapheme: &str) -> Result<bool, FontAccessError> {
            Ok(true)
        }

        fn shape_into(
            &self,
            request: &ShapeRequest<'_>,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, ShapeError> {
            let shaped = &request.text[request.range.clone()];
            let required = shaped.chars().count();
            if output.len() < required {
                return Err(ShapeError::InsufficientCapacity { required });
            }
            let advance = if request.line_edges.has_start() || request.line_edges.has_end() {
                2
            } else {
                1
            };
            for (index, (offset, character)) in shaped.char_indices().enumerate() {
                let start = request.range.start + offset;
                let end = start + character.len_utf8();
                output[index] = ShapedGlyph::new(
                    GlyphId::new(character as u16),
                    TextRange::new(start as u32, end as u32),
                );
                output[index].advance.x = advance;
            }
            Ok(required)
        }
    }

    struct InvalidTypeface;

    impl Typeface for InvalidTypeface {
        fn id(&self) -> FontId {
            FontId::new(10)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics::default())
        }

        fn covers(&self, _grapheme: &str) -> Result<bool, FontAccessError> {
            Ok(true)
        }

        fn shape_into(
            &self,
            _request: &ShapeRequest<'_>,
            output: &mut [ShapedGlyph],
        ) -> Result<usize, ShapeError> {
            Ok(output.len() + 1)
        }
    }

    fn bidi_slots<const N: usize>() -> [BidiRun; N] {
        core::array::from_fn(|_| BidiRun::empty())
    }

    #[test]
    fn splits_runs_by_script_and_fallback_face() {
        let text = "ab世界cd";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let fallback = SimpleTypeface::new(&Source {
            key: 2,
            ascii: false,
        });
        let typefaces: [&dyn Typeface; 2] = [&primary, &fallback];
        let mut bidi_output = bidi_slots::<4>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut output = [LogicalRun::empty(); 4];
        let runs = LogicalRuns::resolve(text, &bidi, &typefaces, &mut output).unwrap();

        assert_eq!(runs.runs().len(), 3);
        assert_eq!(runs.runs()[0].typeface_index(), 0);
        assert_eq!(runs.runs()[1].typeface_index(), 1);
        assert_eq!(runs.runs()[1].script(), Script::Han);
        assert_eq!(runs.runs()[2].typeface_index(), 0);
    }

    #[test]
    fn shapes_resolved_runs_into_caller_storage() {
        let text = "ab世界";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let fallback = SimpleTypeface::new(&Source {
            key: 2,
            ascii: false,
        });
        let typefaces: [&dyn Typeface; 2] = [&primary, &fallback];
        let mut bidi_output = bidi_slots::<4>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 4];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut glyph_output = [ShapedGlyph::default(); 4];
        let mut run_output = [GlyphRun::empty(); 4];
        let shaped = logical
            .shape_into(text, &typefaces, &[], &mut glyph_output, &mut run_output)
            .unwrap();

        assert_eq!(shaped.glyphs().len(), 4);
        assert_eq!(shaped.runs().len(), 2);
        assert_eq!(shaped.run(0).unwrap().font_id(), FontId::new(1));
        assert_eq!(shaped.run(1).unwrap().font_id(), FontId::new(2));
    }

    #[test]
    fn reports_exact_run_and_minimum_glyph_capacity() {
        let text = "abc";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut glyph_output = [ShapedGlyph::default(); 2];
        let mut run_output = [GlyphRun::empty(); 1];

        assert_eq!(
            logical
                .shape_into(text, &typefaces, &[], &mut glyph_output, &mut run_output,)
                .err()
                .unwrap(),
            LayoutError::InsufficientGlyphCapacity { minimum: 3 }
        );
    }

    #[test]
    fn run_capacity_is_exact_when_no_slots_are_available() {
        let text = "abc";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut output = [];

        assert_eq!(
            LogicalRuns::resolve(text, &bidi, &typefaces, &mut output)
                .err()
                .unwrap(),
            LayoutError::InsufficientRunCapacity { required: 1 }
        );
    }

    #[test]
    fn breaks_at_the_last_allowed_width() {
        let text = "ab cd";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut glyph_output = [ShapedGlyph::default(); 5];
        let mut run_output = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(text, &typefaces, &[], &mut glyph_output, &mut run_output)
            .unwrap();
        let mut line_output = [BrokenLine::empty(); 2];
        let lines = shaped
            .break_into(
                text,
                3,
                WrapMode::Word,
                TextSpacing::default(),
                &mut line_output,
            )
            .unwrap();

        assert_eq!(lines.lines().len(), 2);
        assert_eq!(lines.lines()[0].text(), TextRange::new(0, 3));
        assert_eq!(lines.lines()[0].advance(), 3);
        assert_eq!(lines.lines()[1].text(), TextRange::new(3, 5));
        assert_eq!(lines.lines()[1].advance(), 2);

        let mut no_lines = [];
        assert_eq!(
            shaped
                .break_into(
                    text,
                    3,
                    WrapMode::Word,
                    TextSpacing::default(),
                    &mut no_lines,
                )
                .err()
                .unwrap(),
            LayoutError::InsufficientLineCapacity { required: 2 }
        );
    }

    #[test]
    fn wrap_mode_controls_optional_breaks() {
        let text = "ab cd";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut glyph_output = [ShapedGlyph::default(); 5];
        let mut run_output = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(text, &typefaces, &[], &mut glyph_output, &mut run_output)
            .unwrap();
        let mut line_output = [BrokenLine::empty(); 2];

        let word = shaped
            .break_into(
                text,
                4,
                WrapMode::Word,
                TextSpacing::default(),
                &mut line_output,
            )
            .unwrap();
        assert_eq!(word.lines()[0].text(), TextRange::new(0, 3));

        let grapheme = shaped
            .break_into(
                text,
                4,
                WrapMode::Grapheme,
                TextSpacing::default(),
                &mut line_output,
            )
            .unwrap();
        assert_eq!(grapheme.lines()[0].text(), TextRange::new(0, 4));

        let no_wrap = shaped
            .break_into(
                text,
                4,
                WrapMode::NoWrap,
                TextSpacing::default(),
                &mut line_output,
            )
            .unwrap();
        assert_eq!(
            no_wrap.lines(),
            &[BrokenLine {
                text: TextRange::new(0, 5),
                advance: 5,
                end: LineEnd::Paragraph,
            }]
        );
    }

    #[test]
    fn spacing_changes_break_width_and_positioned_advances() {
        let text = "ab c";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 4];
        let mut initial_runs = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let spacing = TextSpacing { letter: 1, word: 2 };
        let mut broken_output = [BrokenLine::empty(); 1];
        let broken = shaped
            .break_into(text, 9, WrapMode::NoWrap, spacing, &mut broken_output)
            .unwrap();
        assert_eq!(broken.lines()[0].advance(), 9);

        let mut scratch = [ShapedGlyph::default(); 4];
        let mut glyphs = [PositionedGlyph::default(); 4];
        let mut runs = [VisualRun::empty(); 1];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 5];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10).with_spacing(spacing),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        assert_eq!(layout.lines()[0].advance(), 9);
        assert_eq!(layout.glyphs()[1].origin.x, 2);
        assert_eq!(layout.glyphs()[2].origin.x, 4);
        assert_eq!(layout.glyphs()[3].origin.x, 8);
        assert_eq!(layout.carets().last().unwrap().position.x, 9);
    }

    #[test]
    fn alignment_is_applied_to_glyphs_lines_and_carets() {
        let text = "ab";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 2];
        let mut initial_runs = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let broken_storage = [BrokenLine {
            text: TextRange::new(0, 2),
            advance: 2,
            end: LineEnd::Paragraph,
        }];
        let broken = BrokenLines {
            shaped: &shaped,
            lines: &broken_storage,
        };
        let mut scratch = [ShapedGlyph::default(); 2];
        let mut glyphs = [PositionedGlyph::default(); 2];
        let mut runs = [VisualRun::empty(); 1];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 3];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10)
                    .with_width(10)
                    .with_alignment(Alignment::Center),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        assert_eq!(layout.lines()[0].origin().x, 4);
        assert_eq!(layout.glyphs()[0].origin.x, 4);
        assert_eq!(layout.glyphs()[1].origin.x, 5);
        assert_eq!(layout.carets()[0].position.x, 4);
        assert_eq!(layout.carets().last().unwrap().position.x, 6);

        let mut scratch = [ShapedGlyph::default(); 2];
        let mut glyphs = [PositionedGlyph::default(); 2];
        let mut runs = [VisualRun::empty(); 1];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 3];
        let rtl_start = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10)
                    .with_width(10)
                    .with_alignment(Alignment::Start)
                    .with_direction(Direction::RightToLeft),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();
        assert_eq!(rtl_start.lines()[0].origin().x, 8);
        assert_eq!(rtl_start.glyphs()[0].origin.x, 8);
    }

    #[test]
    fn justification_expands_internal_spaces_on_wrapped_lines() {
        let text = "a b ";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 1] = [&primary];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 4];
        let mut initial_runs = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let broken_storage = [BrokenLine {
            text: TextRange::new(0, 4),
            advance: 4,
            end: LineEnd::Wrap,
        }];
        let broken = BrokenLines {
            shaped: &shaped,
            lines: &broken_storage,
        };
        let mut scratch = [ShapedGlyph::default(); 4];
        let mut glyphs = [PositionedGlyph::default(); 4];
        let mut runs = [VisualRun::empty(); 1];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 5];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10)
                    .with_width(8)
                    .with_alignment(Alignment::Justify),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        assert_eq!(layout.lines()[0].advance(), 8);
        assert_eq!(layout.glyphs()[1].advance.x, 5);
        assert_eq!(layout.glyphs()[2].origin.x, 6);
        assert_eq!(layout.glyphs()[3].origin.x, 7);
        assert_eq!(layout.carets()[2].position.x, 6);
        assert_eq!(layout.carets().last().unwrap().position.x, 8);
    }

    #[test]
    fn lets_unsafe_sequences_exceed_the_width() {
        let text = "ab cd";
        let mut glyphs = [ShapedGlyph::default(); 5];
        for (index, glyph) in glyphs.iter_mut().enumerate() {
            *glyph = ShapedGlyph::new(
                GlyphId::new(index as u16),
                TextRange::new(index as u32, index as u32 + 1),
            );
            glyph.advance.x = 1;
        }
        glyphs[3].set_unsafe_to_break(true);
        let runs = [GlyphRun {
            text: TextRange::new(0, 5),
            glyphs: TextRange::new(0, 5),
            font_id: FontId::new(1),
            bidi_level: 0,
        }];
        let shaped = ShapedText {
            text: TextRange::new(0, 5),
            glyphs: &glyphs,
            runs: &runs,
        };
        let mut line_output = [BrokenLine::empty(); 2];
        let lines = shaped
            .break_into(
                text,
                3,
                WrapMode::Word,
                TextSpacing::default(),
                &mut line_output,
            )
            .unwrap();

        assert_eq!(lines.lines()[0].text(), TextRange::new(0, 4));
        assert_eq!(lines.lines()[0].advance(), 4);
        assert_eq!(lines.lines()[1].text(), TextRange::new(4, 5));
    }

    #[test]
    fn ellipsis_keeps_unsafe_cluster_boundaries_together() {
        let text = "abc";
        let mut glyphs = [ShapedGlyph::default(); 3];
        for (index, glyph) in glyphs.iter_mut().enumerate() {
            *glyph = ShapedGlyph::new(
                GlyphId::new(index as u16),
                TextRange::new(index as u32, index as u32 + 1),
            );
            glyph.advance.x = 1;
        }
        glyphs[2].set_unsafe_to_break(true);
        let runs = [GlyphRun {
            text: TextRange::new(0, 3),
            glyphs: TextRange::new(0, 3),
            font_id: FontId::new(1),
            bidi_level: 0,
        }];
        let shaped = ShapedText {
            text: TextRange::new(0, 3),
            glyphs: &glyphs,
            runs: &runs,
        };

        assert_eq!(
            fit_prefix(
                &shaped,
                text,
                TextRange::new(0, 3),
                2,
                TextSpacing::default(),
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn reshapes_line_edges_into_visual_outputs() {
        let text = "ab cd";
        let typefaces: [&dyn Typeface; 1] = [&EdgeTypeface];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 5];
        let mut initial_runs = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let mut broken_output = [BrokenLine::empty(); 2];
        let broken = shaped
            .break_into(
                text,
                3,
                WrapMode::Word,
                TextSpacing::default(),
                &mut broken_output,
            )
            .unwrap();
        let mut scratch = [ShapedGlyph::default(); 3];
        let mut glyphs = [PositionedGlyph::default(); 5];
        let mut runs = [VisualRun::empty(); 2];
        let mut lines = [LayoutLine::empty(); 2];
        let mut carets = [CaretStop::default(); 7];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10).with_origin(FlowPoint { x: 5, y: 7 }),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        assert_eq!(layout.lines().len(), 2);
        assert_eq!(layout.lines()[0].advance(), 6);
        assert_eq!(layout.lines()[1].advance(), 4);
        assert_eq!(layout.lines()[0].origin(), FlowPoint { x: 5, y: 7 });
        assert_eq!(layout.lines()[1].origin(), FlowPoint { x: 5, y: 17 });
        assert_eq!(layout.runs().len(), 2);
        assert_eq!(layout.runs()[0].font_id(), FontId::new(9));
        assert_eq!(layout.glyphs().len(), 5);
        assert_eq!(layout.carets().len(), 7);
    }

    #[test]
    fn ellipsis_rechecks_the_reshaped_line_width() {
        let text = "ab cd";
        let typefaces: [&dyn Typeface; 1] = [&EdgeTypeface];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 5];
        let mut initial_runs = [GlyphRun::empty(); 1];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let mut broken_output = [BrokenLine::empty(); 2];
        let broken = shaped
            .break_into(
                text,
                4,
                WrapMode::Word,
                TextSpacing::default(),
                &mut broken_output,
            )
            .unwrap();
        let mut scratch = [ShapedGlyph::default(); 5];
        let mut glyphs = [PositionedGlyph::default(); 5];
        let mut runs = [VisualRun::empty(); 3];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 6];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10)
                    .with_width(4)
                    .with_max_lines(1)
                    .with_overflow(Overflow::Ellipsis),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        assert_eq!(layout.lines()[0].text(), TextRange::new(0, 1));
        assert_eq!(layout.lines()[0].advance(), 4);
        assert_eq!(layout.glyphs()[0].glyph_id(), GlyphId::new('a' as u16));
        assert_eq!(
            layout.glyphs()[1].glyph_id(),
            GlyphId::new('\u{2026}' as u16)
        );
    }

    #[test]
    fn rejects_invalid_typeface_output_counts() {
        let text = "a";
        let typefaces: [&dyn Typeface; 1] = [&InvalidTypeface];
        let mut bidi_output = bidi_slots::<1>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut bidi_output).unwrap();
        let mut logical_output = [LogicalRun::empty(); 1];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut glyphs = [ShapedGlyph::default(); 1];
        let mut runs = [GlyphRun::empty(); 1];

        assert_eq!(
            logical
                .shape_into(text, &typefaces, &[], &mut glyphs, &mut runs)
                .err()
                .unwrap(),
            LayoutError::InvalidTypefaceOutput { run: 0 }
        );
    }

    #[test]
    fn reorders_mixed_runs_for_rtl_lines() {
        let text = "אבג abc";
        let primary = SimpleTypeface::new(&Source {
            key: 1,
            ascii: false,
        });
        let fallback = SimpleTypeface::new(&Source {
            key: 2,
            ascii: true,
        });
        let typefaces: [&dyn Typeface; 2] = [&primary, &fallback];
        let mut bidi_output = bidi_slots::<4>();
        let bidi = BidiText::resolve(
            text,
            0..text.len(),
            BaseDirection::RightToLeft,
            &mut bidi_output,
        )
        .unwrap();
        let mut logical_output = [LogicalRun::empty(); 4];
        let logical = LogicalRuns::resolve(text, &bidi, &typefaces, &mut logical_output).unwrap();
        let mut initial_glyphs = [ShapedGlyph::default(); 7];
        let mut initial_runs = [GlyphRun::empty(); 4];
        let shaped = logical
            .shape_into(
                text,
                &typefaces,
                &[],
                &mut initial_glyphs,
                &mut initial_runs,
            )
            .unwrap();
        let mut broken_output = [BrokenLine::empty(); 1];
        let broken = shaped
            .break_into(
                text,
                100,
                WrapMode::Word,
                TextSpacing::default(),
                &mut broken_output,
            )
            .unwrap();
        let mut scratch = [ShapedGlyph::default(); 3];
        let mut glyphs = [PositionedGlyph::default(); 7];
        let mut runs = [VisualRun::empty(); 4];
        let mut lines = [LayoutLine::empty(); 1];
        let mut carets = [CaretStop::default(); 12];
        let layout = logical
            .layout_into(
                text,
                &typefaces,
                &[],
                &broken,
                LayoutOptions::new(10),
                LayoutBuffers::new(
                    &mut scratch,
                    &mut glyphs,
                    &mut runs,
                    &mut lines,
                    &mut carets,
                ),
            )
            .unwrap();

        let first = layout.runs()[0].text();
        assert_eq!(&text[first.start as usize..first.end as usize], "abc");
        assert_eq!(layout.runs()[0].bidi_level(), 2);
    }
}
