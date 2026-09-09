use crate::properties::{display_width, is_open_punctuation};
use crate::unicode::{graphemes, line_breaks, LineBreakKind};
use core::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Line<'a> {
    text: &'a str,
    range: Range<usize>,
    width: usize,
    height: usize,
    spacing: usize,
}

impl<'a> Line<'a> {
    pub const fn text(&self) -> &'a str {
        self.text
    }

    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    pub const fn spacing(&self) -> usize {
        self.spacing
    }

    pub(crate) fn set_metrics(&mut self, height: usize, spacing: usize) {
        self.height = height;
        self.spacing = spacing;
    }
}

pub(crate) struct Lines<'a> {
    text: &'a str,
    cursor: usize,
    max_width: usize,
    tab_width: usize,
}

impl<'a> Lines<'a> {
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

    fn emit(&mut self, start: usize, candidate: Candidate) -> Line<'a> {
        self.cursor = start + candidate.offset;
        let end = start + candidate.visible_offset;
        Line {
            text: &self.text[start..end],
            range: start..end,
            width: candidate.visible_width,
            height: 0,
            spacing: 0,
        }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;

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
        let mut visible_offset = 0;
        let mut visible_width = 0;

        for cluster in graphemes(remaining) {
            let open = cluster.text.chars().next().is_some_and(is_open_punctuation);
            if open && !previous_open && !only_open {
                last_open = Some(Candidate {
                    offset: cluster.range.start,
                    visible_offset,
                    visible_width,
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
            if !cluster.text.chars().all(char::is_whitespace) {
                visible_offset = cluster.range.end;
                visible_width = width;
            }
            let candidate = Candidate {
                offset: cluster.range.end,
                visible_offset,
                visible_width,
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
    visible_offset: usize,
    visible_width: usize,
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
        Lines::new(text, width, 4).map(|line| line.text()).collect()
    }

    #[test]
    fn preserves_text_across_emergency_breaks() {
        let text = "an \"apple\" tree";

        for width in 0..=text.len() {
            let mut rendered = String::new();
            let mut count = 0;
            for line in Lines::new(text, width, 4) {
                rendered.push_str(line.text());
                count += 1;
            }
            assert!(count <= text.chars().count());
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

    #[test]
    fn reports_visible_range_and_width() {
        let line = Lines::new("ab  cd", 3, 4).next().unwrap();

        assert_eq!(line.text(), "ab");
        assert_eq!(line.range(), 0..2);
        assert_eq!(line.width(), 2);
    }
}
