use core::ops::Range;

pub use unicode_script::Script;
use unicode_script::UnicodeScript;
use unicode_segmentation::{GraphemeIndices, UnicodeSegmentation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grapheme<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptRun {
    pub text: Range<usize>,
    pub script: Script,
}

pub fn graphemes(text: &str) -> impl Iterator<Item = Grapheme<'_>> {
    text.grapheme_indices(true)
        .map(|(start, grapheme)| Grapheme {
            text: grapheme,
            range: start..start + grapheme.len(),
        })
}

pub fn line_breaks(text: &str) -> impl Iterator<Item = LineBreak> + '_ {
    unicode_linebreak::linebreaks(text).map(|(offset, kind)| LineBreak {
        offset,
        kind: match kind {
            unicode_linebreak::BreakOpportunity::Allowed => LineBreakKind::Allowed,
            unicode_linebreak::BreakOpportunity::Mandatory => LineBreakKind::Mandatory,
        },
    })
}

pub fn script_runs(text: &str) -> ScriptRuns<'_> {
    ScriptRuns {
        text,
        graphemes: text.grapheme_indices(true),
        pending: None,
    }
}

pub struct ScriptRuns<'a> {
    text: &'a str,
    graphemes: GraphemeIndices<'a>,
    pending: Option<(usize, &'a str)>,
}

impl Iterator for ScriptRuns<'_> {
    type Item = ScriptRun;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, first) = self.pending.take().or_else(|| self.graphemes.next())?;
        let mut script = strong_script(first);

        for (cluster_start, cluster) in self.graphemes.by_ref() {
            let next_script = strong_script(cluster);
            if script.is_none() {
                script = next_script;
            } else if next_script.is_some() && next_script != script {
                self.pending = Some((cluster_start, cluster));
                break;
            }
        }

        let end = if let Some((pending_start, _)) = self.pending {
            pending_start
        } else {
            self.text.len()
        };

        Some(ScriptRun {
            text: start..end,
            script: script.unwrap_or(Script::Common),
        })
    }
}

fn strong_script(grapheme: &str) -> Option<Script> {
    grapheme
        .chars()
        .map(|character| character.script())
        .find(|script| !matches!(script, Script::Common | Script::Inherited | Script::Unknown))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::*;

    #[test]
    fn grapheme_ranges_preserve_clusters() {
        let text = "a\u{301}👩‍🚀";
        let clusters = graphemes(text).collect::<Vec<_>>();

        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].text, "a\u{301}");
        assert_eq!(clusters[0].range, 0..3);
        assert_eq!(clusters[1].text, "👩‍🚀");
        assert_eq!(clusters[1].range, 3..text.len());
    }

    #[test]
    fn line_breaks_use_byte_offsets() {
        let text = "hello world";
        let breaks = line_breaks(text).collect::<Vec<_>>();

        assert!(breaks.contains(&LineBreak {
            offset: 6,
            kind: LineBreakKind::Allowed,
        }));
        assert_eq!(
            breaks.last(),
            Some(&LineBreak {
                offset: text.len(),
                kind: LineBreakKind::Mandatory,
            })
        );
    }

    #[test]
    fn neutral_prefix_joins_the_first_strong_script() {
        let text = "(العربية) Latin";
        let runs = script_runs(text).collect::<Vec<_>>();

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].script, Script::Arabic);
        assert_eq!(&text[runs[0].text.clone()], "(العربية) ");
        assert_eq!(runs[1].script, Script::Latin);
        assert_eq!(&text[runs[1].text.clone()], "Latin");
    }
}
