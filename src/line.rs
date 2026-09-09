use crate::properties::{display_width, is_open_punctuation};
use crate::unicode::{graphemes, line_breaks, LineBreakKind};

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

impl LineInfo {
    pub fn slices<'a>(&self, string: &'a str) -> &'a str {
        string[self.position.start..self.position.brk.min(self.position.end)]
            .trim_end_matches([' ', '\t', '\r', '\n'])
    }
}

pub(crate) struct Line<'a> {
    text: &'a str,
    cursor: usize,
    max_width: usize,
    tab_width: usize,
}

impl<'a> Line<'a> {
    pub(crate) const fn new(text: &'a str, max_width: usize, tab_width: usize) -> Self {
        Self {
            text,
            cursor: 0,
            max_width,
            tab_width,
        }
    }

    fn next_start(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }
        let mut start = self.cursor;
        while let Some(character) = self.text[start..].chars().next() {
            if !matches!(character, ' ' | '\t') {
                break;
            }
            start += character.len_utf8();
        }
        start
    }

    fn emit(&mut self, start: usize, candidate: Candidate) -> LineInfo {
        let end = start + candidate.offset;
        self.cursor = end;
        LineInfo {
            position: LinePosition {
                start,
                end,
                brk: end,
            },
            line_height: 0,
            line_spacing: 0,
            real_width: candidate.width,
            ideal_width: candidate.width,
        }
    }
}

impl Iterator for Line<'_> {
    type Item = LineInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.next_start();
        if start == self.text.len() {
            self.cursor = start;
            return None;
        }

        let remaining = &self.text[start..];
        let mut breaks = line_breaks(remaining).peekable();
        let mut width = 0usize;
        let mut last_fit = None;
        let mut last_allowed = None;
        let mut last_open = None;
        let mut only_open = true;
        let mut previous_open = false;

        for cluster in graphemes(remaining) {
            let open = cluster.text.chars().next().is_some_and(is_open_punctuation);
            if open && !previous_open && !only_open {
                last_open = Some(Candidate {
                    offset: cluster.range.start,
                    width,
                });
            }
            if !open {
                only_open = false;
            }
            previous_open = open;

            let next_width = width.saturating_add(display_width(cluster.text, self.tab_width));
            if next_width > self.max_width {
                let whitespace = cluster.text.chars().all(char::is_whitespace);
                let candidate = if whitespace {
                    last_fit.or(last_allowed)
                } else {
                    latest(last_allowed, last_open).or(last_fit)
                };
                if let Some(candidate) = candidate {
                    return Some(self.emit(start, candidate));
                }
            }

            width = next_width;
            let candidate = Candidate {
                offset: cluster.range.end,
                width,
            };
            if width <= self.max_width || last_fit.is_none() {
                last_fit = Some(candidate);
            }

            while breaks
                .peek()
                .is_some_and(|line_break| line_break.offset <= cluster.range.end)
            {
                let line_break = breaks.next().unwrap();
                if line_break.offset != cluster.range.end {
                    continue;
                }
                match line_break.kind {
                    LineBreakKind::Mandatory => return Some(self.emit(start, candidate)),
                    LineBreakKind::Allowed if width <= self.max_width => {
                        last_allowed = Some(candidate);
                    }
                    LineBreakKind::Allowed => {}
                }
            }

            if width > self.max_width {
                return Some(self.emit(start, candidate));
            }
        }

        last_fit.map(|candidate| self.emit(start, candidate))
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    offset: usize,
    width: usize,
}

fn latest(left: Option<Candidate>, right: Option<Candidate>) -> Option<Candidate> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left.offset >= right.offset {
            left
        } else {
            right
        }),
        (left, right) => left.or(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    fn slices(text: &str, width: usize) -> Vec<&str> {
        Line::new(text, width, 4)
            .map(|line| line.slices(text))
            .collect()
    }

    #[test]
    fn preserves_text_across_emergency_breaks() {
        let text = "an \"apple\" tree";

        for width in 0..=text.len() {
            let mut rendered = String::new();
            let mut previous_break = 0;
            for line in Line::new(text, width, 4) {
                assert!(line.position.brk > previous_break);
                rendered.push_str(line.slices(text));
                previous_break = line.position.brk;
            }
            assert_eq!(
                rendered
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect::<String>(),
                text.chars()
                    .filter(|character| !character.is_whitespace())
                    .collect::<String>()
            );
        }
    }

    #[test]
    fn respects_graphemes_and_mandatory_breaks() {
        assert_eq!(slices("a\u{301}b", 1), ["a\u{301}", "b"]);
        assert_eq!(slices("a\r\n\r\nb", 8), ["a", "", "b"]);
    }
}
