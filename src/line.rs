use crate::word::{Word, WordType};
use std::iter::Peekable;

#[derive(Debug, Clone, PartialEq)]
pub struct LinePosition {
    pub start: usize,
    pub end: usize,
    pub brk: usize,
}

#[derive(Clone)]
pub struct LineInfo {
    pub position: LinePosition,
    pub line_height: usize,
    pub line_spacing: usize,
    pub real_width: usize,
    pub ideal_width: usize,
}

pub struct Line<'a> {
    text: &'a str,

    line_info_prev: Option<LineInfo>,
    max_width: usize,
    tab_width: usize,
}

impl Line<'_> {
    fn new(text: &str, max_width: usize, tab_width: usize) -> Line {
        Line {
            text,
            line_info_prev: None,
            max_width,
            tab_width,
        }
    }
}

impl Iterator for Line<'_> {
    type Item = LineInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line_info = LineInfo {
            position: LinePosition {
                start: self.line_info_prev.as_ref().map_or(0, |v| v.position.end),
                end: 0,
                brk: 0,
            },
            line_height: 0,
            line_spacing: 0,
            real_width: 0,
            ideal_width: 0,
        };

        let mut word_iter = Word::new(
            &self.text[line_info.position.start..],
            self.max_width,
            self.tab_width,
        )
        .peekable();

        loop {
            let word = word_iter.by_ref().peek()?.clone();
            if word.ideal_width >= self.max_width
                || word.word_type == WordType::NEWLINE
                || word.word_type == WordType::TAB
                || word.word_type == WordType::SPACE
                || word.word_type == WordType::RETURN
            {
                break;
            }
            word_iter.next();
            line_info.position.end = word.position.end;
        }

        self.line_info_prev = Some(line_info.clone());
        Some(line_info)
    }
}
