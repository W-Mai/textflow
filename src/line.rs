use crate::word::{Word, WordInfo, WordType};
use peekmore::PeekMore;

/// Flags for Line
///
/// - FLAG_BREAK_NONE: No break
/// - FLAG_BREAK_ALL: Break all
type Flags = u16;
const FLAG_BREAK_NONE: u16 = 0000_0000_0000_0000;
const FLAG_BREAK_ALL: u16 = 0000_0000_0000_0001;

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
        &string[self.position.start..self.position.brk.min(self.position.end)]
    }
}

pub struct Line<'a> {
    text: &'a str,

    line_info_prev: Option<LineInfo>,
    max_width: usize,
    tab_width: usize,
    long_break: bool,
    flags: Flags,
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
            flags: FLAG_BREAK_NONE,
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

    pub fn with_flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
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

        let break_all = (self.flags & FLAG_BREAK_ALL) == FLAG_BREAK_ALL;

        let mut end;
        let mut brk;
        let mut is_line_leading = true;
        let mut unresolved_op_qu: Option<WordInfo> = None;
        let mut unresolved_op_qu_word_count = 0;
        let mut real_width = 0;
        let mut ideal_width = 0;
        let mut should_take_new_one = false;

        loop {
            let word = word_iter.peek()?.clone();
            real_width += word.real_width;
            ideal_width += word.ideal_width;

            if break_all {
                word_iter.next();

                let word_next = word_iter.peek();
                if let Some(word_next) = word_next {
                    if word_next.position.brk != usize::MAX {
                        end = word_next.position.end;
                        brk = word_next.position.brk;
                        real_width += word_next.real_width;
                        ideal_width += word_next.ideal_width;
                        break;
                    }
                } else {
                    end = word.position.end;
                    brk = if word.position.brk != usize::MAX {
                        word.position.brk
                    } else {
                        word.position.end
                    };
                    break;
                }

                continue;
            }

            if is_line_leading
                && self.long_break == true
                && word.position.brk != usize::MAX
                && !(word.word_type == WordType::RETURN || word.word_type == WordType::NEWLINE)
            {
                end = word.position.end;
                brk = word.position.brk;
                should_take_new_one = true;
                break;
            }

            if word.word_type == WordType::NEWLINE || word.word_type == WordType::RETURN {
                end = word.position.end;
                brk = word.position.end;
                should_take_new_one = true;
                break;
            }

            if word.word_type == WordType::OPEN_PUNCTUATION || word.word_type == WordType::QUOTATION
            {
                let mut qu_processed = false;

                if unresolved_op_qu.is_none()
                    || (word.word_type == WordType::OPEN_PUNCTUATION
                        && unresolved_op_qu_word_count > 0)
                {
                    unresolved_op_qu = Some(word.clone());
                    unresolved_op_qu_word_count = 0;
                    qu_processed = true;
                }

                word_iter.advance_cursor();
                if let Some(word_next) = word_iter.peek() {
                    if word_next.position.brk != usize::MAX
                        && word_next.position.brk != word_next.position.end
                    {
                        if is_line_leading {
                            continue;
                        }

                        if unresolved_op_qu.is_some() && unresolved_op_qu_word_count == 0 {
                            let qu = unresolved_op_qu.unwrap();
                            end = qu.position.start;
                            brk = qu.position.start;
                        } else {
                            end = word.position.start;
                            brk = word.position.start;

                            real_width -= word.real_width;
                            ideal_width -= word.ideal_width;
                        }
                        break;
                    }
                }

                if !qu_processed
                    && word.word_type == WordType::QUOTATION
                    && unresolved_op_qu.is_some()
                {
                    unresolved_op_qu.take();
                    unresolved_op_qu_word_count = 0;
                }
            }

            word_iter.next();

            let word_next = word_iter.peek();
            if let Some(word_next) = word_next {
                if word_next.position.brk != usize::MAX {
                    end = word.position.end;
                    brk = word.position.end;

                    if word_next.position.brk == word_next.position.end {
                        if word_next.word_type == WordType::CJK
                            || word_next.word_type == WordType::LATIN
                            || word_next.word_type == WordType::NUMBER
                        {
                            continue;
                        } else if word_next.word_type == WordType::SPACE
                            || word_next.word_type == WordType::CLOSE_PUNCTUATION
                            || word_next.word_type == WordType::QUOTATION
                            || word_next.word_type == WordType::HYPHEN
                        {
                            if word.word_type == WordType::QUOTATION {
                                if unresolved_op_qu.is_some() {
                                    end = word.position.start;
                                    brk = word.position.start;
                                } else {
                                    end = word.position.end;
                                    brk = word.position.end;
                                }
                            } else {
                                end = word_next.position.end;
                                brk = word_next.position.brk;
                                real_width += word_next.real_width;
                                ideal_width += word_next.ideal_width;
                            }
                            break;
                        }
                    }

                    if word_next.word_type == WordType::RETURN
                        || word_next.word_type == WordType::NEWLINE
                    {
                        brk += 1;
                    } else if !(word.word_type == WordType::CLOSE_PUNCTUATION
                        || word.word_type == WordType::QUOTATION)
                        && (word_next.word_type == WordType::CLOSE_PUNCTUATION
                            || word_next.word_type == WordType::QUOTATION
                            || word_next.word_type == WordType::HYPHEN)
                    {
                        if is_line_leading {
                            end = word_next.position.end;
                            brk = word_next.position.brk;
                        } else {
                            if unresolved_op_qu.is_some() && unresolved_op_qu_word_count == 0 {
                                let op_qu = unresolved_op_qu.unwrap();
                                end = op_qu.position.start;
                                brk = op_qu.position.start;
                            } else {
                                end = word.position.start;
                                brk = word.position.start;
                            }
                        }

                        if is_line_leading == false {
                            real_width -= word.real_width;
                            ideal_width -= word.ideal_width;
                        } else {
                            real_width += word_next.real_width;
                            ideal_width += word_next.ideal_width;
                        }
                    }
                    break;
                } else if word.word_type == WordType::CJK
                    || word.word_type == WordType::LATIN
                    || word.word_type == WordType::NUMBER
                {
                    if unresolved_op_qu.is_some() {
                        unresolved_op_qu_word_count += 1;
                    }
                }
            } else {
                end = word.position.end;
                brk = word.position.end;
                break;
            }

            if unresolved_op_qu.is_none() || unresolved_op_qu_word_count > 0 {
                is_line_leading = false;
            }
        }

        if should_take_new_one {
            word_iter.next();
        }

        if end == brk {
            if let Some(word_next) = word_iter.peek() {
                if word_next.word_type == WordType::SPACE {
                    let space_len = word_next.position.end - word_next.position.start;
                    brk += space_len;
                }
            }
        }

        line_info.position.end = line_info.position.start + end;
        line_info.position.brk = line_info.position.start + brk;
        line_info.real_width = real_width;
        line_info.ideal_width = ideal_width;
        self.line_info_prev = Some(line_info.clone());
        Some(line_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! do_a_test {
        ($text:expr, $n: expr) => {
            do_a_test($text, $n, FLAG_BREAK_NONE);
        };

        ($text:expr, $n: expr, $flags:expr) => {
            do_a_test($text, $n, $flags);
        };
    }

    fn do_a_test(text: &str, n: usize, flags: Flags) {
        let flow = Line::new(text, n, 4)
            .with_long_break(true)
            .with_flags(flags);

        for line in flow {
            let mut display_buffer = text
                [line.position.start..line.position.brk.min(line.position.end)]
                .trim_end_matches("\n")
                .to_owned();
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
        do_a_test!("The quick brown fox jumps over a lazy dog.", 15);
    }

    #[test]
    fn test_line_2() {
        do_a_test!("八百标兵奔北坡炮兵并排北边跑666中英文测试。The quick brown fox jumps over a lazy dog. abcdefghijklmnopq rstuvwxyz", 14);
    }

    #[test]
    fn test_line_3() {
        do_a_test!(
            "为了提供更好的服务和服务。\n请您在使用前充分阅读《TextFlowwwwwwwwwwwwwwwwww 使用隐私 Policy》",
            25
        );
    }

    #[test]
    fn test_line_4() {
        do_a_test!("《Loooooooooooooooong Text》", 20);
    }

    #[test]
    fn test_line_5() {
        do_a_test!("This is a Text》〉>?!", 20);
        do_a_test!("<〈《Teext a>>>", 12);
        do_a_test!("<〈《Tee<ext><>>", 12);
        do_a_test!("<〈《Tee<eext><>>", 12);
        do_a_test!("<〈<<《你》>", 10);
        do_a_test!("<〈<<《Loooooo｜ong>>", 14);
        do_a_test!("this is aaaa \"text word\" test", 15);
        do_a_test!("this is a \"text word\" test", 15);
        do_a_test!("this is a <text> test", 15);
        do_a_test!("this is a text-test", 15);
        do_a_test!("实时操作系统 Nuttx》。", 20);
    }

    #[test]
    fn test_line_6() {
        for i in 1..=15 {
            do_a_test!("an \"apple\" tree", i);
        }
    }

    #[test]
    fn test_line_7() {
        do_a_test!("abc,    bcd, efg  bc", 5);
        do_a_test!("abc,\n    bcd, efg  bc", 5);
        do_a_test!("an    apple         \"is\" a fruit", 1);
        do_a_test!("anyone can be able to", 13);
    }

    #[test]
    fn test_line_8() {
        do_a_test!("a book named 《<《「Wow》>」", 27);
    }

    #[test]
    fn test_line_9() {
        do_a_test!("b  \n\n     a", 2);
    }

    #[test]
    fn test_line_10() {
        do_a_test!("\"abc     aa", 2);
    }

    #[test]
    fn test_line_11() {
        do_a_test!("f abcdefghijklmnopq", 10, FLAG_BREAK_ALL);
    }
}
