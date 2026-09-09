#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(test)]
extern crate std;

use crate::line::Line;

pub use crate::line::{LineInfo, LinePosition};

#[cfg(feature = "shaping")]
use crate::bidi::BaseDirection;
#[cfg(all(feature = "alloc", feature = "shaping"))]
use crate::shaping::Typeface;
#[cfg(feature = "shaping")]
use crate::shaping::{FlowPoint, FontFeature};
#[cfg(all(feature = "alloc", feature = "shaping"))]
use crate::workspace::{TextWorkspace, WorkspaceError};

mod line;
mod lookahead;
mod word;

#[cfg(feature = "bidi")]
pub mod bidi;
#[cfg(feature = "shaping")]
pub mod layout;
#[cfg(feature = "shaping")]
pub mod shaping;
#[cfg(feature = "unicode")]
pub mod unicode;
#[cfg(all(feature = "alloc", feature = "shaping"))]
pub mod workspace;

pub struct TextFlow<'a> {
    text: &'a str,
    max_width: usize,
    line_height: usize,
    line_spacing: usize,
    lines: Line<'a>,
    #[cfg(feature = "shaping")]
    base_direction: BaseDirection,
    #[cfg(feature = "shaping")]
    features: &'a [FontFeature],
    #[cfg(feature = "shaping")]
    origin: FlowPoint,
}

impl<'a> TextFlow<'a> {
    pub fn new(text: &'a str, max_width: usize) -> Self {
        let mut flow = TextFlow {
            text,
            max_width,
            line_height: 0,
            line_spacing: 0,
            lines: Line::new("", 0, 0, 0),
            #[cfg(feature = "shaping")]
            base_direction: BaseDirection::Auto,
            #[cfg(feature = "shaping")]
            features: &[],
            #[cfg(feature = "shaping")]
            origin: FlowPoint { x: 0, y: 0 },
        };

        flow.lines = Line::new(flow.text, flow.max_width, 0, 0).with_long_break(true);

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

    #[cfg(all(feature = "alloc", feature = "shaping"))]
    pub fn layout<'workspace>(
        &self,
        typefaces: &[&dyn Typeface],
        workspace: &'workspace mut TextWorkspace,
    ) -> Result<layout::ParagraphLayout<'workspace>, WorkspaceError> {
        workspace.layout(self, typefaces)
    }
}

impl Iterator for TextFlow<'_> {
    type Item = LineInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = self.lines.next()?;
        line.line_height = self.line_height;
        line.line_spacing = self.line_spacing;
        Some(line)
    }
}
