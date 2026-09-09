use crate::unicode::Script;
use core::ops::Range;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BaseDirection {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

impl Direction {
    pub const fn from_level(level: u8) -> Self {
        if level & 1 == 0 {
            Self::LeftToRight
        } else {
            Self::RightToLeft
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidiRun {
    pub text: Range<usize>,
    pub level: u8,
    pub direction: Direction,
}

impl BidiRun {
    pub const fn empty() -> Self {
        Self {
            text: 0..0,
            level: 0,
            direction: Direction::LeftToRight,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BidiError {
    InsufficientCapacity { required: usize },
    UnsupportedControl { offset: usize },
    InvalidTextRange,
}

pub struct BidiText<'a> {
    text: Range<usize>,
    direction: Direction,
    runs: &'a [BidiRun],
}

impl<'a> BidiText<'a> {
    pub fn resolve(
        text: &str,
        range: Range<usize>,
        base_direction: BaseDirection,
        output: &'a mut [BidiRun],
    ) -> Result<Self, BidiError> {
        validate_range(text, &range)?;
        if let Some(offset) = text[range.clone()]
            .char_indices()
            .find_map(|(offset, character)| {
                is_bidi_control(character).then_some(range.start + offset)
            })
        {
            return Err(BidiError::UnsupportedControl { offset });
        }
        let direction = resolve_base_direction(&text[range.clone()], base_direction);
        let mut writer = RunWriter::new(output, direction);
        resolve_runs(text, range.clone(), direction, &mut writer);
        let count = writer.finish()?;
        Ok(Self {
            text: range,
            direction,
            runs: &output[..count],
        })
    }

    pub const fn direction(&self) -> Direction {
        self.direction
    }

    pub fn text(&self) -> Range<usize> {
        self.text.clone()
    }

    pub const fn logical_runs(&self) -> &[BidiRun] {
        self.runs
    }

    pub fn visual_runs_into<'b>(
        &self,
        output: &'b mut [BidiRun],
    ) -> Result<&'b [BidiRun], BidiError> {
        if output.len() < self.runs.len() {
            return Err(BidiError::InsufficientCapacity {
                required: self.runs.len(),
            });
        }
        output[..self.runs.len()].clone_from_slice(self.runs);
        let max_level = output[..self.runs.len()]
            .iter()
            .map(|run| run.level)
            .max()
            .unwrap_or(0);
        let min_odd = output[..self.runs.len()]
            .iter()
            .filter(|run| run.level % 2 == 1)
            .map(|run| run.level)
            .min();
        if let Some(min_odd) = min_odd {
            for level in (min_odd..=max_level).rev() {
                reverse_level_sequences(&mut output[..self.runs.len()], level);
            }
        }
        Ok(&output[..self.runs.len()])
    }
}

fn resolve_runs(text: &str, range: Range<usize>, base: Direction, writer: &mut RunWriter<'_>) {
    let mut segment_start = range.start;
    let mut active = base;
    let mut neutral_start = None;

    for (local_offset, character) in text[range.clone()].char_indices() {
        let offset = range.start + local_offset;
        match strong_direction(character) {
            Some(next) => {
                if let Some(neutral) = neutral_start.take() {
                    writer.push(segment_start..neutral, active);
                    let neutral_direction = if active == next { active } else { base };
                    writer.push(neutral..offset, neutral_direction);
                    segment_start = offset;
                } else if next != active {
                    writer.push(segment_start..offset, active);
                    segment_start = offset;
                }
                active = next;
            }
            None => {
                neutral_start.get_or_insert(offset);
            }
        }
    }

    if let Some(neutral) = neutral_start {
        writer.push(segment_start..neutral, active);
        writer.push(neutral..range.end, base);
    } else {
        writer.push(segment_start..range.end, active);
    }
}

struct RunWriter<'a> {
    output: &'a mut [BidiRun],
    base: Direction,
    count: usize,
    last_direction: Option<Direction>,
}

impl<'a> RunWriter<'a> {
    fn new(output: &'a mut [BidiRun], base: Direction) -> Self {
        Self {
            output,
            base,
            count: 0,
            last_direction: None,
        }
    }

    fn push(&mut self, text: Range<usize>, direction: Direction) {
        if text.is_empty() {
            return;
        }
        if self.last_direction == Some(direction) && self.count > 0 {
            if let Some(last) = self.output.get_mut(self.count - 1) {
                last.text.end = text.end;
            }
            return;
        }
        if let Some(slot) = self.output.get_mut(self.count) {
            *slot = BidiRun {
                text: text.clone(),
                level: level(self.base, direction),
                direction,
            };
        }
        self.count += 1;
        self.last_direction = Some(direction);
    }

    fn finish(self) -> Result<usize, BidiError> {
        if self.count > self.output.len() {
            Err(BidiError::InsufficientCapacity {
                required: self.count,
            })
        } else {
            Ok(self.count)
        }
    }
}

fn resolve_base_direction(text: &str, requested: BaseDirection) -> Direction {
    match requested {
        BaseDirection::LeftToRight => Direction::LeftToRight,
        BaseDirection::RightToLeft => Direction::RightToLeft,
        BaseDirection::Auto => text
            .chars()
            .find_map(strong_direction)
            .unwrap_or(Direction::LeftToRight),
    }
}

fn strong_direction(character: char) -> Option<Direction> {
    if character.is_ascii_digit() || matches!(character as u32, 0x0660..=0x0669 | 0x06F0..=0x06F9) {
        return Some(Direction::LeftToRight);
    }
    match Script::of(character) {
        Script::Hebrew | Script::Arabic => Some(Direction::RightToLeft),
        Script::Latin
        | Script::Greek
        | Script::Cyrillic
        | Script::Devanagari
        | Script::Han
        | Script::Hiragana
        | Script::Katakana
        | Script::Hangul => Some(Direction::LeftToRight),
        Script::Common | Script::Inherited | Script::Unknown => None,
    }
}

fn level(base: Direction, direction: Direction) -> u8 {
    match (base, direction) {
        (Direction::LeftToRight, Direction::LeftToRight) => 0,
        (Direction::LeftToRight, Direction::RightToLeft) => 1,
        (Direction::RightToLeft, Direction::RightToLeft) => 1,
        (Direction::RightToLeft, Direction::LeftToRight) => 2,
    }
}

fn reverse_level_sequences(runs: &mut [BidiRun], level: u8) {
    let mut start = 0;
    while start < runs.len() {
        if runs[start].level < level {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < runs.len() && runs[end].level >= level {
            end += 1;
        }
        runs[start..end].reverse();
        start = end;
    }
}

fn validate_range(text: &str, range: &Range<usize>) -> Result<(), BidiError> {
    if range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
    {
        Ok(())
    } else {
        Err(BidiError::InvalidTextRange)
    }
}

fn is_bidi_control(character: char) -> bool {
    matches!(
        character as u32,
        0x061C | 0x200E..=0x200F | 0x202A..=0x202E | 0x2066..=0x2069
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    fn slots<const N: usize>() -> [BidiRun; N] {
        core::array::from_fn(|_| BidiRun::empty())
    }

    #[test]
    fn resolves_common_mixed_runs_without_allocation() {
        let text = "abc אבג 123";
        let mut logical = slots::<8>();
        let bidi =
            BidiText::resolve(text, 0..text.len(), BaseDirection::Auto, &mut logical).unwrap();

        assert_eq!(bidi.direction(), Direction::LeftToRight);
        assert_eq!(bidi.logical_runs().len(), 3);
        assert_eq!(&text[bidi.logical_runs()[0].text.clone()], "abc ");
        assert_eq!(&text[bidi.logical_runs()[1].text.clone()], "אבג");
        assert_eq!(&text[bidi.logical_runs()[2].text.clone()], " 123");
    }

    #[test]
    fn reorders_rtl_runs_into_visual_order() {
        let text = "אבג abc";
        let mut logical = slots::<8>();
        let bidi = BidiText::resolve(
            text,
            0..text.len(),
            BaseDirection::RightToLeft,
            &mut logical,
        )
        .unwrap();
        let mut visual = slots::<8>();
        let visual = bidi.visual_runs_into(&mut visual).unwrap();

        assert_eq!(&text[visual[0].text.clone()], "abc");
        assert_eq!(visual[0].level, 2);
        assert_eq!(&text[visual[1].text.clone()], "אבג ");
        assert_eq!(visual[1].level, 1);
    }

    #[test]
    fn reports_capacity_and_unsupported_controls() {
        let mut one = slots::<1>();
        let mixed = "abc אבג";
        assert_eq!(
            BidiText::resolve(mixed, 0..mixed.len(), BaseDirection::Auto, &mut one)
                .err()
                .unwrap(),
            BidiError::InsufficientCapacity { required: 2 }
        );
        let controlled = "abc\u{202E}def";
        assert_eq!(
            BidiText::resolve(
                controlled,
                0..controlled.len(),
                BaseDirection::Auto,
                &mut one
            )
            .err()
            .unwrap(),
            BidiError::UnsupportedControl { offset: 3 }
        );
    }
}
