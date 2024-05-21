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

struct TextFlow {
    text: String,
    max_width: usize,
    line_height: usize,
    line_spacing: usize,
    word_spacing: usize,
    tab_width: usize,
    lines: Vec<LineInfo>,
}

impl TextFlow {
    fn new(text: String, max_width: usize) -> TextFlow {
        TextFlow {
            text,
            max_width,
            line_height: 0,
            line_spacing: 0,
            word_spacing: 0,
            tab_width: 0,
            lines: Vec::new(),
        }
    }

    fn is_latin(&self, ch: char) -> bool {
        if ch >= 'a' && ch <= 'z' || ch >= 'A' && ch <= 'Z' {
            return true;
        }
        return false;
    }

    fn is_cjk(&self, ch: char) -> bool {
        if ch >= '\u{4e00}' && ch <= '\u{9fff}' {
            return true;
        }
        return false;
    }

    fn is_punctuation(&self, ch: char) -> bool {
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

    fn get_word_type(&self, ch: char) -> WordType {
        if self.is_latin(ch) {
            return WordType::LATIN;
        } else if self.is_cjk(ch) {
            return WordType::CJK;
        } else if ch == '-' {
            return WordType::HYPHEN;
        } else if ch >= '0' && ch <= '9' {
            return WordType::NUMBER;
        } else if self.is_punctuation(ch) {
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
        let char_type = self.get_word_type(ch);
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

    fn get_next_word(&self, start: usize) -> WordInfo {
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
        let mut word_type = self.get_word_type(self.text[start..].chars().next().unwrap());
        let mut word_pos_end = start;
        for (end, ch) in self.text[start + 1..].chars().enumerate() {
            let word_type_next = self.get_word_type(ch);
            match word_type {
                WordType::LATIN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        continue;
                    } else {
                        word_pos_end = start + end;
                        break;
                    }
                }
                WordType::CJK => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::HYPHEN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        word_pos_end = start + end;
                        break;
                    } else {
                        continue;
                    }
                }
                WordType::NUMBER => {
                    if word_type_next == WordType::NUMBER {
                        word_pos_end = start + end;
                        break;
                    } else {
                        continue;
                    }
                }
                WordType::PUNCTUATION => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::RETURN => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::NEWLINE => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::SPACE => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::TAB => {
                    word_pos_end = start + end;
                    break;
                }
                WordType::UNKNOWN => {
                    word_pos_end = start + end;
                    break;
                }
            }

            word_type = word_type_next;
        }

        return WordInfo {
            word: Position {
                start,
                end: word_pos_end + 1,
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
    fn it_works() {
        let text = "Hello, world!";
        let flow = TextFlow::new(text.to_string(), 10);

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

        println!("{:?}", word);
    }
}
