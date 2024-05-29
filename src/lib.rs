use std::str::CharIndices;

use crate::word::{WordInfo, WordPosition};

mod word;
mod line;

struct LineInfo {
    line: WordPosition,
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
}
