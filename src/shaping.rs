use crate::bidi::Direction;
use crate::unicode::{graphemes, Script};
use core::ops::Range;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FaceKey(u64);

impl FaceKey {
    pub const fn new(value: u64) -> Self {
        Self(value)
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
    Source(TypefaceError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypefaceError {
    Unavailable,
    Malformed,
    Unsupported,
}

impl From<TypefaceError> for ShapeError {
    fn from(error: TypefaceError) -> Self {
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

pub trait Typeface {
    fn key(&self) -> FaceKey;
    fn metrics(&self) -> Result<FontMetrics, TypefaceError>;
    fn covers(&self, grapheme: &str) -> Result<bool, TypefaceError>;
    fn shape_into(
        &self,
        request: &ShapeRequest<'_>,
        output: &mut [ShapedGlyph],
    ) -> Result<usize, ShapeError>;
}

pub trait GlyphSource {
    fn key(&self) -> FaceKey;
    fn metrics(&self) -> Result<FontMetrics, TypefaceError>;
    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, TypefaceError>;
    fn glyph_advance(&self, glyph: GlyphId) -> Result<FlowPoint, TypefaceError>;

    fn kerning(&self, _left: GlyphId, _right: GlyphId) -> Result<i32, TypefaceError> {
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
    fn key(&self) -> FaceKey {
        self.source.key()
    }

    fn metrics(&self) -> Result<FontMetrics, TypefaceError> {
        self.source.metrics()
    }

    fn covers(&self, grapheme: &str) -> Result<bool, TypefaceError> {
        let mut characters = grapheme.chars();
        let Some(character) = characters.next() else {
            return Ok(false);
        };
        if characters.next().is_some() {
            return Ok(false);
        }
        Ok(self.source.glyph_for(character)?.is_some())
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
            let glyph_id = self
                .source
                .glyph_for(character)?
                .ok_or(ShapeError::MissingGlyph {
                    offset: request.range.start + cluster.range.start,
                })?;
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
        fn key(&self) -> FaceKey {
            FaceKey::new(4)
        }

        fn metrics(&self) -> Result<FontMetrics, TypefaceError> {
            Ok(FontMetrics {
                units_per_em: 1000,
                ascender: 800,
                descender: -200,
                line_gap: 0,
            })
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, TypefaceError> {
            Ok(character
                .is_ascii()
                .then_some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, TypefaceError> {
            Ok(FlowPoint { x: 600, y: 0 })
        }

        fn kerning(&self, left: GlyphId, right: GlyphId) -> Result<i32, TypefaceError> {
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
}
