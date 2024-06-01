use std::iter::Peekable;
use std::ops::Not;
use std::str::CharIndices;

// Define an enum for Line Break Classes as specified in UAX #14 with detailed comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineBreakClass {
    // BK: Mandatory Break
    MandatoryBreak,
    // CR: Carriage Return
    CarriageReturn,
    // LF: Line Feed
    LineFeed,
    // NL: Next Line
    NextLine,
    // CM: Combining Mark
    CombiningMark,
    // WJ: Word Joiner
    WordJoiner,
    // ZW: Zero Width Space
    ZeroWidthSpace,
    // GL: Non-breaking ("Glue")
    NonBreaking,
    // SP: Space
    Space,
    // ZWJ: Zero Width Joiner
    ZeroWidthJoiner,
    // B2: Break Opportunity Before and After
    BreakOpportunityBeforeAndAfter,
    // BA: Break After
    BreakAfter,
    // BB: Break Before
    BreakBefore,
    // CB: Contingent Break Opportunity
    ContingentBreakOpportunity,
    // CL: Close Punctuation
    ClosePunctuation,
    // CP: Close Parenthesis
    CloseParenthesis,
    // EX: Exclamation/Interrogation
    ExclamationInterrogation,
    // IN: Inseparable
    Inseparable,
    // HY: Hyphen
    Hyphen,
    // IS: Infix Numeric Separator
    InfixNumericSeparator,
    // NU: Numeric
    Numeric,
    // PO: Postfix Numeric
    PostfixNumeric,
    // PR: Prefix Numeric
    PrefixNumeric,
    // QU: Quotation
    Quotation,
    // RI: Regional Indicator
    RegionalIndicator,
    // SA: Complex Context Dependent (South East Asian)
    ComplexContextDependent,
    // AI: Ambiguous (Alphabetic or Ideographic)
    Ambiguous,
    // AL: Alphabetic
    Alphabetic,
    // CJ: Conditional Japanese Starter
    ConditionalJapaneseStarter,
    // EB: Emoji Base
    EmojiBase,
    // EM: Emoji Modifier
    EmojiModifier,
    // H2: Hangul LV Syllable
    HangulLVSyllable,
    // H3: Hangul LVT Syllable
    HangulLVTSyllable,
    // HL: Hebrew Letter
    HebrewLetter,
    // Unknown
    Unknown,
}

impl From<char> for LineBreakClass {
    fn from(value: char) -> Self {
        match value {
            '\u{000B}' => LineBreakClass::MandatoryBreak, // BK: Mandatory Break
            '\r' => LineBreakClass::CarriageReturn,       // CR: Carriage Return
            '\n' => LineBreakClass::LineFeed,             // LF: Line Feed
            '\u{0085}' => LineBreakClass::NextLine,       // NL: Next Line
            '\u{0300}'..='\u{036F}' => LineBreakClass::CombiningMark, // CM: Combining Mark
            '\u{2060}' => LineBreakClass::WordJoiner,     // WJ: Word Joiner
            '\u{200B}' => LineBreakClass::ZeroWidthSpace, // ZW: Zero Width Space
            '\u{00A0}' => LineBreakClass::NonBreaking,    // GL: Non-breaking ("Glue")
            ' ' => LineBreakClass::Space,                 // SP: Space
            '\u{200D}' => LineBreakClass::ZeroWidthJoiner, // ZWJ: Zero Width Joiner
            '\t' | '\x0C' => LineBreakClass::BreakOpportunityBeforeAndAfter, // B2: Break Opportunity Before and After
            '\u{2029}' => LineBreakClass::BreakAfter,                        // BA: Break After
            '\u{2028}' => LineBreakClass::BreakBefore,                       // BB: Break Before
            ')' | ']' | '}' | '\u{00BB}' | '\u{2019}' => LineBreakClass::ClosePunctuation, // CL: Close Punctuation
            '\u{0021}' | '\u{003F}' => LineBreakClass::ExclamationInterrogation, // EX: Exclamation/Interrogation
            c if c.is_alphabetic() => LineBreakClass::Alphabetic,                // AL: Alphabetic
            '-' => LineBreakClass::Hyphen,                                       // HY: Hyphen
            ',' | '.' => LineBreakClass::InfixNumericSeparator, // IS: Infix Numeric Separator
            c if c.is_numeric() => LineBreakClass::Numeric,     // NU: Numeric
            '%' | '\u{2030}' | '\u{2031}' => LineBreakClass::PostfixNumeric, // PO: Postfix Numeric
            '$' | '\u{00A2}'..='\u{00A5}' | '\u{20A0}'..='\u{20B5}' => {
                LineBreakClass::PrefixNumeric
            } // PR: Prefix Numeric
            '"' | '\'' | '\u{00AB}' | '\u{00BB}' | '\u{2018}'..='\u{201F}' => {
                LineBreakClass::Quotation
            } // QU: Quotation
            '\u{1F1E6}'..='\u{1F1FF}' => LineBreakClass::RegionalIndicator, // RI: Regional Indicator
            '\u{0E33}'..='\u{0E3A}' | '\u{0E40}'..='\u{0E44}' => {
                LineBreakClass::ComplexContextDependent
            } // SA: Complex Context Dependent (South East Asian)
            _ => LineBreakClass::Unknown,
        }
    }
}

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

    remaining_width: usize,
    tab_width: usize,
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
        || ch == '。'
        || ch == '，'
        || ch == '、'
        || ch == '？'
        || ch == '！'
        || ch == '：'
        || ch == '；'
        || ch == '（'
        || ch == '）'
        || ch == '「'
        || ch == '」'
        || ch == '『'
        || ch == '』'
        || ch == '【'
        || ch == '】'
        || ch == '〔'
        || ch == '〕'
        || ch == '〈'
        || ch == '〉'
        || ch == '《'
        || ch == '》'
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
        WordType::PUNCTUATION => ch.is_ascii().not() as usize + 1,
        WordType::RETURN => 0,
        WordType::NEWLINE => 0,
        WordType::SPACE => 1,
        WordType::TAB => tab_width,
        WordType::UNKNOWN => 0,
    };
}

#[allow(unused)]
impl Word<'_> {
    pub fn new(text: &str, remaining_width: usize, tab_width: usize) -> Word {
        Word {
            char_indices: text.char_indices().peekable(),
            word_info_prev: None,
            remaining_width,
            tab_width,
        }
    }

    pub fn set_remaining_width(&mut self, remaining_width: usize) {
        self.remaining_width = remaining_width;
    }

    pub fn get_remaining_width(&self) -> usize {
        return self.remaining_width;
    }
}

impl Iterator for Word<'_> {
    type Item = WordInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let word_info_prev_ref = self.word_info_prev.as_ref();
        let start = word_info_prev_ref.map_or(0, |v| v.position.end);

        let mut word_pos_end = start;
        let mut word_type = WordType::UNKNOWN;
        let mut word_width = 0;
        let mut brk_pos = word_info_prev_ref.map_or(usize::MAX, |v| v.position.brk);
        let mut real_width = 0;

        loop {
            let ch = self.char_indices.by_ref().peek()?.1;
            let char_len = ch.len_utf8();
            let char_width = get_char_width(ch, self.tab_width);

            if word_type == WordType::UNKNOWN {
                word_type = get_word_type(ch);
            }

            self.char_indices.next();

            let char_next = self.char_indices.by_ref().peek().map_or(0 as char, |v| v.1);
            let char_width_next = get_char_width(char_next, self.tab_width);
            let word_type_next = get_word_type(char_next);

            word_pos_end += char_len;
            word_width += char_width;

            if word_width + char_width_next > self.remaining_width {
                if brk_pos == usize::MAX {
                    brk_pos = word_pos_end;
                    real_width = word_width;
                }
            }

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
                    brk_pos = word_pos_end - 1;
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

        if real_width == 0 {
            real_width = word_width;
        }

        if self.remaining_width >= real_width {
            self.remaining_width -= real_width;
        }

        let info = WordInfo {
            position: WordPosition {
                start,
                end: word_pos_end,
                brk: brk_pos,
            },
            word_type,
            real_width,
            ideal_width: word_width,
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
        let mut flow = Word::new(&text, 10, 4);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 5);
        assert_eq!(&text[word.position.start..word.position.end], "Hello");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::PUNCTUATION);
        assert_eq!(word.position.end, 6);
        assert_eq!(&text[word.position.start..word.position.end], ",");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.position.end, 7);
        assert_eq!(&text[word.position.start..word.position.end], " ");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 12);
        assert_eq!(&text[word.position.start..word.position.end], "world");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::PUNCTUATION);
        assert_eq!(word.position.end, 13);
        assert_eq!(word.position.brk, 10);
        assert_eq!(&text[word.position.start..word.position.end], "!");

        assert_eq!(flow.next(), None);
    }

    #[test]
    fn test_2() {
        let text = "".to_string();
        let mut flow = Word::new(&text, 100, 4);

        let word = flow.next();
        assert_eq!(word, None);
    }

    #[test]
    fn test_3() {
        let text = "H".to_string();
        let mut flow = Word::new(&text, 100, 4);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 1);
        assert_eq!(&text[word.position.start..word.position.end], "H");
    }

    #[test]
    fn test_4() {
        let text = "你好\n世界 Hello123 456 ".to_string();

        let mut flow = Word::new(&text, 100, 4);

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 3);
        assert_eq!(&text[word.position.start..word.position.end], "你");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 6);
        assert_eq!(&text[word.position.start..word.position.end], "好");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::NEWLINE);
        assert_eq!(word.position.end, 7);
        assert_eq!(&text[word.position.start..word.position.end], "\n");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 10);
        assert_eq!(&text[word.position.start..word.position.end], "世");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::CJK);
        assert_eq!(word.position.end, 13);
        assert_eq!(&text[word.position.start..word.position.end], "界");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.position.end, 14);
        assert_eq!(&text[word.position.start..word.position.end], " ");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::LATIN);
        assert_eq!(word.position.end, 22);
        assert_eq!(&text[word.position.start..word.position.end], "Hello123");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.position.end, 23);
        assert_eq!(&text[word.position.start..word.position.end], " ");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::NUMBER);
        assert_eq!(word.position.end, 26);
        assert_eq!(&text[word.position.start..word.position.end], "456");

        let word = flow.next().unwrap();
        assert_eq!(word.word_type, WordType::SPACE);
        assert_eq!(word.position.end, 27);
        assert_eq!(&text[word.position.start..word.position.end], " ");

        assert_eq!(flow.next(), None);
    }
}
