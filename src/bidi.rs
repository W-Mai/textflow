use alloc::vec::Vec;
use core::ops::Range;
use unicode_bidi::{BidiInfo, Level};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidiParagraph {
    pub text: Range<usize>,
    pub level: u8,
    pub direction: Direction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidiRun {
    pub text: Range<usize>,
    pub level: u8,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BidiError {
    ParagraphOutOfBounds,
    InvalidLineRange,
}

pub struct BidiText<'a> {
    info: BidiInfo<'a>,
}

impl<'a> BidiText<'a> {
    pub fn new(text: &'a str, base_direction: BaseDirection) -> Self {
        let level = match base_direction {
            BaseDirection::Auto => None,
            BaseDirection::LeftToRight => Some(Level::ltr()),
            BaseDirection::RightToLeft => Some(Level::rtl()),
        };
        Self {
            info: BidiInfo::new(text, level),
        }
    }

    pub fn paragraphs(&self) -> impl ExactSizeIterator<Item = BidiParagraph> + '_ {
        self.info.paragraphs.iter().map(|paragraph| BidiParagraph {
            text: paragraph.range.clone(),
            level: paragraph.level.number(),
            direction: direction(paragraph.level),
        })
    }

    pub fn logical_runs(&self, text: Range<usize>) -> Result<Vec<BidiRun>, BidiError> {
        if !valid_range(self.info.text, &text) {
            return Err(BidiError::InvalidLineRange);
        }
        Ok(level_runs(&self.info.levels, text))
    }

    pub fn visual_runs(
        &self,
        paragraph_index: usize,
        line: Range<usize>,
    ) -> Result<Vec<BidiRun>, BidiError> {
        let paragraph = self
            .info
            .paragraphs
            .get(paragraph_index)
            .ok_or(BidiError::ParagraphOutOfBounds)?;
        if !valid_range(self.info.text, &line)
            || line.start < paragraph.range.start
            || line.end > paragraph.range.end
        {
            return Err(BidiError::InvalidLineRange);
        }
        let (levels, runs) = self.info.visual_runs(paragraph, line);
        Ok(runs
            .into_iter()
            .map(|text| {
                let level = levels[text.start];
                BidiRun {
                    text,
                    level: level.number(),
                    direction: direction(level),
                }
            })
            .collect())
    }

    pub fn level_at(&self, byte_offset: usize) -> Option<u8> {
        self.info.levels.get(byte_offset).map(Level::number)
    }
}

fn level_runs(levels: &[Level], text: Range<usize>) -> Vec<BidiRun> {
    let mut runs = Vec::new();
    let mut start = text.start;

    while start < text.end {
        let level = levels[start];
        let mut end = start + 1;
        while end < text.end && levels[end] == level {
            end += 1;
        }
        runs.push(BidiRun {
            text: start..end,
            level: level.number(),
            direction: direction(level),
        });
        start = end;
    }
    runs
}

fn valid_range(text: &str, range: &Range<usize>) -> bool {
    range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
}

fn direction(level: Level) -> Direction {
    if level.is_rtl() {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    #[test]
    fn resolves_mixed_direction_runs() {
        let text = "abc אבג";
        let bidi = BidiText::new(text, BaseDirection::Auto);
        let paragraph = bidi.paragraphs().next().unwrap();
        let runs = bidi.logical_runs(paragraph.text).unwrap();

        assert_eq!(runs.len(), 2);
        assert_eq!(&text[runs[0].text.clone()], "abc ");
        assert_eq!(runs[0].direction, Direction::LeftToRight);
        assert_eq!(&text[runs[1].text.clone()], "אבג");
        assert_eq!(runs[1].direction, Direction::RightToLeft);
    }

    #[test]
    fn visual_runs_cover_the_line_once() {
        let text = "אבג abc";
        let bidi = BidiText::new(text, BaseDirection::RightToLeft);
        let runs = bidi.visual_runs(0, 0..text.len()).unwrap();
        let covered = runs.iter().map(|run| run.text.len()).sum::<usize>();

        assert_eq!(covered, text.len());
        assert_eq!(&text[runs[0].text.clone()], "abc");
        assert_eq!(runs[0].direction, Direction::LeftToRight);
        assert_eq!(&text[runs[1].text.clone()], "אבג ");
        assert_eq!(runs[1].direction, Direction::RightToLeft);
    }

    #[test]
    fn rejects_ranges_outside_a_paragraph() {
        let text = "abc\nאבג";
        let bidi = BidiText::new(text, BaseDirection::Auto);

        assert_eq!(
            bidi.visual_runs(0, 0..text.len()),
            Err(BidiError::InvalidLineRange)
        );
        assert_eq!(
            bidi.visual_runs(9, 0..1),
            Err(BidiError::ParagraphOutOfBounds)
        );
    }
}
