use std::iter::Peekable;
use std::str::CharIndices;

#[derive(PartialEq, Debug, Clone)]
pub enum WordType {
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

#[derive(Debug, Clone, PartialEq)]
pub struct WordPosition {
    pub start: usize,
    pub end: usize,
    pub brk: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WordInfo {
    pub position: WordPosition,
    pub word_type: WordType,
    pub real_width: usize,
    pub ideal_width: usize,
}

pub struct Word<'a> {
    char_indices: Peekable<CharIndices<'a>>,

    word_info_prev: Option<WordInfo>,
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
    if is_latin(ch) {
        return WordType::LATIN;
    } else if is_cjk(ch) {
        return WordType::CJK;
    } else if ch == '-' {
        return WordType::HYPHEN;
    } else if ch >= '0' && ch <= '9' {
        return WordType::NUMBER;
    } else if is_punctuation(ch) {
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

fn get_char_width(ch: char, tab_width: usize) -> usize {
    let char_type = get_word_type(ch);
    return match char_type {
        WordType::LATIN => 1,
        WordType::CJK => 2,
        WordType::HYPHEN => 1,
        WordType::NUMBER => 1,
        WordType::PUNCTUATION => 1,
        WordType::RETURN => 0,
        WordType::NEWLINE => 0,
        WordType::SPACE => 1,
        WordType::TAB => tab_width,
        WordType::UNKNOWN => 0,
    };
}

impl Word<'_> {
    pub fn new(text: &str) -> Word {
        Word {
            char_indices: text.char_indices().peekable(),
            word_info_prev: None,
        }
    }
}

impl Iterator for Word<'_> {
    type Item = WordInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let start = if let Some(prev) = &self.word_info_prev {
            prev.position.end
        } else {
            0
        };

        let mut word_pos_end = start;
        let mut word_type = WordType::UNKNOWN;

        // let mut word_pos_end = (&self.word_info_prev?).position.end;
        loop {
            let (end, ch) = self.char_indices.next()?;
            let word_len = ch.len_utf8();
            word_type = get_word_type(ch);

            let ch_next = self.char_indices.by_ref().peek();
            let word_type_next = if let Some(ch_next) = ch_next {
                get_word_type(ch_next.1)
            } else {
                WordType::UNKNOWN
            };
            word_pos_end += word_len;
            match word_type {
                WordType::LATIN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        continue;
                    } else {
                        break;
                    }
                }
                WordType::CJK => {
                    break;
                }
                WordType::HYPHEN => {
                    if word_type_next == WordType::LATIN || word_type_next == WordType::NUMBER {
                        break;
                    } else {
                        continue;
                    }
                }
                WordType::NUMBER => {
                    if word_type_next == WordType::NUMBER {
                        continue;
                    } else {
                        break;
                    }
                }
                WordType::PUNCTUATION => {
                    break;
                }
                WordType::RETURN => {
                    break;
                }
                WordType::NEWLINE => {
                    break;
                }
                WordType::SPACE => {
                    break;
                }
                WordType::TAB => {
                    break;
                }
                WordType::UNKNOWN => {
                    break;
                }
            }
        }

        // word_pos_end += 1;
        let info = WordInfo {
            position: WordPosition {
                start,
                end: word_pos_end,
                brk: 0,
            },
            word_type,
            real_width: 0,
            ideal_width: 0,
        };
        self.word_info_prev = Some(info.clone());
        return Some(info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        let text = "Hello, world!".to_string();
        let mut flow = Word::new(&text);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 5);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::PUNCTUATION);
        assert_eq!(word.position.end, 6);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.position.end, 7);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 12);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::PUNCTUATION);
        assert_eq!(word.position.end, 13);

        assert_eq!(flow.next(), None);
    }

    #[test]
    fn test_2() {
        let text = "".to_string();
        let mut flow = Word::new(&text);

        let word = flow.next();
        assert_eq!(word, None);
    }

    #[test]
    fn test_3() {
        let text = "H".to_string();
        let mut flow = Word::new(&text);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 1);
    }

    #[test]
    fn test_4() {
        let text = "你好\n世界".to_string();

        let mut flow = Word::new(&text);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 3);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 6);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::NEWLINE);
        assert_eq!(word.position.end, 7);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 10);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 13);

        assert_eq!(flow.next(), None);
    }
}
