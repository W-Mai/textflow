use crate::bidi::Direction;
use crate::unicode::{graphemes, line_breaks, LineBreak, LineBreaks, Script};
use core::ops::Range;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontId(u64);

impl FontId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct GlyphId(u16);

impl GlyphId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlowPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FontMetrics {
    pub units_per_em: u16,
    pub ascender: i32,
    pub descender: i32,
    pub line_gap: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl TextRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: u32,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineEdges(u8);

impl LineEdges {
    pub const NONE: Self = Self(0);
    pub const START: Self = Self(1);
    pub const END: Self = Self(2);
    pub const BOTH: Self = Self(3);

    pub const fn has_start(self) -> bool {
        self.0 & Self::START.0 != 0
    }

    pub const fn has_end(self) -> bool {
        self.0 & Self::END.0 != 0
    }
}

impl FontFeature {
    pub const fn new(tag: [u8; 4], value: u32) -> Self {
        Self {
            tag,
            value,
            range: TextRange::new(0, u32::MAX),
        }
    }

    pub const fn with_range(mut self, range: TextRange) -> Self {
        self.range = range;
        self
    }
}

pub struct ShapeRequest<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub direction: Direction,
    pub script: Script,
    pub language: Option<&'a str>,
    pub features: &'a [FontFeature],
    pub line_edges: LineEdges,
}

impl<'a> ShapeRequest<'a> {
    pub fn new(text: &'a str, range: Range<usize>, direction: Direction, script: Script) -> Self {
        Self {
            text,
            range,
            direction,
            script,
            language: None,
            features: &[],
            line_edges: LineEdges::NONE,
        }
    }

    pub fn with_language(mut self, language: &'a str) -> Self {
        self.language = Some(language);
        self
    }

    pub fn with_features(mut self, features: &'a [FontFeature]) -> Self {
        self.features = features;
        self
    }

    pub fn with_line_edges(mut self, edges: LineEdges) -> Self {
        self.line_edges = edges;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeError {
    InvalidTextRange,
    TextTooLong,
    InsufficientCapacity { required: usize },
    UnsupportedScript { script: Script },
    UnsupportedFeature { tag: [u8; 4] },
    UnsupportedCluster { offset: usize },
    MissingGlyph { offset: usize },
    ShapingUnavailable,
    Source(FontAccessError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontAccessError {
    Unavailable,
    Malformed,
    Unsupported,
}

impl From<FontAccessError> for ShapeError {
    fn from(error: FontAccessError) -> Self {
        Self::Source(error)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShapedGlyph {
    glyph_id: GlyphId,
    flags: u16,
    pub cluster: TextRange,
    pub advance: FlowPoint,
    pub offset: FlowPoint,
}

impl ShapedGlyph {
    const UNSAFE_TO_BREAK: u16 = 1;

    pub const fn new(glyph_id: GlyphId, cluster: TextRange) -> Self {
        Self {
            glyph_id,
            flags: 0,
            cluster,
            advance: FlowPoint { x: 0, y: 0 },
            offset: FlowPoint { x: 0, y: 0 },
        }
    }

    pub const fn glyph_id(self) -> GlyphId {
        self.glyph_id
    }

    pub const fn unsafe_to_break(self) -> bool {
        self.flags & Self::UNSAFE_TO_BREAK != 0
    }

    pub fn set_unsafe_to_break(&mut self, unsafe_to_break: bool) {
        if unsafe_to_break {
            self.flags |= Self::UNSAFE_TO_BREAK;
        } else {
            self.flags &= !Self::UNSAFE_TO_BREAK;
        }
    }
}

pub struct ShapedRun<'a> {
    font_id: FontId,
    text: TextRange,
    bidi_level: u8,
    glyphs: &'a [ShapedGlyph],
}

impl<'a> ShapedRun<'a> {
    pub const fn new(
        font_id: FontId,
        text: TextRange,
        bidi_level: u8,
        glyphs: &'a [ShapedGlyph],
    ) -> Self {
        Self {
            font_id,
            text,
            bidi_level,
            glyphs,
        }
    }

    pub const fn font_id(&self) -> FontId {
        self.font_id
    }

    pub const fn text(&self) -> TextRange {
        self.text
    }

    pub const fn direction(&self) -> Direction {
        Direction::from_level(self.bidi_level)
    }

    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    pub const fn glyphs(&self) -> &[ShapedGlyph] {
        self.glyphs
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

    pub fn line_breaks<'text, 'run>(
        &'run self,
        text: &'text str,
    ) -> Result<SafeLineBreaks<'text, 'run, 'a>, ShapeError> {
        let start = self.text.start as usize;
        let end = self.text.end as usize;
        if start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(ShapeError::InvalidTextRange);
        }
        Ok(SafeLineBreaks {
            inner: line_breaks(&text[start..end]),
            run: self,
            base: self.text.start,
        })
    }

    pub fn position_into<'output>(
        &self,
        origin: FlowPoint,
        output: &'output mut [PositionedGlyph],
    ) -> Result<PositionedRun<'output>, PositionError> {
        if output.len() < self.glyphs.len() {
            return Err(PositionError::InsufficientCapacity {
                required: self.glyphs.len(),
            });
        }
        let mut pen = origin;
        for (slot, glyph) in output.iter_mut().zip(self.glyphs) {
            *slot = PositionedGlyph {
                glyph_id: glyph.glyph_id(),
                cluster: glyph.cluster,
                origin: pen,
                advance: glyph.advance,
                offset: glyph.offset,
                bidi_level: self.bidi_level,
            };
            pen.x = pen
                .x
                .checked_add(glyph.advance.x)
                .ok_or(PositionError::CoordinateOverflow)?;
            pen.y = pen
                .y
                .checked_add(glyph.advance.y)
                .ok_or(PositionError::CoordinateOverflow)?;
        }
        Ok(PositionedRun {
            font_id: self.font_id,
            text: self.text,
            direction: self.direction(),
            bidi_level: self.bidi_level,
            end: pen,
            glyphs: &output[..self.glyphs.len()],
        })
    }
}

pub struct SafeLineBreaks<'text, 'run, 'glyph> {
    inner: LineBreaks<'text>,
    run: &'run ShapedRun<'glyph>,
    base: u32,
}

impl Iterator for SafeLineBreaks<'_, '_, '_> {
    type Item = LineBreak;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut next = self.inner.next()?;
            next.offset += self.base as usize;
            if self.run.is_safe_break(next.offset as u32) {
                return Some(next);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionError {
    InsufficientCapacity { required: usize },
    CoordinateOverflow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PositionedGlyph {
    glyph_id: GlyphId,
    pub cluster: TextRange,
    pub origin: FlowPoint,
    pub advance: FlowPoint,
    pub offset: FlowPoint,
    pub bidi_level: u8,
}

impl PositionedGlyph {
    pub const fn glyph_id(self) -> GlyphId {
        self.glyph_id
    }
}

pub struct PositionedRun<'a> {
    font_id: FontId,
    text: TextRange,
    direction: Direction,
    bidi_level: u8,
    end: FlowPoint,
    glyphs: &'a [PositionedGlyph],
}

impl PositionedRun<'_> {
    pub const fn font_id(&self) -> FontId {
        self.font_id
    }

    pub const fn text(&self) -> TextRange {
        self.text
    }

    pub const fn direction(&self) -> Direction {
        self.direction
    }

    pub const fn end(&self) -> FlowPoint {
        self.end
    }

    pub const fn glyphs(&self) -> &[PositionedGlyph] {
        self.glyphs
    }

    pub fn carets_into<'output>(
        &self,
        output: &'output mut [CaretStop],
    ) -> Result<&'output [CaretStop], PositionError> {
        let mut required = 0;
        let mut previous = None;
        for glyph in self.glyphs {
            let text_offset = match self.direction {
                Direction::LeftToRight => glyph.cluster.start,
                Direction::RightToLeft => glyph.cluster.end,
            };
            if previous != Some(text_offset) {
                required += 1;
                previous = Some(text_offset);
            }
        }
        let final_offset = match self.direction {
            Direction::LeftToRight => self.text.end,
            Direction::RightToLeft => self.text.start,
        };
        if previous != Some(final_offset) {
            required += 1;
        }
        if output.len() < required {
            return Err(PositionError::InsufficientCapacity { required });
        }
        let mut count = 0;
        for glyph in self.glyphs {
            let text_offset = match self.direction {
                Direction::LeftToRight => glyph.cluster.start,
                Direction::RightToLeft => glyph.cluster.end,
            };
            if count == 0 || output[count - 1].text_offset != text_offset {
                output[count] = CaretStop {
                    text_offset,
                    position: glyph.origin,
                    bidi_level: self.bidi_level,
                };
                count += 1;
            }
        }
        let text_offset = match self.direction {
            Direction::LeftToRight => self.text.end,
            Direction::RightToLeft => self.text.start,
        };
        if count == 0 || output[count - 1].text_offset != text_offset {
            output[count] = CaretStop {
                text_offset,
                position: self.end,
                bidi_level: self.bidi_level,
            };
            count += 1;
        }
        Ok(&output[..count])
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaretStop {
    pub text_offset: u32,
    pub position: FlowPoint,
    pub bidi_level: u8,
}

pub trait Typeface {
    fn id(&self) -> FontId;
    fn metrics(&self) -> Result<FontMetrics, FontAccessError>;
    fn covers(&self, grapheme: &str) -> Result<bool, FontAccessError>;
    fn supports_complex_shaping(&self) -> bool {
        true
    }
    fn shape_into(
        &self,
        request: &ShapeRequest<'_>,
        output: &mut [ShapedGlyph],
    ) -> Result<usize, ShapeError>;
}

pub trait GlyphSource {
    fn id(&self) -> FontId;
    fn metrics(&self) -> Result<FontMetrics, FontAccessError>;
    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError>;
    fn glyph_advance(&self, glyph: GlyphId) -> Result<FlowPoint, FontAccessError>;

    fn notdef_glyph(&self) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(Some(GlyphId::new(0)))
    }

    fn kerning(&self, _left: GlyphId, _right: GlyphId) -> Result<i32, FontAccessError> {
        Ok(0)
    }
}

pub struct SimpleTypeface<'a, T> {
    source: &'a T,
}

impl<'a, T> SimpleTypeface<'a, T> {
    pub const fn new(source: &'a T) -> Self {
        Self { source }
    }
}

impl<T> Typeface for SimpleTypeface<'_, T>
where
    T: GlyphSource,
{
    fn id(&self) -> FontId {
        self.source.id()
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        self.source.metrics()
    }

    fn covers(&self, grapheme: &str) -> Result<bool, FontAccessError> {
        let mut characters = grapheme.chars();
        let Some(character) = characters.next() else {
            return Ok(false);
        };
        if characters.next().is_some() {
            return Ok(false);
        }
        Ok(self.source.glyph_for(character)?.is_some())
    }

    fn supports_complex_shaping(&self) -> bool {
        false
    }

    fn shape_into(
        &self,
        request: &ShapeRequest<'_>,
        output: &mut [ShapedGlyph],
    ) -> Result<usize, ShapeError> {
        validate_request(request)?;
        if matches!(request.script, Script::Arabic | Script::Devanagari) {
            return Err(ShapeError::UnsupportedScript {
                script: request.script,
            });
        }
        let kern = kerning_enabled(request.features)?;
        let text = &request.text[request.range.clone()];
        let required = graphemes(text).count();
        if output.len() < required {
            return Err(ShapeError::InsufficientCapacity { required });
        }

        let mut previous: Option<(usize, GlyphId)> = None;
        for (logical_index, cluster) in graphemes(text).enumerate() {
            let mut characters = cluster.text.chars();
            let character = characters.next().unwrap();
            if characters.next().is_some() {
                return Err(ShapeError::UnsupportedCluster {
                    offset: request.range.start + cluster.range.start,
                });
            }
            let glyph_id = match self.source.glyph_for(character)? {
                Some(glyph) => glyph,
                None => self
                    .source
                    .notdef_glyph()?
                    .ok_or(ShapeError::MissingGlyph {
                        offset: request.range.start + cluster.range.start,
                    })?,
            };
            let output_index = match request.direction {
                Direction::LeftToRight => logical_index,
                Direction::RightToLeft => required - logical_index - 1,
            };
            let start = request.range.start + cluster.range.start;
            let end = request.range.start + cluster.range.end;
            output[output_index] = ShapedGlyph {
                glyph_id,
                flags: 0,
                cluster: TextRange::new(start as u32, end as u32),
                advance: self.source.glyph_advance(glyph_id)?,
                offset: FlowPoint::default(),
            };
            if kern {
                if let Some((previous_index, previous_glyph)) = previous {
                    output[previous_index].advance.x +=
                        self.source.kerning(previous_glyph, glyph_id)?;
                }
                previous = Some((output_index, glyph_id));
            }
        }
        Ok(required)
    }
}

fn validate_request(request: &ShapeRequest<'_>) -> Result<(), ShapeError> {
    if request.range.start > request.range.end
        || request.range.end > request.text.len()
        || !request.text.is_char_boundary(request.range.start)
        || !request.text.is_char_boundary(request.range.end)
    {
        return Err(ShapeError::InvalidTextRange);
    }
    if request.text.len() > u32::MAX as usize {
        return Err(ShapeError::TextTooLong);
    }
    Ok(())
}

fn kerning_enabled(features: &[FontFeature]) -> Result<bool, ShapeError> {
    let mut enabled = true;
    for feature in features {
        if feature.tag == *b"kern" {
            enabled = feature.value != 0;
        } else {
            return Err(ShapeError::UnsupportedFeature { tag: feature.tag });
        }
    }
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    struct MockFont;

    impl GlyphSource for MockFont {
        fn id(&self) -> FontId {
            FontId::new(4)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics {
                units_per_em: 1000,
                ascender: 800,
                descender: -200,
                line_gap: 0,
            })
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
            Ok(character
                .is_ascii()
                .then_some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint { x: 600, y: 0 })
        }

        fn kerning(&self, left: GlyphId, right: GlyphId) -> Result<i32, FontAccessError> {
            Ok(
                if left.value() == b'A' as u16 && right.value() == b'V' as u16 {
                    -80
                } else {
                    0
                },
            )
        }
    }

    #[test]
    fn shapes_into_caller_storage_with_kerning() {
        let face = SimpleTypeface::new(&MockFont);
        assert!(!face.supports_complex_shaping());
        let text = "AV";
        let mut output = [ShapedGlyph::default(); 2];
        let count = face
            .shape_into(
                &ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin),
                &mut output,
            )
            .unwrap();

        assert_eq!(count, 2);
        assert_eq!(output[0].advance.x, 520);
        assert_eq!(output[1].advance.x, 600);
    }

    #[test]
    fn reverses_rtl_output_without_heap_storage() {
        let face = SimpleTypeface::new(&MockFont);
        let text = "abc";
        let mut output = [ShapedGlyph::default(); 3];
        face.shape_into(
            &ShapeRequest::new(text, 0..text.len(), Direction::RightToLeft, Script::Hebrew),
            &mut output,
        )
        .unwrap();

        assert_eq!(output[0].glyph_id(), GlyphId::new(b'c' as u16));
        assert_eq!(output[2].glyph_id(), GlyphId::new(b'a' as u16));
    }

    #[test]
    fn reports_capacity_and_unsupported_clusters() {
        let face = SimpleTypeface::new(&MockFont);
        let mut output = [ShapedGlyph::default(); 1];
        let text = "ab";
        assert_eq!(
            face.shape_into(
                &ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin,),
                &mut output,
            ),
            Err(ShapeError::InsufficientCapacity { required: 2 })
        );
        let combined = "a\u{301}";
        assert_eq!(
            face.shape_into(
                &ShapeRequest::new(
                    combined,
                    0..combined.len(),
                    Direction::LeftToRight,
                    Script::Latin,
                ),
                &mut output,
            ),
            Err(ShapeError::UnsupportedCluster { offset: 0 })
        );
    }

    #[test]
    fn run_rejects_breaks_inside_or_before_unsafe_clusters() {
        let mut glyphs = [
            ShapedGlyph::new(GlyphId::new(1), TextRange::new(0, 3)),
            ShapedGlyph::new(GlyphId::new(2), TextRange::new(3, 4)),
        ];
        glyphs[1].set_unsafe_to_break(true);
        let run = ShapedRun::new(FontId::new(4), TextRange::new(0, 4), 0, &glyphs);

        assert!(run.is_safe_break(0));
        assert!(!run.is_safe_break(1));
        assert!(!run.is_safe_break(3));
        assert!(run.is_safe_break(4));
        assert!(!run.is_safe_break(5));
    }

    #[test]
    fn run_filters_unsafe_unicode_breaks() {
        let text = "hello world";
        let mut glyphs = [ShapedGlyph::default(); 11];
        for (index, glyph) in glyphs.iter_mut().enumerate() {
            *glyph = ShapedGlyph::new(
                GlyphId::new(index as u16),
                TextRange::new(index as u32, index as u32 + 1),
            );
        }
        glyphs[6].set_unsafe_to_break(true);
        let run = ShapedRun::new(
            FontId::new(4),
            TextRange::new(0, text.len() as u32),
            0,
            &glyphs,
        );
        let breaks = run.line_breaks(text).unwrap().collect::<Vec<_>>();

        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].offset, text.len());
    }

    #[test]
    fn positions_glyphs_and_emits_visual_carets() {
        let mut glyphs = [
            ShapedGlyph::new(GlyphId::new(1), TextRange::new(0, 1)),
            ShapedGlyph::new(GlyphId::new(2), TextRange::new(1, 2)),
        ];
        glyphs[0].advance.x = 3;
        glyphs[1].advance.x = 4;
        let run = ShapedRun::new(FontId::new(4), TextRange::new(0, 2), 0, &glyphs);
        let mut positioned = [PositionedGlyph::default(); 2];
        let positioned = run
            .position_into(FlowPoint { x: 10, y: 20 }, &mut positioned)
            .unwrap();

        assert_eq!(positioned.glyphs()[0].origin, FlowPoint { x: 10, y: 20 });
        assert_eq!(positioned.glyphs()[1].origin, FlowPoint { x: 13, y: 20 });
        assert_eq!(positioned.end(), FlowPoint { x: 17, y: 20 });

        let mut carets = [CaretStop::default(); 3];
        let carets = positioned.carets_into(&mut carets).unwrap();
        assert_eq!(carets[0].text_offset, 0);
        assert_eq!(carets[1].text_offset, 1);
        assert_eq!(carets[2].text_offset, 2);
        assert_eq!(carets[2].position.x, 17);
    }

    #[test]
    fn request_carries_explicit_line_edges() {
        let request = ShapeRequest::new("a", 0..1, Direction::LeftToRight, Script::Latin)
            .with_line_edges(LineEdges::BOTH);

        assert!(request.line_edges.has_start());
        assert!(request.line_edges.has_end());
    }

    #[test]
    fn uses_notdef_for_an_uncovered_scalar() {
        let face = SimpleTypeface::new(&MockFont);
        let text = "世";
        let mut output = [ShapedGlyph::default(); 1];
        let count = face
            .shape_into(
                &ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Han),
                &mut output,
            )
            .unwrap();

        assert_eq!(count, 1);
        assert_eq!(output[0].glyph_id(), GlyphId::new(0));
    }
}
