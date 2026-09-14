use textflow::shaping::{
    FlowPoint, FontAccessError, FontId, FontMetrics, GlyphBuffer, GlyphId, GlyphMask, GlyphSource,
    LookupRequest, LookupStatus, ScriptProvider, ShapeError, ShapeRequest, ShapedGlyph,
    ShapingData, TextRange,
};
use textflow::unicode::Script;

pub const UNITS_PER_EM: u16 = 1000;

mod latin_data {
    include!(concat!(env!("OUT_DIR"), "/latin_font.rs"));
}

pub struct LatinFont;

impl GlyphSource for LatinFont {
    fn id(&self) -> FontId {
        FontId::new(2)
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        Ok(FontMetrics {
            units_per_em: UNITS_PER_EM,
            ascender: latin_data::LATIN_ASCENDER,
            descender: latin_data::LATIN_DESCENDER,
            line_gap: latin_data::LATIN_LINE_GAP,
        })
    }

    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
        let Ok(code) = u16::try_from(character as u32) else {
            return Ok(None);
        };
        Ok(latin_data::LATIN_ADVANCES
            .binary_search_by_key(&code, |entry| entry.0)
            .ok()
            .map(|_| GlyphId::new(code)))
    }

    fn notdef_glyph(&self) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(None)
    }

    fn glyph_advance(&self, glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
        let index = latin_data::LATIN_ADVANCES
            .binary_search_by_key(&glyph.value(), |entry| entry.0)
            .map_err(|_| FontAccessError::Unavailable)?;
        Ok(FlowPoint {
            x: latin_data::LATIN_ADVANCES[index].1,
            y: 0,
        })
    }

    fn kerning(&self, left: GlyphId, right: GlyphId) -> Result<i32, FontAccessError> {
        let pair = (left.value(), right.value());
        Ok(latin_data::LATIN_KERNING
            .binary_search_by_key(&pair, |entry| entry.0)
            .ok()
            .map_or(0, |index| latin_data::LATIN_KERNING[index].1))
    }
}

pub struct DemoFont;

pub fn character(glyph: GlyphId) -> Option<char> {
    char::from_u32(u32::from(glyph.value()))
}

impl GlyphSource for DemoFont {
    fn id(&self) -> FontId {
        FontId::new(1)
    }

    fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
        Ok(FontMetrics {
            units_per_em: UNITS_PER_EM,
            ascender: 800,
            descender: -200,
            line_gap: 0,
        })
    }

    fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(u16::try_from(character as u32).ok().map(GlyphId::new))
    }

    fn notdef_glyph(&self) -> Result<Option<GlyphId>, FontAccessError> {
        Ok(None)
    }

    fn glyph_advance(&self, glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
        let width = match character(glyph) {
            Some(' ' | '\t') => 300,
            Some('\u{FB01}' | '\u{FB02}') => 1020,
            Some(
                '\u{0300}'..='\u{036F}'
                | '\u{0591}'..='\u{05BD}'
                | '\u{064B}'..='\u{065F}'
                | '\u{0900}'..='\u{0903}'
                | '\u{093A}'..='\u{094C}'
                | '\u{0E31}'
                | '\u{0E34}'..='\u{0E3A}'
                | '\u{0E47}'..='\u{0E4E}',
            ) => 0,
            Some('\u{0600}'..='\u{06FF}' | '\u{FE70}'..='\u{FEFF}') => 600,
            Some('\u{2E80}'..='\u{9FFF}') => 1000,
            _ => 620,
        };
        Ok(FlowPoint { x: width, y: 0 })
    }
}

pub struct DemoShaping;

pub struct LatinShaping;

pub struct DemoLatin;

fn enabled(request: &ShapeRequest<'_>, tag: [u8; 4], default: bool) -> bool {
    request
        .features
        .iter()
        .filter(|feature| {
            feature.tag == tag
                && feature.range.end > request.range.start as u32
                && feature.range.start < request.range.end as u32
        })
        .fold(default, |_, feature| feature.value != 0)
}

impl ScriptProvider for DemoLatin {
    fn script(&self) -> Script {
        Script::Latin
    }

    fn substitute(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        if enabled(request, *b"liga", true) {
            font.substitute(
                LookupRequest::new(request, *b"liga", GlyphMask::ALL),
                glyphs,
            )?;
        }
        Ok(())
    }

    fn position(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        if enabled(request, *b"kern", true) {
            font.position(
                LookupRequest::new(request, *b"kern", GlyphMask::ALL),
                glyphs,
            )?;
        }
        Ok(())
    }
}

impl ShapingData for DemoShaping {
    fn substitute(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError> {
        if request.feature() == *b"liga" {
            let mut index = 0;
            let mut applied = false;
            while index + 1 < glyphs.len() {
                let left = *glyphs.get(index).ok_or(ShapeError::InvalidGlyphRange)?;
                let right = *glyphs.get(index + 1).ok_or(ShapeError::InvalidGlyphRange)?;
                let ligature = match (character(left.glyph_id()), character(right.glyph_id())) {
                    (Some('f'), Some('i')) => Some(0xFB01),
                    (Some('f'), Some('l')) => Some(0xFB02),
                    _ => None,
                };
                if let Some(glyph) = ligature {
                    let replacement = ShapedGlyph::new(
                        GlyphId::new(glyph),
                        TextRange::new(left.cluster.start, right.cluster.end),
                    );
                    glyphs.replace(index..index + 2, &[replacement])?;
                    applied = true;
                }
                index += 1;
            }
            return Ok(if applied {
                LookupStatus::Applied
            } else {
                LookupStatus::NotFound
            });
        }
        let form = match request.feature() {
            [b'i', b's', b'o', b'l'] => 0,
            [b'f', b'i', b'n', b'a'] => 1,
            [b'i', b'n', b'i', b't'] => 2,
            [b'm', b'e', b'd', b'i'] => 3,
            _ => return Ok(LookupStatus::NotFound),
        };
        let mut applied = false;
        for index in 0..glyphs.len() {
            let Some(mask) = glyphs.mask(index) else {
                continue;
            };
            if !request.selects(mask) {
                continue;
            }
            let Some(base) = glyphs
                .get(index)
                .and_then(|glyph| character(glyph.glyph_id()))
            else {
                continue;
            };
            if let Some(mapped) = arabic_form(base, form) {
                glyphs.set_glyph(index, GlyphId::new(mapped))?;
                applied = true;
            }
        }
        Ok(if applied {
            LookupStatus::Applied
        } else {
            LookupStatus::NotFound
        })
    }

    fn position(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError> {
        if request.feature() == *b"kern" {
            let mut applied = false;
            for index in 1..glyphs.len() {
                let left = glyphs
                    .get(index - 1)
                    .and_then(|glyph| character(glyph.glyph_id()));
                let right = glyphs
                    .get(index)
                    .and_then(|glyph| character(glyph.glyph_id()));
                let adjustment = match (left, right) {
                    (Some('A'), Some('V' | 'W' | 'Y' | 'v')) => -180,
                    (Some('T'), Some('o' | 'a' | 'e')) => -150,
                    (Some('W'), Some('a' | 'o')) => -120,
                    _ => 0,
                };
                if adjustment != 0 {
                    glyphs
                        .get_mut(index - 1)
                        .ok_or(ShapeError::InvalidGlyphRange)?
                        .advance
                        .x += adjustment;
                    applied = true;
                }
            }
            return Ok(if applied {
                LookupStatus::Applied
            } else {
                LookupStatus::NotFound
            });
        }
        if ![*b"mark", *b"mkmk", *b"abvm", *b"blwm"].contains(&request.feature()) {
            return Ok(LookupStatus::NotFound);
        }
        let mut applied = false;
        for glyph in glyphs.glyphs_mut() {
            let Some(ch) = character(glyph.glyph_id()) else {
                continue;
            };
            if is_mark(ch) {
                glyph.advance.x = 0;
                glyph.offset.y = -240;
                applied = true;
            }
        }
        Ok(if applied {
            LookupStatus::Applied
        } else {
            LookupStatus::NotFound
        })
    }
}

impl ShapingData for LatinShaping {
    fn substitute(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError> {
        DemoShaping.substitute(request, glyphs)
    }

    fn position(
        &self,
        request: LookupRequest<'_, '_>,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<LookupStatus, ShapeError> {
        if request.feature() != *b"kern" {
            return Ok(LookupStatus::NotFound);
        }
        let font = LatinFont;
        let mut applied = false;
        for index in 1..glyphs.len() {
            let left = glyphs
                .get(index - 1)
                .ok_or(ShapeError::InvalidGlyphRange)?
                .glyph_id();
            let right = glyphs
                .get(index)
                .ok_or(ShapeError::InvalidGlyphRange)?
                .glyph_id();
            let adjustment = font.kerning(left, right)?;
            if adjustment != 0 {
                glyphs
                    .get_mut(index - 1)
                    .ok_or(ShapeError::InvalidGlyphRange)?
                    .advance
                    .x += adjustment;
                applied = true;
            }
        }
        Ok(if applied {
            LookupStatus::Applied
        } else {
            LookupStatus::NotFound
        })
    }
}

fn is_mark(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x036F | 0x064B..=0x065F | 0x0900..=0x0903 | 0x093A..=0x094C | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E)
}

const ARABIC: &[(char, [u16; 4])] = &[
    ('ا', [0xFE8D, 0xFE8E, 0, 0]),
    ('ب', [0xFE8F, 0xFE90, 0xFE91, 0xFE92]),
    ('ت', [0xFE95, 0xFE96, 0xFE97, 0xFE98]),
    ('ث', [0xFE99, 0xFE9A, 0xFE9B, 0xFE9C]),
    ('ج', [0xFE9D, 0xFE9E, 0xFE9F, 0xFEA0]),
    ('ح', [0xFEA1, 0xFEA2, 0xFEA3, 0xFEA4]),
    ('خ', [0xFEA5, 0xFEA6, 0xFEA7, 0xFEA8]),
    ('د', [0xFEA9, 0xFEAA, 0, 0]),
    ('ذ', [0xFEAB, 0xFEAC, 0, 0]),
    ('ر', [0xFEAD, 0xFEAE, 0, 0]),
    ('ز', [0xFEAF, 0xFEB0, 0, 0]),
    ('س', [0xFEB1, 0xFEB2, 0xFEB3, 0xFEB4]),
    ('ش', [0xFEB5, 0xFEB6, 0xFEB7, 0xFEB8]),
    ('ص', [0xFEB9, 0xFEBA, 0xFEBB, 0xFEBC]),
    ('ض', [0xFEBD, 0xFEBE, 0xFEBF, 0xFEC0]),
    ('ط', [0xFEC1, 0xFEC2, 0xFEC3, 0xFEC4]),
    ('ظ', [0xFEC5, 0xFEC6, 0xFEC7, 0xFEC8]),
    ('ع', [0xFEC9, 0xFECA, 0xFECB, 0xFECC]),
    ('غ', [0xFECD, 0xFECE, 0xFECF, 0xFED0]),
    ('ف', [0xFED1, 0xFED2, 0xFED3, 0xFED4]),
    ('ق', [0xFED5, 0xFED6, 0xFED7, 0xFED8]),
    ('ك', [0xFED9, 0xFEDA, 0xFEDB, 0xFEDC]),
    ('ل', [0xFEDD, 0xFEDE, 0xFEDF, 0xFEE0]),
    ('م', [0xFEE1, 0xFEE2, 0xFEE3, 0xFEE4]),
    ('ن', [0xFEE5, 0xFEE6, 0xFEE7, 0xFEE8]),
    ('ه', [0xFEE9, 0xFEEA, 0xFEEB, 0xFEEC]),
    ('و', [0xFEED, 0xFEEE, 0, 0]),
    ('ي', [0xFEEF, 0xFEF0, 0xFEF1, 0xFEF2]),
];

fn arabic_form(base: char, form: usize) -> Option<u16> {
    ARABIC
        .iter()
        .find(|entry| entry.0 == base)
        .map(|entry| entry.1[form])
        .filter(|form| *form != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_forms_have_consistent_advance() {
        assert_eq!(arabic_form('ب', 2), Some(0xFE91));
        assert_eq!(
            DemoFont.glyph_advance(GlyphId::new('ب' as u16)).unwrap(),
            DemoFont.glyph_advance(GlyphId::new(0xFE91)).unwrap()
        );
    }
}
