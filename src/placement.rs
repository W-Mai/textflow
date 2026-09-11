//! Allocation-free placement of linear paragraph output on sampled baselines.
//!
//! A baseline creates a fresh forward cursor for each pass. Cursors must return
//! deterministic samples for monotonically increasing distances. This permits
//! capacity and geometry validation before caller-owned output is modified.

use crate::layout::{LayoutLine, ParagraphLayout};
use crate::shaping::{CaretStop, FlowPoint, PositionedGlyph};

/// Fixed-point value representing a unit-length direction.
pub const UNIT_SCALE: i32 = 1 << 8;
const UNIT_TOLERANCE: i32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaselineError {
    InvalidGeometry,
    CoordinateOverflow,
    NonMonotonic { previous: i32, requested: i32 },
    OutOfRange { distance: i32, length: i32 },
    InvalidDirection,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BaselineSample {
    /// Baseline position in the layout coordinate space.
    pub position: FlowPoint,
    /// Normalized direction in `UNIT_SCALE` units.
    pub unit_tangent: FlowPoint,
}

/// Stateful forward-only access to one baseline.
pub trait BaselineCursor {
    fn length(&self) -> i32;

    fn sample_forward(&mut self, distance: i32) -> Result<BaselineSample, BaselineError>;
}

/// Reusable baseline geometry capable of creating independent cursors.
pub trait TextBaseline {
    type Cursor<'a>: BaselineCursor
    where
        Self: 'a;

    fn length(&self) -> i32;

    fn cursor(&self) -> Self::Cursor<'_>;
}

/// A straight baseline between two fixed-point positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineBaseline {
    start: FlowPoint,
    end: FlowPoint,
    length: i32,
    unit_tangent: FlowPoint,
}

impl LineBaseline {
    pub fn new(start: FlowPoint, end: FlowPoint) -> Result<Self, BaselineError> {
        let (length, unit_tangent) = segment_metrics(start, end)?;
        Ok(Self {
            start,
            end,
            length,
            unit_tangent,
        })
    }

    pub const fn start(self) -> FlowPoint {
        self.start
    }

    pub const fn end(self) -> FlowPoint {
        self.end
    }
}

impl TextBaseline for LineBaseline {
    type Cursor<'a> = LineCursor;

    fn length(&self) -> i32 {
        self.length
    }

    fn cursor(&self) -> Self::Cursor<'_> {
        LineCursor {
            baseline: *self,
            previous: None,
        }
    }
}

/// Forward cursor returned by [`LineBaseline`].
pub struct LineCursor {
    baseline: LineBaseline,
    previous: Option<i32>,
}

impl BaselineCursor for LineCursor {
    fn length(&self) -> i32 {
        self.baseline.length
    }

    fn sample_forward(&mut self, distance: i32) -> Result<BaselineSample, BaselineError> {
        validate_distance(self.previous, distance, self.baseline.length)?;
        self.previous = Some(distance);
        Ok(BaselineSample {
            position: interpolate(
                self.baseline.start,
                self.baseline.end,
                distance,
                self.baseline.length,
            )?,
            unit_tangent: self.baseline.unit_tangent,
        })
    }
}

/// A borrowed piecewise-linear baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolylineBaseline<'a> {
    points: &'a [FlowPoint],
    length: i32,
}

impl<'a> PolylineBaseline<'a> {
    pub fn new(points: &'a [FlowPoint]) -> Result<Self, BaselineError> {
        if points.len() < 2 {
            return Err(BaselineError::InvalidGeometry);
        }
        let mut length = 0i32;
        for segment in points.windows(2) {
            if segment[0] == segment[1] {
                continue;
            }
            let (segment_length, _) = segment_metrics(segment[0], segment[1])?;
            length = length
                .checked_add(segment_length)
                .ok_or(BaselineError::CoordinateOverflow)?;
        }
        if length == 0 {
            return Err(BaselineError::InvalidGeometry);
        }
        Ok(Self { points, length })
    }

    pub const fn points(self) -> &'a [FlowPoint] {
        self.points
    }
}

impl TextBaseline for PolylineBaseline<'_> {
    type Cursor<'a>
        = PolylineCursor<'a>
    where
        Self: 'a;

    fn length(&self) -> i32 {
        self.length
    }

    fn cursor(&self) -> Self::Cursor<'_> {
        PolylineCursor {
            baseline: *self,
            segment: 0,
            segment_start: 0,
            previous: None,
        }
    }
}

/// Forward cursor returned by [`PolylineBaseline`].
pub struct PolylineCursor<'a> {
    baseline: PolylineBaseline<'a>,
    segment: usize,
    segment_start: i32,
    previous: Option<i32>,
}

impl BaselineCursor for PolylineCursor<'_> {
    fn length(&self) -> i32 {
        self.baseline.length
    }

    fn sample_forward(&mut self, distance: i32) -> Result<BaselineSample, BaselineError> {
        validate_distance(self.previous, distance, self.baseline.length)?;
        self.previous = Some(distance);
        loop {
            let start = self.baseline.points[self.segment];
            let end = self.baseline.points[self.segment + 1];
            if start == end {
                self.segment = next_nonzero_segment(self.baseline.points, self.segment + 1)
                    .ok_or(BaselineError::InvalidGeometry)?;
                continue;
            }
            let (length, unit_tangent) = segment_metrics(start, end)?;
            let segment_end = self
                .segment_start
                .checked_add(length)
                .ok_or(BaselineError::CoordinateOverflow)?;
            if distance >= segment_end {
                let Some(next) = next_nonzero_segment(self.baseline.points, self.segment + 1)
                else {
                    return Ok(BaselineSample {
                        position: end,
                        unit_tangent,
                    });
                };
                self.segment = next;
                self.segment_start = segment_end;
                continue;
            }
            let local = distance
                .checked_sub(self.segment_start)
                .ok_or(BaselineError::CoordinateOverflow)?;
            return Ok(BaselineSample {
                position: interpolate(start, end, local.min(length), length)?,
                unit_tangent,
            });
        }
    }
}

fn next_nonzero_segment(points: &[FlowPoint], mut segment: usize) -> Option<usize> {
    while segment + 1 < points.len() {
        if points[segment] != points[segment + 1] {
            return Some(segment);
        }
        segment += 1;
    }
    None
}

/// Per-glyph geometry paired by index with a positioned glyph.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GlyphFrame {
    pub local_origin: FlowPoint,
    pub unit_tangent: FlowPoint,
}

/// Per-caret geometry paired by index with a linear caret stop.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaretFrame {
    pub local_origin: FlowPoint,
    pub unit_tangent: FlowPoint,
}

/// Caller storage required by one placement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlacementRequirements {
    pub glyphs: usize,
    pub carets: usize,
}

/// Caller-owned frame buffers used by [`BaselinePlacement::place_into`].
pub struct PlacementOutput<'a> {
    glyph_frames: &'a mut [GlyphFrame],
    caret_frames: Option<&'a mut [CaretFrame]>,
}

impl<'a> PlacementOutput<'a> {
    pub fn new(glyph_frames: &'a mut [GlyphFrame]) -> Self {
        Self {
            glyph_frames,
            caret_frames: None,
        }
    }

    pub fn with_carets(mut self, caret_frames: &'a mut [CaretFrame]) -> Self {
        self.caret_frames = Some(caret_frames);
        self
    }
}

/// Borrowed frame slices produced by path placement.
#[derive(Debug)]
pub struct PlacedText<'a> {
    glyph_frames: &'a [GlyphFrame],
    caret_frames: Option<&'a [CaretFrame]>,
}

impl<'a> PlacedText<'a> {
    pub const fn glyph_frames(&self) -> &[GlyphFrame] {
        self.glyph_frames
    }

    pub const fn caret_frames(&self) -> Option<&[CaretFrame]> {
        self.caret_frames
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementError {
    BaselineCount { required: usize, provided: usize },
    InsufficientGlyphCapacity { required: usize },
    InsufficientCaretCapacity { required: usize },
    InvalidLayout,
    NegativeAdvance { glyph: usize },
    NonMonotonicGlyphs { glyph: usize },
    NonMonotonicCarets { caret: usize },
    CoordinateOverflow,
    Baseline { line: usize, error: BaselineError },
}

/// Validated placement operation for a paragraph and one baseline per line.
pub struct BaselinePlacement<'a, 'layout, B> {
    layout: &'a ParagraphLayout<'layout>,
    baselines: &'a [B],
}

impl<'layout> ParagraphLayout<'layout> {
    pub fn place_on<'a, B>(&'a self, baselines: &'a [B]) -> BaselinePlacement<'a, 'layout, B>
    where
        B: TextBaseline,
    {
        BaselinePlacement {
            layout: self,
            baselines,
        }
    }
}

impl<B> BaselinePlacement<'_, '_, B>
where
    B: TextBaseline,
{
    pub const fn requirements(&self) -> PlacementRequirements {
        PlacementRequirements {
            glyphs: self.layout.glyphs().len(),
            carets: self.layout.carets().len(),
        }
    }

    pub fn preflight(&self) -> Result<PlacementRequirements, PlacementError> {
        self.validate(true)?;
        Ok(self.requirements())
    }

    pub fn place_into<'output>(
        &self,
        output: PlacementOutput<'output>,
    ) -> Result<PlacedText<'output>, PlacementError> {
        let required = self.requirements();
        if output.glyph_frames.len() < required.glyphs {
            return Err(PlacementError::InsufficientGlyphCapacity {
                required: required.glyphs,
            });
        }
        if let Some(carets) = output.caret_frames.as_ref() {
            if carets.len() < required.carets {
                return Err(PlacementError::InsufficientCaretCapacity {
                    required: required.carets,
                });
            }
        }
        let include_carets = output.caret_frames.is_some();
        self.validate(include_carets)?;

        let PlacementOutput {
            glyph_frames,
            caret_frames,
        } = output;
        self.write_glyphs(&mut glyph_frames[..required.glyphs])?;
        if let Some(carets) = caret_frames {
            self.write_carets(&mut carets[..required.carets])?;
            Ok(PlacedText {
                glyph_frames: &glyph_frames[..required.glyphs],
                caret_frames: Some(&carets[..required.carets]),
            })
        } else {
            Ok(PlacedText {
                glyph_frames: &glyph_frames[..required.glyphs],
                caret_frames: None,
            })
        }
    }

    /// Places only caret frames into caller-owned storage.
    pub fn place_carets_into<'output>(
        &self,
        output: &'output mut [CaretFrame],
    ) -> Result<&'output [CaretFrame], PlacementError> {
        let required = self.requirements().carets;
        if output.len() < required {
            return Err(PlacementError::InsufficientCaretCapacity { required });
        }
        self.validate_carets()?;
        self.write_carets(&mut output[..required])?;
        Ok(&output[..required])
    }

    fn validate(&self, include_carets: bool) -> Result<(), PlacementError> {
        if self.baselines.len() < self.layout.lines().len() {
            return Err(PlacementError::BaselineCount {
                required: self.layout.lines().len(),
                provided: self.baselines.len(),
            });
        }
        for (line_index, line) in self.layout.lines().iter().copied().enumerate() {
            let glyphs = self.line_glyphs(line)?;
            let mut cursor = self.baselines[line_index].cursor();
            validate_glyphs(line_index, glyphs, &mut cursor)?;
            if include_carets {
                let carets = self.line_carets(line)?;
                let mut cursor = self.baselines[line_index].cursor();
                validate_carets(line_index, carets, &mut cursor)?;
            }
        }
        Ok(())
    }

    fn validate_carets(&self) -> Result<(), PlacementError> {
        if self.baselines.len() < self.layout.lines().len() {
            return Err(PlacementError::BaselineCount {
                required: self.layout.lines().len(),
                provided: self.baselines.len(),
            });
        }
        for (line_index, line) in self.layout.lines().iter().copied().enumerate() {
            let carets = self.line_carets(line)?;
            let mut cursor = self.baselines[line_index].cursor();
            validate_carets(line_index, carets, &mut cursor)?;
        }
        Ok(())
    }

    fn write_glyphs(&self, output: &mut [GlyphFrame]) -> Result<(), PlacementError> {
        for (line_index, line) in self.layout.lines().iter().copied().enumerate() {
            let range = checked_range(line.glyphs(), self.layout.glyphs().len())?;
            let glyphs = &self.layout.glyphs()[range.clone()];
            let frames = &mut output[range];
            let mut cursor = self.baselines[line_index].cursor();
            for (line_glyph, (glyph, frame)) in glyphs.iter().zip(frames).enumerate() {
                *frame = place_glyph(line_index, line_glyph, line, glyph, &mut cursor)?;
            }
        }
        Ok(())
    }

    fn write_carets(&self, output: &mut [CaretFrame]) -> Result<(), PlacementError> {
        for (line_index, line) in self.layout.lines().iter().copied().enumerate() {
            let range = checked_range(line.carets(), self.layout.carets().len())?;
            let carets = &self.layout.carets()[range.clone()];
            let frames = &mut output[range];
            let mut cursor = self.baselines[line_index].cursor();
            for (line_caret, (caret, frame)) in carets.iter().zip(frames).enumerate() {
                *frame = place_caret(line_index, line_caret, line, caret, &mut cursor)?;
            }
        }
        Ok(())
    }

    fn line_glyphs(&self, line: LayoutLine) -> Result<&[PositionedGlyph], PlacementError> {
        let range = checked_range(line.glyphs(), self.layout.glyphs().len())?;
        Ok(&self.layout.glyphs()[range])
    }

    fn line_carets(&self, line: LayoutLine) -> Result<&[CaretStop], PlacementError> {
        let range = checked_range(line.carets(), self.layout.carets().len())?;
        Ok(&self.layout.carets()[range])
    }
}

fn validate_glyphs(
    line_index: usize,
    glyphs: &[PositionedGlyph],
    cursor: &mut impl BaselineCursor,
) -> Result<(), PlacementError> {
    let mut previous = None;
    for (glyph_index, glyph) in glyphs.iter().enumerate() {
        if glyph.advance.x < 0 {
            return Err(PlacementError::NegativeAdvance { glyph: glyph_index });
        }
        let distance = inline_distance(glyph.origin)?;
        if previous.is_some_and(|previous| distance < previous) {
            return Err(PlacementError::NonMonotonicGlyphs { glyph: glyph_index });
        }
        previous = Some(distance);
        let center = distance
            .checked_add(glyph.advance.x / 2)
            .ok_or(PlacementError::CoordinateOverflow)?;
        let _ = sample(line_index, cursor, distance)?;
        let _ = sample(line_index, cursor, center)?;
    }
    Ok(())
}

fn validate_carets(
    line_index: usize,
    carets: &[CaretStop],
    cursor: &mut impl BaselineCursor,
) -> Result<(), PlacementError> {
    let mut previous = None;
    for (caret_index, caret) in carets.iter().enumerate() {
        let distance = inline_distance(caret.position)?;
        if previous.is_some_and(|previous| distance < previous) {
            return Err(PlacementError::NonMonotonicCarets { caret: caret_index });
        }
        previous = Some(distance);
        let _ = sample(line_index, cursor, distance)?;
    }
    Ok(())
}

fn place_glyph(
    line_index: usize,
    glyph_index: usize,
    line: LayoutLine,
    glyph: &PositionedGlyph,
    cursor: &mut impl BaselineCursor,
) -> Result<GlyphFrame, PlacementError> {
    if glyph.advance.x < 0 {
        return Err(PlacementError::NegativeAdvance { glyph: glyph_index });
    }
    let distance = inline_distance(glyph.origin)?;
    let center = distance
        .checked_add(glyph.advance.x / 2)
        .ok_or(PlacementError::CoordinateOverflow)?;
    let origin = sample(line_index, cursor, distance)?;
    let center = sample(line_index, cursor, center)?;
    let normal = glyph
        .origin
        .y
        .checked_sub(line.origin().y)
        .and_then(|value| value.checked_add(glyph.offset.y))
        .ok_or(PlacementError::CoordinateOverflow)?;
    let local_origin =
        project_offset(origin.position, center.unit_tangent, glyph.offset.x, normal)?;
    Ok(GlyphFrame {
        local_origin,
        unit_tangent: center.unit_tangent,
    })
}

fn place_caret(
    line_index: usize,
    _caret_index: usize,
    line: LayoutLine,
    caret: &CaretStop,
    cursor: &mut impl BaselineCursor,
) -> Result<CaretFrame, PlacementError> {
    let distance = inline_distance(caret.position)?;
    let sample = sample(line_index, cursor, distance)?;
    let normal = caret
        .position
        .y
        .checked_sub(line.origin().y)
        .ok_or(PlacementError::CoordinateOverflow)?;
    Ok(CaretFrame {
        local_origin: project_offset(sample.position, sample.unit_tangent, 0, normal)?,
        unit_tangent: sample.unit_tangent,
    })
}

const fn inline_distance(point: FlowPoint) -> Result<i32, PlacementError> {
    Ok(point.x)
}

fn sample(
    line: usize,
    cursor: &mut impl BaselineCursor,
    distance: i32,
) -> Result<BaselineSample, PlacementError> {
    let sample = cursor
        .sample_forward(distance)
        .map_err(|error| PlacementError::Baseline { line, error })?;
    validate_unit(sample.unit_tangent).map_err(|error| PlacementError::Baseline { line, error })?;
    Ok(sample)
}

fn checked_range(
    range: crate::shaping::TextRange,
    len: usize,
) -> Result<core::ops::Range<usize>, PlacementError> {
    let start = range.start as usize;
    let end = range.end as usize;
    if start > end || end > len {
        return Err(PlacementError::InvalidLayout);
    }
    Ok(start..end)
}

fn project_offset(
    origin: FlowPoint,
    tangent: FlowPoint,
    inline: i32,
    normal: i32,
) -> Result<FlowPoint, PlacementError> {
    let x = i128::from(origin.x)
        .checked_add(
            (i128::from(inline) * i128::from(tangent.x)
                - i128::from(normal) * i128::from(tangent.y))
                / i128::from(UNIT_SCALE),
        )
        .ok_or(PlacementError::CoordinateOverflow)?;
    let y = i128::from(origin.y)
        .checked_add(
            (i128::from(inline) * i128::from(tangent.y)
                + i128::from(normal) * i128::from(tangent.x))
                / i128::from(UNIT_SCALE),
        )
        .ok_or(PlacementError::CoordinateOverflow)?;
    Ok(FlowPoint {
        x: i32::try_from(x).map_err(|_| PlacementError::CoordinateOverflow)?,
        y: i32::try_from(y).map_err(|_| PlacementError::CoordinateOverflow)?,
    })
}

fn validate_distance(
    previous: Option<i32>,
    distance: i32,
    length: i32,
) -> Result<(), BaselineError> {
    if let Some(previous) = previous {
        if distance < previous {
            return Err(BaselineError::NonMonotonic {
                previous,
                requested: distance,
            });
        }
    }
    if distance < 0 || distance > length {
        return Err(BaselineError::OutOfRange { distance, length });
    }
    Ok(())
}

fn validate_unit(direction: FlowPoint) -> Result<(), BaselineError> {
    let magnitude = i64::from(direction.x) * i64::from(direction.x)
        + i64::from(direction.y) * i64::from(direction.y);
    let minimum = i64::from(UNIT_SCALE - UNIT_TOLERANCE).pow(2);
    let maximum = i64::from(UNIT_SCALE + UNIT_TOLERANCE).pow(2);
    if magnitude < minimum || magnitude > maximum {
        return Err(BaselineError::InvalidDirection);
    }
    Ok(())
}

fn segment_metrics(start: FlowPoint, end: FlowPoint) -> Result<(i32, FlowPoint), BaselineError> {
    let dx = i128::from(end.x) - i128::from(start.x);
    let dy = i128::from(end.y) - i128::from(start.y);
    let squared = (dx * dx + dy * dy) as u128;
    if squared == 0 {
        return Err(BaselineError::InvalidGeometry);
    }
    let maximum = i128::from(i32::MAX) * i128::from(i32::MAX);
    if squared > maximum as u128 {
        return Err(BaselineError::CoordinateOverflow);
    }
    let length = rounded_sqrt(squared);
    let scaled_squared = squared * (UNIT_SCALE as u128) * (UNIT_SCALE as u128);
    let scaled_length = rounded_sqrt_u128(scaled_squared);
    let tangent = FlowPoint {
        x: i32::try_from(
            dx * i128::from(UNIT_SCALE) * i128::from(UNIT_SCALE)
                / i128::try_from(scaled_length).map_err(|_| BaselineError::CoordinateOverflow)?,
        )
        .map_err(|_| BaselineError::CoordinateOverflow)?,
        y: i32::try_from(
            dy * i128::from(UNIT_SCALE) * i128::from(UNIT_SCALE)
                / i128::try_from(scaled_length).map_err(|_| BaselineError::CoordinateOverflow)?,
        )
        .map_err(|_| BaselineError::CoordinateOverflow)?,
    };
    validate_unit(tangent)?;
    Ok((
        i32::try_from(length).map_err(|_| BaselineError::CoordinateOverflow)?,
        tangent,
    ))
}

fn interpolate(
    start: FlowPoint,
    end: FlowPoint,
    distance: i32,
    length: i32,
) -> Result<FlowPoint, BaselineError> {
    if distance == length {
        return Ok(end);
    }
    let x = i128::from(start.x)
        + (i128::from(end.x) - i128::from(start.x)) * i128::from(distance) / i128::from(length);
    let y = i128::from(start.y)
        + (i128::from(end.y) - i128::from(start.y)) * i128::from(distance) / i128::from(length);
    Ok(FlowPoint {
        x: i32::try_from(x).map_err(|_| BaselineError::CoordinateOverflow)?,
        y: i32::try_from(y).map_err(|_| BaselineError::CoordinateOverflow)?,
    })
}

fn rounded_sqrt(value: u128) -> u128 {
    rounded_sqrt_u128(value)
}

fn rounded_sqrt_u128(value: u128) -> u128 {
    let floor = integer_sqrt(value);
    let lower = value - floor * floor;
    let next = floor + 1;
    let upper = next * next - value;
    if upper < lower {
        next
    } else {
        floor
    }
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1u128;
    let mut high = 1u128 << (128 - value.leading_zeros() as usize).div_ceil(2);
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if middle <= value / middle {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaping::{
        FontAccessError, FontId, FontMetrics, GlyphId, GlyphSource, SimpleTypeface,
    };
    use crate::{LayoutScratch, TextFlow};

    struct Mono;

    impl GlyphSource for Mono {
        fn id(&self) -> FontId {
            FontId::new(1)
        }

        fn metrics(&self) -> Result<FontMetrics, FontAccessError> {
            Ok(FontMetrics {
                units_per_em: 256,
                ascender: 192,
                descender: -64,
                line_gap: 0,
            })
        }

        fn glyph_for(&self, character: char) -> Result<Option<GlyphId>, FontAccessError> {
            Ok(Some(GlyphId::new(character as u16)))
        }

        fn glyph_advance(&self, _glyph: GlyphId) -> Result<FlowPoint, FontAccessError> {
            Ok(FlowPoint { x: 256, y: 0 })
        }
    }

    #[test]
    fn line_cursor_is_forward_only_and_reaches_exact_endpoints() {
        let baseline =
            LineBaseline::new(FlowPoint { x: 10, y: 20 }, FlowPoint { x: 266, y: 276 }).unwrap();
        let mut cursor = baseline.cursor();

        assert_eq!(cursor.sample_forward(0).unwrap().position, baseline.start());
        assert_eq!(
            cursor.sample_forward(baseline.length()).unwrap().position,
            baseline.end()
        );
        assert!(matches!(
            cursor.sample_forward(0),
            Err(BaselineError::NonMonotonic { .. })
        ));
    }

    #[test]
    fn polyline_skips_zero_segments_and_uses_outgoing_vertex_tangent() {
        let points = [
            FlowPoint { x: 0, y: 0 },
            FlowPoint { x: 0, y: 0 },
            FlowPoint { x: 256, y: 0 },
            FlowPoint { x: 256, y: 256 },
        ];
        let baseline = PolylineBaseline::new(&points).unwrap();
        let mut cursor = baseline.cursor();

        let vertex = cursor.sample_forward(256).unwrap();
        assert_eq!(vertex.position, FlowPoint { x: 256, y: 0 });
        assert_eq!(vertex.unit_tangent, FlowPoint { x: 0, y: 256 });
    }

    #[test]
    fn polyline_reaches_the_last_nonzero_endpoint_before_trailing_duplicates() {
        let points = [
            FlowPoint { x: 0, y: 0 },
            FlowPoint { x: 256, y: 0 },
            FlowPoint { x: 256, y: 0 },
            FlowPoint { x: 256, y: 0 },
        ];
        let baseline = PolylineBaseline::new(&points).unwrap();
        let mut cursor = baseline.cursor();

        assert_eq!(
            cursor.sample_forward(baseline.length()).unwrap().position,
            FlowPoint { x: 256, y: 0 }
        );
    }

    #[test]
    fn extreme_coordinate_differences_return_errors_without_overflowing() {
        assert_eq!(
            LineBaseline::new(
                FlowPoint {
                    x: i32::MIN,
                    y: i32::MIN,
                },
                FlowPoint {
                    x: i32::MAX,
                    y: i32::MAX,
                },
            ),
            Err(BaselineError::CoordinateOverflow)
        );
    }

    #[test]
    fn sampled_arc_stays_borrowed_and_has_bounded_direction_error() {
        let quarter_arc = [
            FlowPoint { x: 256, y: 0 },
            FlowPoint { x: 237, y: 98 },
            FlowPoint { x: 181, y: 181 },
            FlowPoint { x: 98, y: 237 },
            FlowPoint { x: 0, y: 256 },
        ];
        let baseline = PolylineBaseline::new(&quarter_arc).unwrap();
        let mut cursor = baseline.cursor();
        let sample = cursor.sample_forward(baseline.length() / 2).unwrap();

        assert_eq!(baseline.points().as_ptr(), quarter_arc.as_ptr());
        validate_unit(sample.unit_tangent).unwrap();
        assert!(sample.position.x > 128 && sample.position.y > 128);
    }

    #[test]
    fn placement_preserves_linear_alignment_and_maps_offsets_to_the_frame() {
        let face = SimpleTypeface::new(&Mono);
        let mut scratch = LayoutScratch::<4, 8, 2, 8>::new();
        let layout = TextFlow::new("ab", 1024)
            .with_line_height(256)
            .with_alignment(crate::Alignment::Center)
            .layout_with_scratch(&[&face], &mut scratch)
            .unwrap();
        let baseline =
            [LineBaseline::new(FlowPoint { x: 0, y: 0 }, FlowPoint { x: 1024, y: 0 }).unwrap()];
        let mut glyphs = [GlyphFrame::default(); 2];
        let mut carets = [CaretFrame::default(); 3];
        let placed = layout
            .place_on(&baseline)
            .place_into(PlacementOutput::new(&mut glyphs).with_carets(&mut carets))
            .unwrap();

        assert_eq!(placed.glyph_frames()[0].local_origin.x, 256);
        assert_eq!(placed.glyph_frames()[1].local_origin.x, 512);
        assert_eq!(placed.caret_frames().unwrap()[0].local_origin.x, 256);
        assert_eq!(placed.caret_frames().unwrap()[2].local_origin.x, 768);
    }

    #[test]
    fn caret_only_placement_does_not_require_a_glyph_buffer() {
        let face = SimpleTypeface::new(&Mono);
        let mut scratch = LayoutScratch::<4, 8, 2, 8>::new();
        let layout = TextFlow::new("ab", 512)
            .with_line_height(256)
            .layout_with_scratch(&[&face], &mut scratch)
            .unwrap();
        let baseline =
            [LineBaseline::new(FlowPoint { x: 10, y: 20 }, FlowPoint { x: 522, y: 20 }).unwrap()];
        let mut carets = [CaretFrame::default(); 3];

        let placed = layout
            .place_on(&baseline)
            .place_carets_into(&mut carets)
            .unwrap();

        assert_eq!(placed[0].local_origin, FlowPoint { x: 10, y: 20 });
        assert_eq!(placed[2].local_origin, FlowPoint { x: 522, y: 20 });
    }

    #[test]
    fn insufficient_output_is_failure_atomic() {
        let face = SimpleTypeface::new(&Mono);
        let mut scratch = LayoutScratch::<4, 8, 2, 8>::new();
        let layout = TextFlow::new("ab", 512)
            .with_line_height(256)
            .layout_with_scratch(&[&face], &mut scratch)
            .unwrap();
        let baseline =
            [LineBaseline::new(FlowPoint { x: 0, y: 0 }, FlowPoint { x: 512, y: 0 }).unwrap()];
        let sentinel = GlyphFrame {
            local_origin: FlowPoint { x: 77, y: 88 },
            unit_tangent: FlowPoint { x: 99, y: 111 },
        };
        let mut glyphs = [sentinel; 1];

        assert_eq!(
            layout
                .place_on(&baseline)
                .place_into(PlacementOutput::new(&mut glyphs))
                .unwrap_err(),
            PlacementError::InsufficientGlyphCapacity { required: 2 }
        );
        assert_eq!(glyphs, [sentinel]);
    }

    #[test]
    fn geometry_failure_is_atomic_before_any_frame_is_written() {
        let face = SimpleTypeface::new(&Mono);
        let mut scratch = LayoutScratch::<4, 8, 2, 8>::new();
        let layout = TextFlow::new("ab", 512)
            .with_line_height(256)
            .layout_with_scratch(&[&face], &mut scratch)
            .unwrap();
        let short =
            [LineBaseline::new(FlowPoint { x: 0, y: 0 }, FlowPoint { x: 128, y: 0 }).unwrap()];
        let sentinel = GlyphFrame {
            local_origin: FlowPoint { x: 77, y: 88 },
            unit_tangent: FlowPoint { x: 99, y: 111 },
        };
        let mut glyphs = [sentinel; 2];

        assert!(matches!(
            layout
                .place_on(&short)
                .place_into(PlacementOutput::new(&mut glyphs)),
            Err(PlacementError::Baseline {
                error: BaselineError::OutOfRange { .. },
                ..
            })
        ));
        assert_eq!(glyphs, [sentinel; 2]);
    }
}
