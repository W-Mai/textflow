use super::{
    FontFeature, GlyphBuffer, GlyphMask, LookupRequest, LookupStatus, ScriptProvider, ShapeError,
    ShapeRequest, TextRange,
};
use crate::unicode::{graphemes, Script};

const ALL: GlyphMask = GlyphMask::ALL;

fn enabled(request: &ShapeRequest<'_>, tag: [u8; 4], default: bool) -> bool {
    request
        .features
        .iter()
        .filter(|feature| feature.tag == tag && overlaps(feature, request))
        .fold(default, |_, feature| feature.value != 0)
}

fn overlaps(feature: &FontFeature, request: &ShapeRequest<'_>) -> bool {
    feature.range.end > request.range.start as u32 && feature.range.start < request.range.end as u32
}

fn substitute(
    request: &ShapeRequest<'_>,
    font: &dyn super::ShapingData,
    glyphs: &mut GlyphBuffer<'_>,
    tag: [u8; 4],
    mask: GlyphMask,
    default: bool,
) -> Result<LookupStatus, ShapeError> {
    if !enabled(request, tag, default) {
        return Ok(LookupStatus::NotFound);
    }
    font.substitute(LookupRequest::new(request, tag, mask), glyphs)
}

fn position(
    request: &ShapeRequest<'_>,
    font: &dyn super::ShapingData,
    glyphs: &mut GlyphBuffer<'_>,
    tag: [u8; 4],
    default: bool,
) -> Result<LookupStatus, ShapeError> {
    if !enabled(request, tag, default) {
        return Ok(LookupStatus::NotFound);
    }
    font.position(LookupRequest::new(request, tag, ALL), glyphs)
}

fn normalize_clusters(request: &ShapeRequest<'_>, glyphs: &mut GlyphBuffer<'_>) {
    let text = &request.text[request.range.clone()];
    for cluster in graphemes(text) {
        let range = TextRange::new(
            (request.range.start + cluster.range.start) as u32,
            (request.range.start + cluster.range.end) as u32,
        );
        let mut matched = 0;
        for glyph in glyphs.glyphs_mut() {
            if glyph.cluster.start >= range.start && glyph.cluster.start < range.end {
                glyph.cluster = range;
                matched += 1;
            }
        }
        if matched > 1 {
            for glyph in glyphs.glyphs_mut() {
                if glyph.cluster == range {
                    glyph.set_unsafe_to_break(true);
                }
            }
        }
    }
}

#[cfg(feature = "script-arabic")]
/// Built-in Arabic script provider.
pub const ARABIC: Arabic = Arabic;

#[cfg(feature = "script-arabic")]
/// Applies Arabic joining forms, ligatures, cursive attachment, and mark positioning.
pub struct Arabic;

#[cfg(feature = "script-arabic")]
const ISOL: GlyphMask = GlyphMask::new(1 << 0);
#[cfg(feature = "script-arabic")]
const INIT: GlyphMask = GlyphMask::new(1 << 1);
#[cfg(feature = "script-arabic")]
const MEDI: GlyphMask = GlyphMask::new(1 << 2);
#[cfg(feature = "script-arabic")]
const FINA: GlyphMask = GlyphMask::new(1 << 3);

#[cfg(feature = "script-arabic")]
#[derive(Clone, Copy, Eq, PartialEq)]
enum Joining {
    None,
    Right,
    Dual,
    Transparent,
}

#[cfg(feature = "script-arabic")]
impl Joining {
    const fn joins_previous(self) -> bool {
        matches!(self, Self::Right | Self::Dual)
    }

    const fn joins_next(self) -> bool {
        matches!(self, Self::Dual)
    }
}

#[cfg(feature = "script-arabic")]
fn joining(character: char) -> Joining {
    let scalar = character as u32;
    if matches!(
        scalar,
        0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED | 0x08D3..=0x08FF
    ) {
        return Joining::Transparent;
    }
    if matches!(
        scalar,
        0x0622..=0x0625
            | 0x0627
            | 0x0629
            | 0x062F..=0x0632
            | 0x0648
            | 0x0671..=0x0673
            | 0x0675..=0x0677
            | 0x0688..=0x0699
            | 0x06C0
            | 0x06C3..=0x06CB
            | 0x06CD
            | 0x06CF
            | 0x06D2..=0x06D3
    ) {
        return Joining::Right;
    }
    if matches!(
        scalar,
        0x0626
            | 0x0628
            | 0x062A..=0x062E
            | 0x0633..=0x063A
            | 0x0641..=0x0647
            | 0x0649..=0x064A
            | 0x066E..=0x066F
            | 0x0678..=0x0687
            | 0x069A..=0x06BF
            | 0x06CC
            | 0x06CE
            | 0x06D0..=0x06D1
            | 0x06FA..=0x06FC
    ) {
        return Joining::Dual;
    }
    Joining::None
}

#[cfg(feature = "script-arabic")]
fn character_at(
    request: &ShapeRequest<'_>,
    glyphs: &GlyphBuffer<'_>,
    index: usize,
) -> Option<char> {
    let start = usize::try_from(glyphs.get(index)?.cluster.start).ok()?;
    request.text.get(start..)?.chars().next()
}

#[cfg(feature = "script-arabic")]
fn neighbor(
    request: &ShapeRequest<'_>,
    glyphs: &GlyphBuffer<'_>,
    index: usize,
    previous: bool,
) -> Option<Joining> {
    if previous {
        for slot in (0..index).rev() {
            let class = joining(character_at(request, glyphs, slot)?);
            if class != Joining::Transparent {
                return Some(class);
            }
        }
    } else {
        for slot in index + 1..glyphs.len() {
            let class = joining(character_at(request, glyphs, slot)?);
            if class != Joining::Transparent {
                return Some(class);
            }
        }
    }
    None
}

#[cfg(feature = "script-arabic")]
fn assign_joining_masks(
    request: &ShapeRequest<'_>,
    glyphs: &mut GlyphBuffer<'_>,
) -> Result<bool, ShapeError> {
    let mut joined = false;
    for index in 0..glyphs.len() {
        let current =
            joining(character_at(request, glyphs, index).ok_or(ShapeError::InvalidGlyphRange)?);
        if current == Joining::Transparent {
            glyphs.set_mask(index, GlyphMask::NONE)?;
            continue;
        }
        let previous = neighbor(request, glyphs, index, true)
            .is_some_and(|class| class.joins_next() && current.joins_previous());
        let next = neighbor(request, glyphs, index, false)
            .is_some_and(|class| current.joins_next() && class.joins_previous());
        let mask = match (previous, next) {
            (false, false) => ISOL,
            (false, true) => INIT,
            (true, true) => MEDI,
            (true, false) => FINA,
        };
        joined |= previous || next;
        if previous || next {
            glyphs
                .get_mut(index)
                .expect("valid joining slot")
                .set_unsafe_to_break(true);
        }
        glyphs.set_mask(index, mask)?;
    }
    Ok(joined)
}

#[cfg(feature = "script-arabic")]
impl ScriptProvider for Arabic {
    fn script(&self) -> Script {
        Script::Arabic
    }

    fn substitute(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        let joined = assign_joining_masks(request, glyphs)?;
        normalize_clusters(request, glyphs);
        substitute(request, font, glyphs, *b"ccmp", ALL, true)?;
        substitute(request, font, glyphs, *b"locl", ALL, true)?;
        let mut forms = 0;
        for (tag, mask) in [
            (*b"isol", ISOL),
            (*b"fina", FINA),
            (*b"medi", MEDI),
            (*b"init", INIT),
        ] {
            forms += usize::from(
                substitute(request, font, glyphs, tag, mask, true)? == LookupStatus::Applied,
            );
        }
        if joined && forms == 0 {
            return Err(ShapeError::ShapingUnavailable);
        }
        for tag in [*b"rlig", *b"calt", *b"liga"] {
            substitute(request, font, glyphs, tag, ALL, true)?;
        }
        Ok(())
    }

    fn position(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        position(request, font, glyphs, *b"curs", true)?;
        position(request, font, glyphs, *b"kern", true)?;
        let has_marks = request.text[request.range.clone()]
            .chars()
            .any(|character| joining(character) == Joining::Transparent);
        let mark = position(request, font, glyphs, *b"mark", true)?;
        let mkmk = position(request, font, glyphs, *b"mkmk", true)?;
        if has_marks && mark == LookupStatus::NotFound && mkmk == LookupStatus::NotFound {
            return Err(ShapeError::ShapingUnavailable);
        }
        Ok(())
    }
}

#[cfg(feature = "script-thai")]
/// Built-in Thai script provider.
pub const THAI: Thai = Thai;

#[cfg(feature = "script-thai")]
/// Applies Thai cluster substitutions and mark positioning.
pub struct Thai;

#[cfg(feature = "script-thai")]
fn thai_mark(character: char) -> bool {
    matches!(character as u32, 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E)
}

#[cfg(feature = "script-thai")]
impl ScriptProvider for Thai {
    fn script(&self) -> Script {
        Script::Thai
    }

    fn substitute(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        normalize_clusters(request, glyphs);
        for tag in [*b"ccmp", *b"locl", *b"liga"] {
            substitute(request, font, glyphs, tag, ALL, true)?;
        }
        Ok(())
    }

    fn position(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        position(request, font, glyphs, *b"kern", true)?;
        let has_marks = request.text[request.range.clone()].chars().any(thai_mark);
        let mark = position(request, font, glyphs, *b"mark", true)?;
        let mkmk = position(request, font, glyphs, *b"mkmk", true)?;
        if has_marks && mark == LookupStatus::NotFound && mkmk == LookupStatus::NotFound {
            return Err(ShapeError::ShapingUnavailable);
        }
        Ok(())
    }
}

#[cfg(feature = "script-devanagari")]
/// Built-in Devanagari script provider.
pub const DEVANAGARI: Devanagari = Devanagari;

#[cfg(feature = "script-devanagari")]
/// Applies Devanagari conjunct forms, pre-base reordering, and mark positioning.
pub struct Devanagari;

#[cfg(feature = "script-devanagari")]
fn devanagari_consonant(character: char) -> bool {
    matches!(
        character as u32,
        0x0915..=0x0939 | 0x0958..=0x095F | 0x0978..=0x097F
    )
}

#[cfg(feature = "script-devanagari")]
fn devanagari_mark(character: char) -> bool {
    matches!(
        character as u32,
        0x0900..=0x0903
            | 0x093A..=0x094C
            | 0x094E..=0x0957
            | 0x0962..=0x0963
            | 0xA8E0..=0xA8F1
    )
}

#[cfg(feature = "script-devanagari")]
fn glyph_at_text_offset(glyphs: &GlyphBuffer<'_>, offset: u32) -> Option<usize> {
    glyphs
        .glyphs()
        .iter()
        .position(|glyph| glyph.cluster.start <= offset && offset < glyph.cluster.end)
}

#[cfg(feature = "script-devanagari")]
fn reorder_devanagari_prebase_matra(request: &ShapeRequest<'_>, glyphs: &mut GlyphBuffer<'_>) {
    let mut base = None;
    for (relative, character) in request.text[request.range.clone()].char_indices() {
        let offset = (request.range.start + relative) as u32;
        if devanagari_consonant(character) {
            base = Some(offset);
            continue;
        }
        if character != '\u{093F}' {
            continue;
        }
        let (Some(base_index), Some(matra_index)) = (
            base.and_then(|base| glyph_at_text_offset(glyphs, base)),
            glyph_at_text_offset(glyphs, offset),
        ) else {
            continue;
        };
        if base_index < matra_index {
            glyphs.glyphs_mut()[base_index..=matra_index].rotate_right(1);
        }
    }
}

#[cfg(feature = "script-devanagari")]
impl ScriptProvider for Devanagari {
    fn script(&self) -> Script {
        Script::Devanagari
    }

    fn substitute(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        for tag in [*b"locl", *b"ccmp", *b"nukt"] {
            substitute(request, font, glyphs, tag, ALL, true)?;
        }

        let mut conjunct_forms = 0;
        for tag in [
            *b"akhn", *b"rphf", *b"rkrf", *b"pref", *b"blwf", *b"half", *b"pstf", *b"vatu",
            *b"cjct",
        ] {
            conjunct_forms += usize::from(
                substitute(request, font, glyphs, tag, ALL, true)? == LookupStatus::Applied,
            );
        }
        if request.text[request.range.clone()].contains('\u{094D}') && conjunct_forms == 0 {
            return Err(ShapeError::ShapingUnavailable);
        }

        reorder_devanagari_prebase_matra(request, glyphs);
        for tag in [*b"pres", *b"abvs", *b"blws", *b"psts", *b"haln", *b"calt"] {
            substitute(request, font, glyphs, tag, ALL, true)?;
        }
        normalize_clusters(request, glyphs);
        Ok(())
    }

    fn position(
        &self,
        request: &ShapeRequest<'_>,
        font: &dyn super::ShapingData,
        glyphs: &mut GlyphBuffer<'_>,
    ) -> Result<(), ShapeError> {
        position(request, font, glyphs, *b"kern", true)?;
        position(request, font, glyphs, *b"dist", true)?;
        let has_marks = request.text[request.range.clone()]
            .chars()
            .any(devanagari_mark);
        let abvm = position(request, font, glyphs, *b"abvm", true)?;
        let blwm = position(request, font, glyphs, *b"blwm", true)?;
        let mark = position(request, font, glyphs, *b"mark", true)?;
        let mkmk = position(request, font, glyphs, *b"mkmk", true)?;
        if has_marks
            && [abvm, blwm, mark, mkmk]
                .iter()
                .all(|status| *status == LookupStatus::NotFound)
        {
            return Err(ShapeError::ShapingUnavailable);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidi::Direction;
    use crate::shaping::{
        FlowPoint, FontAccessError, FontId, GlyphId, GlyphSource, ShapedGlyph, ShapingData,
    };

    struct Font;

    impl GlyphSource for Font {
        fn id(&self) -> FontId {
            FontId::new(1)
        }
        fn metrics(&self) -> Result<super::super::FontMetrics, FontAccessError> {
            Ok(super::super::FontMetrics::default())
        }
        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
            Ok(Some(GlyphId::new(character as u16)))
        }
        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint { x: 10, y: 0 })
        }
    }

    struct Data;
    impl ShapingData for Data {
        fn substitute(
            &self,
            request: LookupRequest<'_, '_>,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            for index in 0..glyphs.len() {
                if glyphs.mask(index).is_some_and(|mask| request.selects(mask)) {
                    glyphs.set_glyph(index, GlyphId::new(100 + request.feature()[0] as u16))?;
                }
            }
            Ok(LookupStatus::Applied)
        }
        fn position(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }
    }

    struct MissingData;

    impl ShapingData for MissingData {
        fn substitute(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::NotFound)
        }

        fn position(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::NotFound)
        }
    }

    #[cfg(feature = "script-devanagari")]
    struct DevanagariData;

    #[cfg(feature = "script-devanagari")]
    impl ShapingData for DevanagariData {
        fn substitute(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }

        fn position(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }
    }

    #[cfg(feature = "script-arabic")]
    struct FormData;

    #[cfg(feature = "script-arabic")]
    impl ShapingData for FormData {
        fn substitute(
            &self,
            request: LookupRequest<'_, '_>,
            glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            let Some(form) = [*b"isol", *b"init", *b"medi", *b"fina"]
                .iter()
                .position(|tag| *tag == request.feature())
            else {
                return Ok(LookupStatus::NotFound);
            };
            for index in 0..glyphs.len() {
                if glyphs.mask(index).is_some_and(|mask| request.selects(mask)) {
                    glyphs.set_glyph(index, GlyphId::new(200 + form as u16))?;
                }
            }
            Ok(LookupStatus::Applied)
        }

        fn position(
            &self,
            _request: LookupRequest<'_, '_>,
            _glyphs: &mut GlyphBuffer<'_>,
        ) -> Result<LookupStatus, ShapeError> {
            Ok(LookupStatus::Applied)
        }
    }

    #[cfg(feature = "script-arabic")]
    #[test]
    fn arabic_assigns_contextual_forms_without_allocating() {
        let providers: [&dyn ScriptProvider; 1] = [&ARABIC];
        let face = super::super::ScriptTypeface::new(&Font, &Data).with_scripts(&providers);
        let text = "مرحبا";
        let request =
            ShapeRequest::new(text, 0..text.len(), Direction::RightToLeft, Script::Arabic);
        let mut output = [ShapedGlyph::default(); 5];
        let count = super::super::Typeface::shape_into(&face, &request, &mut output).unwrap();
        assert_eq!(count, 5);
        assert!(output
            .iter()
            .all(|glyph| glyph.glyph_id() == GlyphId::new(208)));
        assert!(output.iter().all(|glyph| glyph.unsafe_to_break()));
    }

    #[cfg(feature = "script-arabic")]
    #[test]
    fn arabic_joining_masks_skip_transparent_marks() {
        let text = "بَبب";
        let request =
            ShapeRequest::new(text, 0..text.len(), Direction::RightToLeft, Script::Arabic);
        let mut output = [ShapedGlyph::default(); 4];
        for (slot, (offset, character)) in output.iter_mut().zip(text.char_indices()) {
            *slot = ShapedGlyph::new(
                GlyphId::new(character as u16),
                TextRange::new(offset as u32, (offset + character.len_utf8()) as u32),
            );
        }
        let mut glyphs = GlyphBuffer::new(&mut output, 4).unwrap();

        assert!(assign_joining_masks(&request, &mut glyphs).unwrap());
        assert_eq!(glyphs.mask(0), Some(INIT));
        assert_eq!(glyphs.mask(1), Some(GlyphMask::NONE));
        assert_eq!(glyphs.mask(2), Some(MEDI));
        assert_eq!(glyphs.mask(3), Some(FINA));
    }

    #[cfg(feature = "script-arabic")]
    #[test]
    fn arabic_marks_do_not_receive_joining_forms() {
        let providers: [&dyn ScriptProvider; 1] = [&ARABIC];
        let face = super::super::ScriptTypeface::new(&Font, &FormData).with_scripts(&providers);
        let text = "بَب";
        let request =
            ShapeRequest::new(text, 0..text.len(), Direction::RightToLeft, Script::Arabic);
        let mut output = [ShapedGlyph::default(); 3];

        let count = super::super::Typeface::shape_into(&face, &request, &mut output).unwrap();

        assert_eq!(count, 3);
        assert_eq!(output[1].glyph_id(), GlyphId::new('\u{64e}' as u16));
        assert_ne!(output[0].cluster, output[1].cluster);
        assert_eq!(output[1].cluster, output[2].cluster);
    }

    #[cfg(feature = "script-arabic")]
    #[test]
    fn arabic_marks_require_positioning_data() {
        let providers: [&dyn ScriptProvider; 1] = [&ARABIC];
        let face = super::super::ScriptTypeface::new(&Font, &MissingData).with_scripts(&providers);
        let text = "بَ";
        let request =
            ShapeRequest::new(text, 0..text.len(), Direction::RightToLeft, Script::Arabic);
        let mut output = [ShapedGlyph::default(); 2];

        assert_eq!(
            super::super::Typeface::shape_into(&face, &request, &mut output),
            Err(ShapeError::ShapingUnavailable)
        );
    }

    #[cfg(feature = "script-thai")]
    #[test]
    fn thai_keeps_base_and_mark_in_one_cluster() {
        let providers: [&dyn ScriptProvider; 1] = [&THAI];
        let face = super::super::ScriptTypeface::new(&Font, &Data).with_scripts(&providers);
        let text = "กิ";
        let request = ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Thai);
        let mut output = [ShapedGlyph::default(); 2];
        let count = super::super::Typeface::shape_into(&face, &request, &mut output).unwrap();
        assert_eq!(count, 2);
        assert!(output
            .iter()
            .all(|glyph| glyph.glyph_id() == GlyphId::new(208)));
        assert_eq!(output[0].cluster, output[1].cluster);
        assert!(output.iter().all(|glyph| glyph.unsafe_to_break()));
    }

    #[cfg(feature = "script-thai")]
    #[test]
    fn thai_marks_require_positioning_data() {
        let providers: [&dyn ScriptProvider; 1] = [&THAI];
        let face = super::super::ScriptTypeface::new(&Font, &MissingData).with_scripts(&providers);
        let text = "กิ";
        let request = ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Thai);
        let mut output = [ShapedGlyph::default(); 2];

        assert_eq!(
            super::super::Typeface::shape_into(&face, &request, &mut output),
            Err(ShapeError::ShapingUnavailable)
        );
    }

    #[cfg(feature = "script-devanagari")]
    #[test]
    fn devanagari_reorders_prebase_matra_without_allocating() {
        let providers: [&dyn ScriptProvider; 1] = [&DEVANAGARI];
        let face =
            super::super::ScriptTypeface::new(&Font, &DevanagariData).with_scripts(&providers);
        let text = "कि";
        let request = ShapeRequest::new(
            text,
            0..text.len(),
            Direction::LeftToRight,
            Script::Devanagari,
        );
        let mut output = [ShapedGlyph::default(); 2];

        let count = super::super::Typeface::shape_into(&face, &request, &mut output).unwrap();

        assert_eq!(count, 2);
        assert_eq!(output[0].glyph_id(), GlyphId::new('\u{093F}' as u16));
        assert_eq!(output[1].glyph_id(), GlyphId::new('\u{0915}' as u16));
        assert_eq!(output[0].cluster, output[1].cluster);
        assert!(output.iter().all(|glyph| glyph.unsafe_to_break()));
    }

    #[cfg(feature = "script-devanagari")]
    #[test]
    fn devanagari_conjuncts_require_shaping_data() {
        let providers: [&dyn ScriptProvider; 1] = [&DEVANAGARI];
        let face = super::super::ScriptTypeface::new(&Font, &MissingData).with_scripts(&providers);
        let text = "क्ष";
        let request = ShapeRequest::new(
            text,
            0..text.len(),
            Direction::LeftToRight,
            Script::Devanagari,
        );
        let mut output = [ShapedGlyph::default(); 3];

        assert_eq!(
            super::super::Typeface::shape_into(&face, &request, &mut output),
            Err(ShapeError::ShapingUnavailable)
        );
    }

    #[cfg(feature = "script-devanagari")]
    #[test]
    fn devanagari_marks_require_positioning_data() {
        let providers: [&dyn ScriptProvider; 1] = [&DEVANAGARI];
        let face = super::super::ScriptTypeface::new(&Font, &MissingData).with_scripts(&providers);
        let text = "कि";
        let request = ShapeRequest::new(
            text,
            0..text.len(),
            Direction::LeftToRight,
            Script::Devanagari,
        );
        let mut output = [ShapedGlyph::default(); 2];

        assert_eq!(
            super::super::Typeface::shape_into(&face, &request, &mut output),
            Err(ShapeError::ShapingUnavailable)
        );
    }
}
