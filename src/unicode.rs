use core::iter::Peekable;
use core::ops::Range;
use core::str::CharIndices;

use crate::properties::{is_close_punctuation, is_open_punctuation, is_wide};

#[cfg(feature = "unicode")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Script {
    Common,
    Inherited,
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Arabic,
    Devanagari,
    Han,
    Hiragana,
    Katakana,
    Hangul,
    Unknown,
}

#[cfg(feature = "unicode")]
impl Script {
    pub fn of(character: char) -> Self {
        script(character)
    }

    pub const fn iso15924_tag(self) -> [u8; 4] {
        match self {
            Self::Common => *b"Zyyy",
            Self::Inherited => *b"Zinh",
            Self::Latin => *b"Latn",
            Self::Greek => *b"Grek",
            Self::Cyrillic => *b"Cyrl",
            Self::Hebrew => *b"Hebr",
            Self::Arabic => *b"Arab",
            Self::Devanagari => *b"Deva",
            Self::Han => *b"Hani",
            Self::Hiragana => *b"Hira",
            Self::Katakana => *b"Kana",
            Self::Hangul => *b"Hang",
            Self::Unknown => *b"Zzzz",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grapheme<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
}

pub struct Graphemes<'a> {
    text: &'a str,
    cursor: usize,
}

pub fn graphemes(text: &str) -> Graphemes<'_> {
    Graphemes { text, cursor: 0 }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = Grapheme<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor == self.text.len() {
            return None;
        }
        let start = self.cursor;
        let mut chars = self.text[start..].char_indices();
        let (_, first) = chars.next()?;
        let mut previous = grapheme_class(first);
        let mut regional_count = usize::from(previous == GraphemeClass::Regional);
        let mut end = start + first.len_utf8();

        for (offset, character) in chars {
            let next = grapheme_class(character);
            if grapheme_break(previous, next, regional_count) {
                break;
            }
            end = start + offset + character.len_utf8();
            if next == GraphemeClass::Regional {
                regional_count += 1;
            } else if !matches!(next, GraphemeClass::Extend | GraphemeClass::Zwj) {
                regional_count = 0;
            }
            previous = next;
        }

        self.cursor = end;
        Some(Grapheme {
            text: &self.text[start..end],
            range: start..end,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineBreakKind {
    Allowed,
    Mandatory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineBreak {
    pub offset: usize,
    pub kind: LineBreakKind,
}

pub struct LineBreaks<'a> {
    text: &'a str,
    chars: Peekable<CharIndices<'a>>,
    emitted_end: bool,
}

pub fn line_breaks(text: &str) -> LineBreaks<'_> {
    LineBreaks {
        text,
        chars: text.char_indices().peekable(),
        emitted_end: false,
    }
}

impl Iterator for LineBreaks<'_> {
    type Item = LineBreak;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((start, character)) = self.chars.next() {
            let offset = start + character.len_utf8();
            let next = self.chars.peek().map(|(_, next)| *next);
            let kind = if character == '\n' || character == '\r' && next != Some('\n') {
                Some(LineBreakKind::Mandatory)
            } else if is_space(character) {
                (!next.is_some_and(is_space)).then_some(LineBreakKind::Allowed)
            } else if matches!(character, '-' | '/' | '\u{2010}' | '\u{2013}')
                || next.is_some_and(|next| allows_wide_break(character, next))
            {
                Some(LineBreakKind::Allowed)
            } else {
                None
            };
            if let Some(kind) = kind {
                if offset == self.text.len() {
                    self.emitted_end = true;
                    return Some(LineBreak {
                        offset,
                        kind: LineBreakKind::Mandatory,
                    });
                }
                return Some(LineBreak { offset, kind });
            }
        }
        if self.emitted_end {
            None
        } else {
            self.emitted_end = true;
            Some(LineBreak {
                offset: self.text.len(),
                kind: LineBreakKind::Mandatory,
            })
        }
    }
}

#[cfg(feature = "unicode")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptRun {
    pub text: Range<usize>,
    pub script: Script,
}

#[cfg(feature = "unicode")]
pub fn script_runs(text: &str) -> ScriptRuns<'_> {
    ScriptRuns {
        text,
        graphemes: graphemes(text),
        pending: None,
    }
}

#[cfg(feature = "unicode")]
pub struct ScriptRuns<'a> {
    text: &'a str,
    graphemes: Graphemes<'a>,
    pending: Option<Grapheme<'a>>,
}

#[cfg(feature = "unicode")]
impl Iterator for ScriptRuns<'_> {
    type Item = ScriptRun;

    fn next(&mut self) -> Option<Self::Item> {
        let first = self.pending.take().or_else(|| self.graphemes.next())?;
        let start = first.range.start;
        let mut run_script = strong_script(first.text);

        for cluster in self.graphemes.by_ref() {
            let next_script = strong_script(cluster.text);
            if run_script.is_none() {
                run_script = next_script;
            } else if next_script.is_some() && next_script != run_script {
                self.pending = Some(cluster);
                break;
            }
        }

        let end = self
            .pending
            .as_ref()
            .map_or(self.text.len(), |cluster| cluster.range.start);
        Some(ScriptRun {
            text: start..end,
            script: run_script.unwrap_or(Script::Common),
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GraphemeClass {
    Other,
    Cr,
    Lf,
    Control,
    Extend,
    Zwj,
    Regional,
    Prepend,
    SpacingMark,
    L,
    V,
    T,
    Lv,
    Lvt,
    ExtendedPictographic,
}

fn grapheme_break(previous: GraphemeClass, next: GraphemeClass, regional_count: usize) -> bool {
    if previous == GraphemeClass::Cr && next == GraphemeClass::Lf {
        return false;
    }
    if matches!(
        previous,
        GraphemeClass::Cr | GraphemeClass::Lf | GraphemeClass::Control
    ) || matches!(
        next,
        GraphemeClass::Cr | GraphemeClass::Lf | GraphemeClass::Control
    ) {
        return true;
    }
    if matches!(
        (previous, next),
        (
            GraphemeClass::L,
            GraphemeClass::L | GraphemeClass::V | GraphemeClass::Lv | GraphemeClass::Lvt
        ) | (
            GraphemeClass::Lv | GraphemeClass::V,
            GraphemeClass::V | GraphemeClass::T
        ) | (GraphemeClass::Lvt | GraphemeClass::T, GraphemeClass::T)
    ) {
        return false;
    }
    if matches!(
        next,
        GraphemeClass::Extend | GraphemeClass::Zwj | GraphemeClass::SpacingMark
    ) || previous == GraphemeClass::Prepend
        || previous == GraphemeClass::Zwj && next == GraphemeClass::ExtendedPictographic
    {
        return false;
    }
    !(previous == GraphemeClass::Regional
        && next == GraphemeClass::Regional
        && regional_count % 2 == 1)
}

fn grapheme_class(character: char) -> GraphemeClass {
    let value = character as u32;
    match value {
        0x000D => GraphemeClass::Cr,
        0x000A => GraphemeClass::Lf,
        0x0000..=0x001F | 0x007F..=0x009F => GraphemeClass::Control,
        0x200D => GraphemeClass::Zwj,
        0x1F1E6..=0x1F1FF => GraphemeClass::Regional,
        0x0600..=0x0605 | 0x06DD | 0x070F | 0x08E2 => GraphemeClass::Prepend,
        0x0903 | 0x093B..=0x0940 | 0x0949..=0x094C | 0x0982..=0x0983 => GraphemeClass::SpacingMark,
        0x1100..=0x115F | 0xA960..=0xA97C => GraphemeClass::L,
        0x1160..=0x11A7 | 0xD7B0..=0xD7C6 => GraphemeClass::V,
        0x11A8..=0x11FF | 0xD7CB..=0xD7FB => GraphemeClass::T,
        0xAC00..=0xD7A3 if (value - 0xAC00) % 28 == 0 => GraphemeClass::Lv,
        0xAC00..=0xD7A3 => GraphemeClass::Lvt,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF => GraphemeClass::ExtendedPictographic,
        _ if is_extend(value) => GraphemeClass::Extend,
        _ => GraphemeClass::Other,
    }
}

fn is_extend(value: u32) -> bool {
    matches!(
        value,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06ED
            | 0x0900..=0x0902
            | 0x093A
            | 0x093C
            | 0x0941..=0x0948
            | 0x094D
            | 0x0951..=0x0957
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x1F3FB..=0x1F3FF
            | 0xE0100..=0xE01EF
    )
}

#[cfg(feature = "unicode")]
fn script(character: char) -> Script {
    let value = character as u32;
    match value {
        _ if is_extend(value) => Script::Inherited,
        0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0370..=0x03FF | 0x1F00..=0x1FFF => Script::Greek,
        0x0400..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Script::Cyrillic,
        0x0590..=0x05FF => Script::Hebrew,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            Script::Arabic
        }
        0x0900..=0x097F | 0xA8E0..=0xA8FF => Script::Devanagari,
        0x3040..=0x309F => Script::Hiragana,
        0x30A0..=0x30FF | 0x31F0..=0x31FF => Script::Katakana,
        0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => Script::Hangul,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF => Script::Han,
        0x0000..=0x0040
        | 0x005B..=0x0060
        | 0x007B..=0x00BF
        | 0x2000..=0x206F
        | 0x20A0..=0x20CF
        | 0x2100..=0x214F
        | 0x2190..=0x2BFF
        | 0x1F000..=0x1FAFF => Script::Common,
        _ => Script::Unknown,
    }
}

#[cfg(feature = "unicode")]
fn strong_script(grapheme: &str) -> Option<Script> {
    grapheme
        .chars()
        .map(script)
        .find(|script| !matches!(script, Script::Common | Script::Inherited | Script::Unknown))
}

fn is_space(character: char) -> bool {
    matches!(
        character,
        ' ' | '\t' | '\u{00A0}' | '\u{2000}'..='\u{200A}' | '\u{3000}'
    )
}

fn allows_wide_break(previous: char, next: char) -> bool {
    !is_open_punctuation(previous)
        && !is_close_punctuation(next)
        && (is_wide(previous) || is_wide(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    #[test]
    fn grapheme_ranges_preserve_common_clusters() {
        let text = "a\u{301}👩‍🚀🇨🇳";
        let clusters = graphemes(text).collect::<Vec<_>>();

        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[0].text, "a\u{301}");
        assert_eq!(clusters[1].text, "👩‍🚀");
        assert_eq!(clusters[2].text, "🇨🇳");
    }

    #[test]
    fn line_breaks_preserve_words_and_cjk_punctuation() {
        let latin = line_breaks("hello world").collect::<Vec<_>>();
        let cjk = line_breaks("你好，世界").collect::<Vec<_>>();

        assert!(latin.contains(&LineBreak {
            offset: 6,
            kind: LineBreakKind::Allowed,
        }));
        assert!(!cjk
            .iter()
            .any(|line_break| line_break.offset == "你好".len()));
        assert_eq!(cjk.last().unwrap().kind, LineBreakKind::Mandatory);
    }

    #[test]
    fn line_breaks_keep_bracket_edges_together() {
        let text = "《文字》〉";
        let offsets = line_breaks(text)
            .filter(|line_break| line_break.kind == LineBreakKind::Allowed)
            .map(|line_break| line_break.offset)
            .collect::<Vec<_>>();

        assert_eq!(offsets, ["《文".len()]);
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn neutral_prefix_joins_the_first_strong_script() {
        let text = "(العربية) Latin";
        let runs = script_runs(text).collect::<Vec<_>>();

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].script, Script::Arabic);
        assert_eq!(&text[runs[0].text.clone()], "(العربية) ");
        assert_eq!(runs[1].script, Script::Latin);
    }
}
