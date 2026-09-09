use crate::bidi::BidiText;
use crate::shaping::{
    FaceKey, FontFeature, ShapeError, ShapeRequest, ShapedGlyph, ShapedRun, TextRange, Typeface,
    TypefaceError,
};
use crate::unicode::{graphemes, line_breaks, script_runs, LineBreakKind, LineBreaks, Script};
use core::iter::Peekable;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogicalRun {
    text: TextRange,
    face: u16,
    script: Script,
    bidi_level: u8,
}

impl LogicalRun {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            face: 0,
            script: Script::Common,
            bidi_level: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn face_index(self) -> usize {
        self.face as usize
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
    TooManyFaces,
    InsufficientRunCapacity { required: usize },
    InsufficientGlyphCapacity { minimum: usize },
    InsufficientLineCapacity { required: usize },
    InvalidRun,
    InvalidWidth,
    CoordinateOverflow,
    Typeface(TypefaceError),
    Shape { run: usize, error: ShapeError },
}

impl From<TypefaceError> for LayoutError {
    fn from(error: TypefaceError) -> Self {
        Self::Typeface(error)
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
            return Err(LayoutError::TooManyFaces);
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
                    let face = select_typeface(cluster.text, typefaces)?;
                    writer.push(LogicalRun {
                        text: TextRange::new(start as u32, end as u32),
                        face,
                        script: script_run.script,
                        bidi_level: bidi_run.level,
                    });
                }
            }
        }
        let count = writer.finish()?;
        Ok(Self {
            text: TextRange::new(paragraph.start as u32, paragraph.end as u32),
            runs: &output[..count],
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
            let Some(typeface) = typefaces.get(logical.face_index()) else {
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
            if end > u32::MAX as usize {
                return Err(LayoutError::InvalidRun);
            }
            runs[run_index] = GlyphRun {
                text: logical.text,
                glyphs: TextRange::new(glyph_count as u32, end as u32),
                face: typeface.key(),
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
}

struct LogicalRunWriter<'a> {
    output: &'a mut [LogicalRun],
    count: usize,
    last: Option<LogicalRun>,
}

impl<'a> LogicalRunWriter<'a> {
    fn new(output: &'a mut [LogicalRun]) -> Self {
        Self {
            output,
            count: 0,
            last: None,
        }
    }

    fn push(&mut self, run: LogicalRun) {
        if let Some(previous) = self.last.as_mut() {
            if previous.text.end == run.text.start
                && previous.face == run.face
                && previous.script == run.script
                && previous.bidi_level == run.bidi_level
            {
                previous.text.end = run.text.end;
                if let Some(stored) = self.output.get_mut(self.count - 1) {
                    stored.text.end = run.text.end;
                }
                return;
            }
        }
        if let Some(slot) = self.output.get_mut(self.count) {
            *slot = run;
        }
        self.count += 1;
        self.last = Some(run);
    }

    fn finish(self) -> Result<usize, LayoutError> {
        if self.count > self.output.len() {
            Err(LayoutError::InsufficientRunCapacity {
                required: self.count,
            })
        } else {
            Ok(self.count)
        }
    }
}

fn select_typeface(grapheme: &str, typefaces: &[&dyn Typeface]) -> Result<u16, TypefaceError> {
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
    face: FaceKey,
    bidi_level: u8,
}

impl GlyphRun {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            glyphs: TextRange::new(0, 0),
            face: FaceKey::new(0),
            bidi_level: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn glyphs(self) -> TextRange {
        self.glyphs
    }

    pub const fn face(self) -> FaceKey {
        self.face
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
        Some(ShapedRun::new(run.face, run.text, run.bidi_level, glyphs))
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

    pub fn break_into<'output>(
        &self,
        text: &str,
        max_width: i32,
        output: &'output mut [BrokenLine],
    ) -> Result<BrokenLines<'output>, LayoutError> {
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
        let mut breaker = LineBreaker::new(self, self.text, max_width, breaks, output);
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
}

impl BrokenLine {
    pub const fn empty() -> Self {
        Self {
            text: TextRange::new(0, 0),
            advance: 0,
        }
    }

    pub const fn text(self) -> TextRange {
        self.text
    }

    pub const fn advance(self) -> i32 {
        self.advance
    }
}

pub struct BrokenLines<'a> {
    lines: &'a [BrokenLine],
}

impl BrokenLines<'_> {
    pub const fn lines(&self) -> &[BrokenLine] {
        self.lines
    }
}

struct LineBreaker<'shaped, 'glyphs, 'text, 'output> {
    shaped: &'shaped ShapedText<'glyphs>,
    paragraph: TextRange,
    max_width: i32,
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
        breaks: Peekable<LineBreaks<'text>>,
        output: &'output mut [BrokenLine],
    ) -> Self {
        Self {
            shaped,
            paragraph,
            max_width,
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
            if let Some((offset, width)) = self.last_allowed.take() {
                self.emit(offset, width)?;
            } else if width_before > 0
                && cluster.start > self.line_start
                && self.shaped.is_safe_break(cluster.start)
            {
                self.emit(cluster.start, width_before)?;
            } else if self.line_start == cluster.start && self.shaped.is_safe_break(cluster.end) {
                self.emit(cluster.end, self.width)?;
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
                LineBreakKind::Mandatory => self.emit(global, self.width)?,
                LineBreakKind::Allowed if self.width > self.max_width => {
                    self.emit(global, self.width)?;
                }
                LineBreakKind::Allowed => self.last_allowed = Some((global, self.width)),
            }
        }
        Ok(())
    }

    fn emit(&mut self, end: u32, advance: i32) -> Result<(), LayoutError> {
        if end <= self.line_start {
            return Ok(());
        }
        self.writer.push(BrokenLine {
            text: TextRange::new(self.line_start, end),
            advance,
        });
        self.line_start = end;
        self.width = self
            .width
            .checked_sub(advance)
            .ok_or(LayoutError::CoordinateOverflow)?;
        self.last_allowed = None;
        Ok(())
    }

    fn finish(mut self) -> Result<BrokenLines<'output>, LayoutError> {
        self.take_breaks_through(self.paragraph.end)?;
        if self.line_start < self.paragraph.end {
            self.emit(self.paragraph.end, self.width)?;
        }
        let count = self.writer.finish()?;
        Ok(BrokenLines {
            lines: &self.writer.output[..count],
        })
    }
}

struct LineWriter<'a> {
    output: &'a mut [BrokenLine],
    count: usize,
}

impl<'a> LineWriter<'a> {
    fn new(output: &'a mut [BrokenLine]) -> Self {
        Self { output, count: 0 }
    }

    fn push(&mut self, line: BrokenLine) {
        if let Some(slot) = self.output.get_mut(self.count) {
            *slot = line;
        }
        self.count += 1;
    }

    fn finish(&self) -> Result<usize, LayoutError> {
        if self.count > self.output.len() {
            Err(LayoutError::InsufficientLineCapacity {
                required: self.count,
            })
        } else {
            Ok(self.count)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidi::{BaseDirection, BidiRun};
    use crate::shaping::{FaceKey, FlowPoint, FontMetrics, GlyphId, GlyphSource, SimpleTypeface};
    use std::prelude::v1::*;

    struct Source {
        key: u64,
        ascii: bool,
    }

    impl GlyphSource for Source {
        fn key(&self) -> FaceKey {
            FaceKey::new(self.key)
        }

        fn metrics(&self) -> Result<FontMetrics, TypefaceError> {
            Ok(FontMetrics::default())
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, TypefaceError> {
            let covered = character.is_ascii() == self.ascii;
            Ok(covered.then_some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, TypefaceError> {
            Ok(FlowPoint { x: 1, y: 0 })
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
        assert_eq!(runs.runs()[0].face_index(), 0);
        assert_eq!(runs.runs()[1].face_index(), 1);
        assert_eq!(runs.runs()[1].script(), Script::Han);
        assert_eq!(runs.runs()[2].face_index(), 0);
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
        assert_eq!(shaped.run(0).unwrap().face(), FaceKey::new(1));
        assert_eq!(shaped.run(1).unwrap().face(), FaceKey::new(2));
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
        let lines = shaped.break_into(text, 3, &mut line_output).unwrap();

        assert_eq!(lines.lines().len(), 2);
        assert_eq!(lines.lines()[0].text(), TextRange::new(0, 3));
        assert_eq!(lines.lines()[0].advance(), 3);
        assert_eq!(lines.lines()[1].text(), TextRange::new(3, 5));
        assert_eq!(lines.lines()[1].advance(), 2);

        let mut no_lines = [];
        assert_eq!(
            shaped.break_into(text, 3, &mut no_lines).err().unwrap(),
            LayoutError::InsufficientLineCapacity { required: 2 }
        );
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
            face: FaceKey::new(1),
            bidi_level: 0,
        }];
        let shaped = ShapedText {
            text: TextRange::new(0, 5),
            glyphs: &glyphs,
            runs: &runs,
        };
        let mut line_output = [BrokenLine::empty(); 2];
        let lines = shaped.break_into(text, 3, &mut line_output).unwrap();

        assert_eq!(lines.lines()[0].text(), TextRange::new(0, 4));
        assert_eq!(lines.lines()[0].advance(), 4);
        assert_eq!(lines.lines()[1].text(), TextRange::new(4, 5));
    }
}
