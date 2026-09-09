pub(crate) fn is_wide(character: char) -> bool {
    matches!(
        character as u32,
        0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0x1F000..=0x1FAFF
            | 0x20000..=0x323AF
    )
}

pub(crate) fn is_open_punctuation(character: char) -> bool {
    matches!(
        character,
        '(' | '['
            | '{'
            | '<'
            | '（'
            | '「'
            | '『'
            | '【'
            | '〔'
            | '〈'
            | '《'
            | '⦗'
            | '⟨'
            | '‘'
            | '“'
    )
}

pub(crate) fn is_close_punctuation(character: char) -> bool {
    matches!(
        character,
        '.' | ','
            | ';'
            | ':'
            | '!'
            | '?'
            | '。'
            | '，'
            | '、'
            | '？'
            | '！'
            | '：'
            | '；'
            | ')'
            | ']'
            | '}'
            | '>'
            | '）'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〉'
            | '》'
            | '⦘'
            | '⟩'
            | '’'
            | '”'
            | '|'
            | '｜'
            | '·'
            | '/'
            | '—'
            | '～'
    )
}

pub(crate) fn display_width(text: &str, tab_width: usize) -> usize {
    if text == "\t" {
        tab_width
    } else if text
        .chars()
        .all(|character| matches!(character, '\r' | '\n'))
    {
        0
    } else if text.chars().any(is_wide) {
        2
    } else {
        1
    }
}
