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

        let mut end = 0;
        let mut brk = 0;

        loop {
            let mut word = word_iter.by_ref().peek()?.clone();

            if word.position.brk != usize::MAX {
                end = word.position.end;
                brk = word.position.brk;

                if word.word_type == WordType::RETURN || word.word_type == WordType::NEWLINE {
                    word_iter.next();
                }
                break;
            }

            word_iter.next();
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
        let text = "八百标\n兵奔北坡炮兵\n并排北边跑炮兵怕把标兵碰标兵怕碰炮兵炮";
        // let flow = Line::new(text, 11, 4);
        //
        // for line in flow {
        //     println!("{:?}", line);
        // }

        let flow = Line::new(text, 16, 4);

        for line in flow {
            println!("{}", &text[line.position.start..line.position.brk]);
        }
    }
}
