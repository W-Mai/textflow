use super::{
    validate_request, FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource, ShapeError,
    ShapeRequest, ShapedGlyph, SimpleTypeface, Typeface,
};
use crate::bidi::Direction;
use crate::unicode::Script;
use core::ops::Range;

const OUTPUT_FLAGS: u16 = ShapedGlyph::UNSAFE_TO_BREAK;
const MASK_SHIFT: u32 = 1;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlyphMask(u16);

impl GlyphMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u16::MAX >> MASK_SHIFT);

    pub const fn new(bits: u16) -> Self {
        Self(bits & Self::ALL.0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

pub struct GlyphBuffer<'a> {
    storage: &'a mut [ShapedGlyph],
    len: usize,
}

impl<'a> GlyphBuffer<'a> {
    pub fn new(storage: &'a mut [ShapedGlyph], len: usize) -> Result<Self, ShapeError> {
        if len > storage.len() {
            return Err(ShapeError::InsufficientCapacity { required: len });
        }
        Ok(Self { storage, len })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn capacity(&self) -> usize {
        self.storage.len()
    }

    pub fn get(&self, index: usize) -> Option<&ShapedGlyph> {
        self.storage.get(index).filter(|_| index < self.len)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut ShapedGlyph> {
        if index >= self.len {
            return None;
        }
        self.storage.get_mut(index)
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.storage[..self.len]
    }

    pub fn glyphs_mut(&mut self) -> &mut [ShapedGlyph] {
        &mut self.storage[..self.len]
    }

    pub fn mask(&self, index: usize) -> Option<GlyphMask> {
        self.get(index)
            .map(|glyph| GlyphMask::new(glyph.flags >> MASK_SHIFT))
    }

    pub fn set_mask(&mut self, index: usize, mask: GlyphMask) -> Result<(), ShapeError> {
        let glyph = self.get_mut(index).ok_or(ShapeError::InvalidGlyphRange)?;
        glyph.flags = (glyph.flags & OUTPUT_FLAGS) | (mask.bits() << MASK_SHIFT);
        Ok(())
    }

    pub fn set_glyph(&mut self, index: usize, glyph_id: GlyphId) -> Result<(), ShapeError> {
        let glyph = self.get_mut(index).ok_or(ShapeError::InvalidGlyphRange)?;
        glyph.glyph_id = glyph_id;
        Ok(())
    }

    pub fn replace(
        &mut self,
        range: Range<usize>,
        replacements: &[ShapedGlyph],
    ) -> Result<(), ShapeError> {
        if range.start > range.end || range.end > self.len {
            return Err(ShapeError::InvalidGlyphRange);
        }
        let removed = range.end - range.start;
        let required = self
            .len
            .checked_sub(removed)
            .and_then(|len| len.checked_add(replacements.len()))
            .ok_or(ShapeError::InsufficientCapacity {
                required: usize::MAX,
            })?;
        if required > self.storage.len() {
            return Err(ShapeError::InsufficientCapacity { required });
        }
        if replacements.len() != removed {
            self.storage
                .copy_within(range.end..self.len, range.start + replacements.len());
        }
        self.storage[range.start..range.start + replacements.len()].copy_from_slice(replacements);
        self.len = required;
        Ok(())
    }

    fn finish(&mut self) -> usize {
        for glyph in &mut self.storage[..self.len] {
            glyph.flags &= OUTPUT_FLAGS;
        }
        self.len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupStatus {
    NotFound,
    Applied,
}

#[derive(Clone, Copy)]
pub struct LookupRequest<'a, 'text> {
    shape: &'a ShapeRequest<'text>,
    feature: [u8; 4],
    mask: GlyphMask,
}

impl<'a, 'text> LookupRequest<'a, 'text> {
    pub const fn new(shape: &'a ShapeRequest<'text>, feature: [u8; 4], mask: GlyphMask) -> Self {
        Self {
            shape,
            feature,
            mask,
        }
    }

    pub const fn shape(self) -> &'a ShapeRequest<'text> {
        self.shape
    }

    pub const fn feature(self) -> [u8; 4] {
        self.feature
    }

    pub const fn mask(self) -> GlyphMask {
        self.mask
    }

    pub const fn selects(self, glyph_mask: GlyphMask) -> bool {
        self.mask.bits() == GlyphMask::ALL.bits() || self.mask.intersects(glyph_mask)
    }
}

pub trait ShapingData {
    fn substitute(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError>;

    fn position(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError>;
}

pub trait ScriptProvider {
    fn script(&self) -> Script;

    fn substitute(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError>;

    fn position(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError>;
}

pub struct ScriptTypeface<'a> {
    glyphs: &'a dyn GlyphSource,
    shaping: &'a dyn ShapingData,
    scripts: &'a [&'a dyn ScriptProvider],
}

impl<'a> ScriptTypeface<'a> {
    pub const fn new(glyphs: &'a dyn GlyphSource, shaping: &'a dyn ShapingData) -> Self {
        Self {
            glyphs,
            shaping,
            scripts: &[],
        }
    }

    pub const fn with_scripts(mut self, scripts: &'a [&'a dyn ScriptProvider]) -> Self {
        self.scripts = scripts;
        self
    }

    fn provider(&self, script: Script) -> Option<&dyn ScriptProvider> {
        self.scripts
            .iter()
            .copied()
            .find(|provider| provider.script() == script)
    }

    fn map_into(
        &self,
        request: &ShapeRequest<'_>,
        output: &mut [ShapedGlyph],
    ) -> Result<usize, ShapeError> {
        let text = &request.text[request.range.clone()];
        let required = text.chars().count();
        if output.len() < required {
            return Err(ShapeError::InsufficientCapacity { required });
        }
        for (slot, (offset, character)) in output.iter_mut().zip(text.char_indices()) {
            let start = request.range.start + offset;
            let end = start + character.len_utf8();
            let glyph = self
                .glyphs
                .glyph_for(character)?
                .or(self.glyphs.notdef_glyph()?)
                .ok_or(ShapeError::MissingGlyph { offset: start })?;
            *slot = ShapedGlyph::new(glyph, super::TextRange::new(start as u32, end as u32));
        }
        Ok(required)
    }
}

impl Typeface for ScriptTypeface<'_> {
    fn id(&self) -> FontId {
        self.glyphs.id()
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        self.glyphs.metrics()
    }

    fn covers(&self, grapheme: &str) -> Result<bool, FontAccessError> {
        for character in grapheme.chars() {
            if self.glyphs.glyph_for(character)?.is_none() {
                return Ok(false);
            }
        }
        Ok(!grapheme.is_empty())
    }

    fn supports_complex_shaping(&self) -> bool {
        true
    }

    fn shape_into(
        &self,
        request: &ShapeRequest<'_>,
        output: &mut [ShapedGlyph],
    ) -> Result<usize, ShapeError> {
        validate_request(request)?;
        let Some(provider) = self.provider(request.script) else {
            return SimpleTypeface::new(self.glyphs).shape_into(request, output);
        };
        let count = self.map_into(request, output)?;
        let mut glyphs = GlyphBuffer::new(output, count)?;
        provider.substitute(request, self.shaping, &mut glyphs)?;
        for glyph in glyphs.glyphs_mut() {
            glyph.advance = self.glyphs.glyph_advance(glyph.glyph_id())?;
        }
        provider.position(request, self.shaping, &mut glyphs)?;
        let count = glyphs.finish();
        if request.direction == Direction::RightToLeft {
            output[..count].reverse();
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaping::{FlowPoint, TextRange};

    struct Font;

    impl GlyphSource for Font {
        fn id(&self) -> FontId {
            FontId::new(7)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics::default())
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
            Ok(Some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint {
                x: i32::from(glyph.value()),
                y: 0,
            })
        }
    }

    struct Data;

    impl ShapingData for Data {
        fn substitute(
            &self,
            request: LookupRequest<'_, '_>,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            if request.feature() != *b"test" {
                return Ok(LookupStatus::NotFound);
            }
            glyphs.set_glyph(0, GlyphId::new(9)).unwrap();
            Ok(LookupStatus::Applied)
        }

        fn position(
            &self,
            request: LookupRequest<'_, '_>,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            if request.feature() != *b"mark" {
                return Ok(LookupStatus::NotFound);
            }
            glyphs.get_mut(1).unwrap().offset.y = -3;
            Ok(LookupStatus::Applied)
        }
    }

    struct Provider;

    impl ScriptProvider for Provider {
        fn script(&self) -> Script {
            Script::Thai
        }

        fn substitute(
            &self,
            request: &ShapeRequest<'_>,
            font: &dyn ShapingData,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<(), ShapeError> {
            glyphs.set_mask(0, GlyphMask::new(1))?;
            font.substitute(
                LookupRequest::new(request, *b"test", GlyphMask::new(1)),
                glyphs,
            )?;
            Ok(())
        }

        fn position(
            &self,
            request: &ShapeRequest<'_>,
            font: &dyn ShapingData,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<(), ShapeError> {
            font.position(
                LookupRequest::new(request, *b"mark", GlyphMask::ALL),
                glyphs,
            )?;
            Ok(())
        }
    }

    #[test]
    fn glyph_buffer_replaces_in_place_and_reports_capacity() {
        let mut storage = [ShapedGlyph::default(); 3];
        storage[0] = ShapedGlyph::new(GlyphId::new(1), TextRange::new(0, 1));
        storage[1] = ShapedGlyph::new(GlyphId::new(2), TextRange::new(1, 2));
        let mut buffer = GlyphBuffer::new(&mut storage, 2).unwrap();
        let replacements = [
            ShapedGlyph::new(GlyphId::new(3), TextRange::new(0, 1)),
            ShapedGlyph::new(GlyphId::new(4), TextRange::new(0, 1)),
        ];
        buffer.replace(0..1, &replacements).unwrap();
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(2).unwrap().glyph_id(), GlyphId::new(2));

        let before = *buffer.get(0).unwrap();
        assert_eq!(
            buffer.replace(0..1, &replacements),
            Err(ShapeError::InsufficientCapacity { required: 4 })
        );
        assert_eq!(*buffer.get(0).unwrap(), before);
    }

    #[test]
    fn all_lookup_mask_selects_unmarked_glyphs() {
        let shape = ShapeRequest::new("a", 0..1, Direction::LeftToRight, Script::Latin);
        let all = LookupRequest::new(&shape, *b"ccmp", GlyphMask::ALL);
        let form = LookupRequest::new(&shape, *b"init", GlyphMask::new(1));

        assert!(all.selects(GlyphMask::NONE));
        assert!(form.selects(GlyphMask::new(1)));
        assert!(!form.selects(GlyphMask::NONE));
    }

    #[test]
    fn script_typeface_runs_provider_without_heap_storage() {
        let providers: [&dyn ScriptProvider; 1] = [&Provider];
        let face = ScriptTypeface::new(&Font, &Data).with_scripts(&providers);
        let text = "กิ";
        let request = ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Thai);
        let mut output = [ShapedGlyph::default(); 2];

        let count = face.shape_into(&request, &mut output).unwrap();

        assert_eq!(count, 2);
        assert_eq!(output[0].glyph_id(), GlyphId::new(9));
        assert_eq!(output[0].advance.x, 9);
        assert_eq!(output[1].offset.y, -3);
    }
}
