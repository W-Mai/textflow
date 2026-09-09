use crate::bidi::Direction;
use crate::unicode::Script;
use alloc::vec::Vec;
use core::ops::Range;
use core::str::FromStr;
use rustybuzz::{
    BufferClusterLevel, BufferFlags, Face, Feature, GlyphBuffer, Language, ShapePlan, UnicodeBuffer,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FaceKey(u64);

impl FaceKey {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

pub struct ShapingFace<'a> {
    key: FaceKey,
    face: Face<'a>,
}

impl<'a> ShapingFace<'a> {
    pub fn new(key: FaceKey, data: &'a [u8], face_index: u32) -> Option<Self> {
        Some(Self {
            key,
            face: Face::from_slice(data, face_index)?,
        })
    }

    pub const fn key(&self) -> FaceKey {
        self.key
    }

    pub fn units_per_em(&self) -> u16 {
        self.face.units_per_em() as u16
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: u32,
    pub range: Range<usize>,
}

impl FontFeature {
    pub const fn new(tag: [u8; 4], value: u32) -> Self {
        Self {
            tag,
            value,
            range: 0..usize::MAX,
        }
    }

    pub fn with_range(mut self, range: Range<usize>) -> Self {
        self.range = range;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeError {
    InvalidTextRange,
    InvalidLanguage,
    TextTooLong,
}

pub struct ShapeRequest<'a> {
    text: &'a str,
    range: Range<usize>,
    direction: Direction,
    script: Script,
    language: Option<&'a str>,
    features: &'a [FontFeature],
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub cluster: Range<usize>,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub unsafe_to_break: bool,
}

pub struct ShapedRun<'a> {
    pub text: Range<usize>,
    pub direction: Direction,
    pub glyphs: &'a [ShapedGlyph],
    pub x_advance: i32,
    pub y_advance: i32,
}

impl ShapedRun<'_> {
    pub fn is_safe_break(&self, byte_offset: usize) -> bool {
        if byte_offset == self.text.start || byte_offset == self.text.end {
            return true;
        }
        if byte_offset < self.text.start || byte_offset > self.text.end {
            return false;
        }
        !self.glyphs.iter().any(|glyph| {
            glyph.cluster.start < byte_offset && byte_offset < glyph.cluster.end
                || glyph.cluster.start == byte_offset && glyph.unsafe_to_break
        })
    }
}

#[derive(Default)]
pub struct ShapingWorkspace {
    buffer: Option<UnicodeBuffer>,
    glyphs: Vec<ShapedGlyph>,
    cluster_starts: Vec<usize>,
    plan: Option<CachedPlan>,
}

impl ShapingWorkspace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shape<'a>(
        &'a mut self,
        face: &ShapingFace<'_>,
        request: ShapeRequest<'_>,
    ) -> Result<ShapedRun<'a>, ShapeError> {
        validate_request(&request)?;
        self.prepare_plan(face, &request)?;

        let mut buffer = self.buffer.take().unwrap_or_default();
        buffer.push_str(&request.text[request.range.clone()]);
        buffer.set_pre_context(&request.text[..request.range.start]);
        buffer.set_post_context(&request.text[request.range.end..]);
        buffer.set_direction(rustybuzz_direction(request.direction));
        buffer.set_script(rustybuzz_script(request.script));
        if let Some(language) = self.plan.as_ref().and_then(|plan| plan.language.clone()) {
            buffer.set_language(language);
        }
        buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
        let mut flags = BufferFlags::empty();
        if request.range.start == 0 {
            flags |= BufferFlags::BEGINNING_OF_TEXT;
        }
        if request.range.end == request.text.len() {
            flags |= BufferFlags::END_OF_TEXT;
        }
        buffer.set_flags(flags);

        let glyph_buffer = {
            let plan = &self.plan.as_ref().unwrap().plan;
            rustybuzz::shape_with_plan(&face.face, plan, buffer)
        };
        self.copy_glyphs(&glyph_buffer, request.range.clone());
        self.buffer = Some(glyph_buffer.clear());

        let (x_advance, y_advance) = self.glyphs.iter().fold((0, 0), |sum, glyph| {
            (sum.0 + glyph.x_advance, sum.1 + glyph.y_advance)
        });
        Ok(ShapedRun {
            text: request.range,
            direction: request.direction,
            glyphs: &self.glyphs,
            x_advance,
            y_advance,
        })
    }

    fn prepare_plan(
        &mut self,
        face: &ShapingFace<'_>,
        request: &ShapeRequest<'_>,
    ) -> Result<(), ShapeError> {
        if self
            .plan
            .as_ref()
            .is_some_and(|plan| plan.matches(face.key, request))
        {
            return Ok(());
        }
        let language = request
            .language
            .map(Language::from_str)
            .transpose()
            .map_err(|_| ShapeError::InvalidLanguage)?;
        let features = request
            .features
            .iter()
            .map(|feature| {
                Feature::new(
                    rustybuzz::ttf_parser::Tag::from_bytes(&feature.tag),
                    feature.value,
                    feature.range.clone(),
                )
            })
            .collect::<Vec<_>>();
        let plan = ShapePlan::new(
            &face.face,
            rustybuzz_direction(request.direction),
            Some(rustybuzz_script(request.script)),
            language.as_ref(),
            &features,
        );
        self.plan = Some(CachedPlan {
            face: face.key,
            direction: request.direction,
            script: request.script,
            language,
            features: request.features.to_vec(),
            plan,
        });
        Ok(())
    }

    fn copy_glyphs(&mut self, buffer: &GlyphBuffer, text: Range<usize>) {
        self.cluster_starts.clear();
        self.cluster_starts.extend(
            buffer
                .glyph_infos()
                .iter()
                .map(|glyph| text.start + glyph.cluster as usize),
        );
        self.cluster_starts.push(text.end);
        self.cluster_starts.sort_unstable();
        self.cluster_starts.dedup();

        self.glyphs.clear();
        self.glyphs.extend(
            buffer
                .glyph_infos()
                .iter()
                .zip(buffer.glyph_positions())
                .map(|(info, position)| {
                    let cluster_start = text.start + info.cluster as usize;
                    let cluster_index = self.cluster_starts.binary_search(&cluster_start).unwrap();
                    ShapedGlyph {
                        glyph_id: info.glyph_id as u16,
                        cluster: cluster_start..self.cluster_starts[cluster_index + 1],
                        x_advance: position.x_advance,
                        y_advance: position.y_advance,
                        x_offset: position.x_offset,
                        y_offset: position.y_offset,
                        unsafe_to_break: info.unsafe_to_break(),
                    }
                }),
        );
    }
}

struct CachedPlan {
    face: FaceKey,
    direction: Direction,
    script: Script,
    language: Option<Language>,
    features: Vec<FontFeature>,
    plan: ShapePlan,
}

impl CachedPlan {
    fn matches(&self, face: FaceKey, request: &ShapeRequest<'_>) -> bool {
        self.face == face
            && self.direction == request.direction
            && self.script == request.script
            && self.features == request.features
            && match (&self.language, request.language) {
                (None, None) => true,
                (Some(cached), Some(requested)) => cached.as_str().eq_ignore_ascii_case(requested),
                _ => false,
            }
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
    if request.range.len() > u32::MAX as usize {
        return Err(ShapeError::TextTooLong);
    }
    Ok(())
}

fn rustybuzz_direction(direction: Direction) -> rustybuzz::Direction {
    match direction {
        Direction::LeftToRight => rustybuzz::Direction::LeftToRight,
        Direction::RightToLeft => rustybuzz::Direction::RightToLeft,
    }
}

fn rustybuzz_script(script: Script) -> rustybuzz::Script {
    let tag = rustybuzz::ttf_parser::Tag::from_bytes(&script.as_iso15924_tag().to_be_bytes());
    rustybuzz::Script::from_iso15924_tag(tag).unwrap_or(rustybuzz::script::UNKNOWN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    const FONT: &[u8] = include_bytes!("../tests/assets/roboto-latin-subset.ttf");

    fn face() -> ShapingFace<'static> {
        ShapingFace::new(FaceKey::new(7), FONT, 0).unwrap()
    }

    #[test]
    fn shapes_ligatures_with_global_cluster_ranges() {
        let face = face();
        let mut workspace = ShapingWorkspace::new();
        let text = "office";
        let run = workspace
            .shape(
                &face,
                ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin),
            )
            .unwrap();

        assert!(run.glyphs.len() < text.chars().count());
        assert!(run.glyphs.iter().any(|glyph| glyph.cluster.len() > 1));
        assert!(!run.is_safe_break(2));
    }

    #[test]
    fn feature_override_disables_standard_ligatures() {
        let face = face();
        let mut workspace = ShapingWorkspace::new();
        let text = "office";
        let features = [FontFeature::new(*b"liga", 0)];
        let run = workspace
            .shape(
                &face,
                ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin)
                    .with_features(&features),
            )
            .unwrap();

        assert_eq!(run.glyphs.len(), text.chars().count());
    }

    #[test]
    fn kerning_is_enabled_by_default() {
        let face = face();
        let mut workspace = ShapingWorkspace::new();
        let text = "AV";
        let kerned = workspace
            .shape(
                &face,
                ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin),
            )
            .unwrap()
            .x_advance;
        let features = [FontFeature::new(*b"kern", 0)];
        let unkerned = workspace
            .shape(
                &face,
                ShapeRequest::new(text, 0..text.len(), Direction::LeftToRight, Script::Latin)
                    .with_features(&features),
            )
            .unwrap()
            .x_advance;

        assert!(kerned < unkerned);
    }

    #[test]
    fn context_ranges_keep_global_offsets() {
        let face = face();
        let mut workspace = ShapingWorkspace::new();
        let text = "AV office";
        let run = workspace
            .shape(
                &face,
                ShapeRequest::new(text, 3..text.len(), Direction::LeftToRight, Script::Latin),
            )
            .unwrap();

        assert_eq!(run.text, 3..text.len());
        assert!(run.glyphs.iter().all(|glyph| glyph.cluster.start >= 3));
        assert!(run
            .glyphs
            .iter()
            .all(|glyph| glyph.cluster.end <= text.len()));
    }
}
