//! Bounded `no_std` Unicode text layout and shaping.
//!
//! [`TextFlow`] provides borrowed line layout with explicit width, height, and spacing metrics.
//! The optional shaping pipeline adds bidirectional text, font fallback, safe line breaking,
//! positioned glyphs, visual runs, and caret positions through caller-owned buffers.
//!
//! # Line layout
//!
//! ```
//! use textflow::TextFlow;
//!
//! let text = "A small UI can still set type well.";
//! let lines = TextFlow::new(text, 12)
//!     .map(|line| line.text())
//!     .collect::<Vec<_>>();
//!
//! assert_eq!(lines, ["A small UI", "can still", "set type", "well."]);
//! ```

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(test)]
extern crate std;

use crate::line::Lines;

#[cfg(feature = "shaping")]
pub use crate::layout::{Alignment, WrapMode};
pub use crate::line::Line;

#[cfg(feature = "shaping")]
use crate::bidi::BaseDirection;
#[cfg(all(feature = "alloc", feature = "shaping"))]
use crate::shaping::Typeface;
#[cfg(feature = "shaping")]
use crate::shaping::{FlowPoint, FontFeature};
#[cfg(all(feature = "alloc", feature = "shaping"))]
use crate::workspace::{TextWorkspace, WorkspaceError};

#[cfg(feature = "shaping")]
mod buffer;
mod line;
mod properties;

#[cfg(feature = "bidi")]
pub mod bidi;
#[cfg(feature = "shaping")]
pub mod layout;
#[cfg(feature = "shaping")]
pub mod shaping;
#[cfg(not(feature = "unicode"))]
mod unicode;
#[cfg(feature = "unicode")]
pub mod unicode;
#[cfg(all(feature = "alloc", feature = "shaping"))]
pub mod workspace;

pub struct TextFlow<'a> {
    text: &'a str,
    max_width: usize,
    line_height: usize,
    line_spacing: usize,
    lines: Lines<'a>,
    #[cfg(feature = "shaping")]
    base_direction: BaseDirection,
    #[cfg(feature = "shaping")]
    features: &'a [FontFeature],
    #[cfg(feature = "shaping")]
    origin: FlowPoint,
    #[cfg(feature = "shaping")]
    wrap: WrapMode,
    #[cfg(feature = "shaping")]
    letter_spacing: i32,
    #[cfg(feature = "shaping")]
    word_spacing: i32,
    #[cfg(feature = "shaping")]
    alignment: Alignment,
}

impl<'a> TextFlow<'a> {
    pub fn new(text: &'a str, max_width: usize) -> Self {
        let mut flow = TextFlow {
            text,
            max_width,
            line_height: 0,
            line_spacing: 0,
            lines: Lines::new("", 0, 4),
            #[cfg(feature = "shaping")]
            base_direction: BaseDirection::Auto,
            #[cfg(feature = "shaping")]
            features: &[],
            #[cfg(feature = "shaping")]
            origin: FlowPoint { x: 0, y: 0 },
            #[cfg(feature = "shaping")]
            wrap: WrapMode::Word,
            #[cfg(feature = "shaping")]
            letter_spacing: 0,
            #[cfg(feature = "shaping")]
            word_spacing: 0,
            #[cfg(feature = "shaping")]
            alignment: Alignment::Start,
        };

        flow.lines = Lines::new(flow.text, flow.max_width, 4);

        flow
    }

    pub const fn with_line_height(mut self, line_height: usize) -> Self {
        self.line_height = line_height;
        self
    }

    pub const fn with_line_spacing(mut self, line_spacing: usize) -> Self {
        self.line_spacing = line_spacing;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_direction(mut self, direction: BaseDirection) -> Self {
        self.base_direction = direction;
        self
    }

    #[cfg(feature = "shaping")]
    pub fn with_features(mut self, features: &'a [FontFeature]) -> Self {
        self.features = features;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_origin(mut self, origin: FlowPoint) -> Self {
        self.origin = origin;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_wrap(mut self, wrap: WrapMode) -> Self {
        self.wrap = wrap;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_letter_spacing(mut self, spacing: i32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_word_spacing(mut self, spacing: i32) -> Self {
        self.word_spacing = spacing;
        self
    }

    #[cfg(feature = "shaping")]
    pub const fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[cfg(all(feature = "alloc", feature = "shaping"))]
    pub fn layout<'workspace>(
        &self,
        typefaces: &[&dyn Typeface],
        workspace: &'workspace mut TextWorkspace,
    ) -> Result<layout::ParagraphLayout<'workspace>, WorkspaceError> {
        workspace.layout(self, typefaces)
    }
}

impl<'a> Iterator for TextFlow<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = self.lines.next()?;
        line.set_metrics(self.line_height, self.line_spacing);
        Some(line)
    }
}
