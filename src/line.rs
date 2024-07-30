use crate::word::{Word, WordType};
use peekmore::PeekMore;

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
    long_break: bool,
}

#[allow(dead_code)]
impl Line<'_> {
    pub fn new(text: &str, max_width: usize, tab_width: usize) -> Line {
        Line {
            text,
            line_info_prev: None,
            max_width,
            tab_width,
            long_break: false,
        }
    }

    pub fn with_max_width(mut self, max_width: usize) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn with_tab_width(mut self, tab_width: usize) -> Self {
        self.tab_width = tab_width;
        self
    }

    pub fn with_long_break(mut self, long_break: bool) -> Self {
        self.long_break = long_break;
        self
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
        .peekmore();

        let mut end;
        let mut brk;
        let mut is_line_leading = true;

        loop {
            let word = word_iter.peek()?.clone();

            if is_line_leading
                && self.long_break == true
                && word.position.brk != usize::MAX
                && word.position.brk != word.position.end
                && !(word.word_type == WordType::RETURN || word.word_type == WordType::NEWLINE)
            {
                end = word.position.end;
                brk = word.position.brk;
                word_iter.next();
                break;
            }

            if word.word_type == WordType::OPEN_PUNCTUATION {
                word_iter.advance_cursor();
                if let Some(word_next) = word_iter.peek() {
                    if word_next.position.brk != usize::MAX
                        && word_next.position.brk != word_next.position.end
                    {
                        if is_line_leading {
                            continue;
                        }

                            end = word.position.start;
                            brk = word.position.start;
                        break;
                    }
                }
            }

            word_iter.next();

            let word_next = word_iter.peek();
            if let Some(word_next) = word_next {
                if word_next.position.brk != usize::MAX
                    && word_next.position.brk != word_next.position.end
                {
                    end = word.position.end;
                    brk = word.position.end;

                    if word_next.word_type == WordType::RETURN
                        || word_next.word_type == WordType::NEWLINE
                    {
                        word_iter.next();
                    } else if word.word_type != WordType::CLOSE_PUNCTUATION
                        && word_next.word_type == WordType::CLOSE_PUNCTUATION
                    {
                        end = word.position.start;
                        brk = word.position.start;
                    }
                    break;
                }
            } else {
                end = word.position.end;
                brk = word.position.end;
                word_iter.next();
                break;
            }

            is_line_leading = false;
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

    fn do_a_test(text: &str, n: usize) {
        let flow = Line::new(text, n, 4).with_long_break(true);

        for line in flow {
            let mut display_buffer = String::from(&text[line.position.start..line.position.brk]);
            if display_buffer == "\n" {
                continue;
            }
            let text_len = display_buffer.len();

            // calc real width of the line: wide char is 2, others are 1
            let full_width =
                display_buffer
                    .chars()
                    .fold(0, |acc, ch| if ch.is_ascii() { acc + 1 } else { acc + 2 });

            for _ in full_width..n {
                display_buffer += " ";
            }
            println!(
                "{}| [{:3?}) len: {:3?}",
                display_buffer,
                line.position.start..line.position.end,
                text_len
            );
        }
        println!();
    }

    #[test]
    fn test_line_1() {
        do_a_test("The quick brown fox jumps over a lazy dog.", 15);
    }

    #[test]
    fn test_line_2() {
        do_a_test("八百标兵奔北坡炮兵并排北边跑666中英文测试。The quick brown fox jumps over a lazy dog. abcdefghijklmnopq rstuvwxyz", 14);
    }

    #[test]
    fn test_line_3() {
        do_a_test(
            "为了提供更好的服务和服务。\n请您在使用前充分阅读《TextFlow 使用隐私 Policy》",
            25,
        );
    }

    #[test]
    fn test_line_4() {
        do_a_test("《Loooooooooooooooong Text》", 20);
    }
}
