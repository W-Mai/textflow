use crate::word::{Word, WordInfo, WordType};
use std::iter::Peekable;

#[derive(Debug, Clone, PartialEq)]
pub struct LinePosition {
    pub start: usize,
    pub end: usize,
    pub brk: usize,
}

#[derive(Clone, Debug)]
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
                start: self.line_info_prev.as_ref().map_or(0, |v| v.position.brk),
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

        let mut end;
        let mut brk;

        loop {
            let word = word_iter.by_ref().peek()?.clone();

            word_iter.next();

            let word_next = word_iter.by_ref().peek();
            if let Some(word_next) = word_next {
                if word_next.position.brk != usize::MAX {
                    end = word.position.end;
                    brk = word.position.end;

                    if word_next.word_type == WordType::RETURN
                        || word_next.word_type == WordType::NEWLINE
                    {
                        word_iter.next();
                    } else if word_next.word_type == WordType::PUNCTUATION {
                        end = word.position.start;
                        brk = word.position.start;
                    }
                    break;
                } else {
                    {
                        end = word.position.end;
                        brk = word.position.end;
                    }
                }
            } else {
                end = word.position.end;
                brk = word.position.end;
                word_iter.next();
                break;
            }
        }

        line_info.position.end = line_info.position.start + end;
        line_info.position.brk = line_info.position.start + brk;
        self.line_info_prev = Some(line_info.clone());
        Some(line_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_1() {
        let text = "The quick brown fox jumps over a lazy dog.";

        let n = 21;

        let flow = Line::new(text, n, 4);

        for line in flow {
            let mut display_buffer = String::new();
            display_buffer += &text[line.position.start..line.position.end];
            for _ in display_buffer.chars().count()..n - 1 {
                display_buffer += " ";
            }
            display_buffer += "|";
            println!(
                "{} {:?}",
                display_buffer,
                line.position.start..line.position.end
            );
        }
    }
}
