use crate::line::{Line, LineInfo};

mod line;
mod word;

#[allow(dead_code)]
struct TextFlowContext {}

#[allow(dead_code)]
pub struct TextFlow<'a> {
    text: &'a str,
    max_width: usize,
    line_height: usize,
    line_spacing: usize,
    word_spacing: usize,
    tab_width: usize,

    context: TextFlowContext,

    lines: Line<'a>,
}

impl TextFlow<'_> {
    pub fn new(text: &str, max_width: usize) -> TextFlow {
        let mut flow = TextFlow {
            text,
            max_width,
            line_height: 0,
            line_spacing: 0,
            word_spacing: 0,
            tab_width: 0,
            context: TextFlowContext {},
            lines: Line::new("", 0, 0),
        };

        flow.lines = Line::new(flow.text, flow.max_width, flow.tab_width).with_long_break(true);

        flow
    }
}

impl Iterator for TextFlow<'_> {
    type Item = LineInfo;

    fn next(&mut self) -> Option<Self::Item> {
        self.lines.next()
    }
}
