use std::str::CharIndices;

#[derive(PartialEq, Debug)]
enum WordType {
    LATIN,
    CJK,
    HYPHEN,
    NUMBER,
    PUNCTUATION,
    RETURN,
    NEWLINE,
    SPACE,
    TAB,
    UNKNOWN,
}

#[derive(Debug)]
struct Position {
    start: usize,
    end: usize,
    brk: usize,
}

#[derive(Debug)]
struct WordInfo {
    word: Position,
    word_type: WordType,
    real_width: usize,
    ideal_width: usize,
}

struct LineInfo {
    line: Position,
    words: Vec<WordInfo>,
}

struct TextFlowContext<'a> {
    char_indices: CharIndices<'a>,
}

struct TextFlow<'a> {
    text: &'a String,
    max_width: usize,
    line_height: usize,
    line_spacing: usize,
    word_spacing: usize,
    tab_width: usize,

    context: TextFlowContext<'a>,

    lines: Vec<LineInfo>,
}

impl TextFlow<'_> {
    fn new(text: &'_ String, max_width: usize) -> TextFlow {
        TextFlow {
            text,
            max_width,
            line_height: 0,
            line_spacing: 0,
            word_spacing: 0,
            tab_width: 0,
            context: TextFlowContext {
                char_indices: text.char_indices(),
            },
            lines: Vec::new(),
        }
    }

    fn is_latin(ch: char) -> bool {
        if ch >= 'a' && ch <= 'z' || ch >= 'A' && ch <= 'Z' {
            return true;
        }
        return false;
    }

    fn is_cjk(ch: char) -> bool {
        if ch >= '\u{4e00}' && ch <= '\u{9fff}' {
            return true;
        }
        return false;
    }

    fn is_punctuation(ch: char) -> bool {
        if ch == '.'
            || ch == ','
            || ch == ';'
            || ch == ':'
            || ch == '!'
            || ch == '?'
            || ch == '('
            || ch == ')'
            || ch == '['
            || ch == ']'
            || ch == '{'
            || ch == '}'
        {
            return true;
        }
        return false;
    }

    fn get_word_type(ch: char) -> WordType {
        if Self::is_latin(ch) {
            return WordType::LATIN;
        } else if Self::is_cjk(ch) {
            return WordType::CJK;
        } else if ch == '-' {
            return WordType::HYPHEN;
        } else if ch >= '0' && ch <= '9' {
            return WordType::NUMBER;
        } else if Self::is_punctuation(ch) {
            return WordType::PUNCTUATION;
        } else if ch == '\n' {
            return WordType::NEWLINE;
        } else if ch == '\r' {
            return WordType::RETURN;
        } else if ch == ' ' {
            return WordType::SPACE;
        } else if ch == '\t' {
            return WordType::TAB;
        }
        return WordType::UNKNOWN;
    }

    fn get_char_width(&self, ch: char) -> usize {
        let char_type = Self::get_word_type(ch);
        return match char_type {
            WordType::LATIN => 1,
            WordType::CJK => 2,
            WordType::HYPHEN => 1,
            WordType::NUMBER => 1,
            WordType::PUNCTUATION => 1,
            WordType::RETURN => 0,
            WordType::NEWLINE => 0,
            WordType::SPACE => 1,
            WordType::TAB => self.tab_width,
            WordType::UNKNOWN => 0,
        };
    }

    fn get_next_word(&mut self, start: usize) -> WordInfo {
        if self.text.len() == 0 {
            return WordInfo {
                word: Position {
                    start: 0,
                    end: 0,
                    brk: 0,
                },
                word_type: WordType::UNKNOWN,
                real_width: 0,
                ideal_width: 0,
            };
        }

        let mut word_type = if let Some(first_char) = self.context.char_indices.next() {
            Self::get_word_type(first_char.1)
        } else {
            return WordInfo {
                word: Position {
                    start: start,
                    end: start,
                    brk: 0,
                },
                word_type: WordType::UNKNOWN,
                real_width: 0,
                ideal_width: 0,
            };
        };

        let mut word_pos_end = start + 1;
        for (end, ch) in self.context.char_indices.by_ref() {
            let word_type_next = Self::get_word_type(ch);
            match word_type {
                WordType::LATIN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        continue;
                    } else {
                        word_pos_end = end;
                        break;
                    }
                }
                WordType::CJK => {
                    word_pos_end = end;
                    break;
                }
                WordType::HYPHEN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        word_pos_end = end;
                        break;
                    } else {
                        continue;
                    }
                }
                WordType::NUMBER => {
                    if word_type_next == WordType::NUMBER {
                        word_pos_end = end;
                        break;
                    } else {
                        continue;
                    }
                }
                WordType::PUNCTUATION => {
                    word_pos_end = end;
                    break;
                }
                WordType::RETURN => {
                    word_pos_end = end;
                    break;
                }
                WordType::NEWLINE => {
                    word_pos_end = end;
                    break;
                }
                WordType::SPACE => {
                    word_pos_end = end;
                    break;
                }
                WordType::TAB => {
                    word_pos_end = end;
                    break;
                }
                WordType::UNKNOWN => {
                    word_pos_end = end;
                    break;
                }
            }

            word_type = word_type_next;
        }

        return WordInfo {
            word: Position {
                start,
                end: word_pos_end,
                brk: 0,
            },
            word_type,
            real_width: 0,
            ideal_width: 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        let text = "Hello, world!".to_string();
        let mut flow = TextFlow::new(&text, 10);

        let word = flow.get_next_word(0);
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.word.end, 5);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::PUNCTUATION);
        assert_eq!(word.word.end, 6);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.word.end, 7);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.word.end, 12);
    }

    #[test]
    fn test_2() {
        let text = "".to_string();
        let mut flow = TextFlow::new(&text, 10);

        let word = flow.get_next_word(0);
        assert_eq!(word.word_type, WordType::UNKNOWN);
    }

    #[test]
    fn test_3() {
        let text = "H".to_string();
        let mut flow = TextFlow::new(&text, 10);

        let word = flow.get_next_word(0);
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.word.end, 1);
    }

    #[test]
    fn test_4() {
        let text = "你好\n世界".to_string();

        assert_eq!(text.chars().count(), 5);

        let mut flow = TextFlow::new(&text, 10);

        let word = flow.get_next_word(0);
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.word.end, 1);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.word.end, 2);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::NEWLINE);
        assert_eq!(word.word.end, 3);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.word.end, 4);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.word.end, 5);

        let word = flow.get_next_word(word.word.end);
        assert_eq!(word.word_type, WordType::UNKNOWN);
        assert_eq!(word.word.end, 5);
    }

    #[test]
    fn test_5() {
        let text = "你好\n世界";
        let mut indexes = text.char_indices();
        assert_eq!(indexes.next(), Some((0, '你')));
        assert_eq!(indexes.next(), Some((3, '好')));
    }
}
