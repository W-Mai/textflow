use crate::font::{character, DemoFont, DemoLatin, DemoShaping, UNITS_PER_EM};
use serde::{Deserialize, Serialize};
use textflow::bidi::{BaseDirection, BidiRun, BidiText, Direction};
use textflow::layout::{LayoutLine, VisualRun};
use textflow::placement::{
    BaselineError, CaretFrame, GlyphFrame, PlacementError, PlacementOutput, PolylineBaseline,
    TextBaseline,
};
use textflow::shaping::{
    CaretStop, FlowPoint, FontFeature, PositionedGlyph, ScriptProvider, ScriptTypeface,
    SimpleTypeface, Typeface,
};
use textflow::unicode::{graphemes, line_breaks, script_runs};
use textflow::workspace::{LayoutLimits, LayoutOutput, TextWorkspace};
use textflow::{scripts, Alignment, LayoutScratch, Overflow, TextFlow, WrapMode};

#[derive(Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub width: u32,
    pub font_size: u32,
    pub line_height: u32,
    pub line_spacing: u32,
    pub direction: String,
    pub wrap: String,
    pub alignment: String,
    pub overflow: String,
    pub max_lines: usize,
    pub letter_spacing: i32,
    pub word_spacing: i32,
    pub kern: bool,
    pub liga: bool,
    pub text_limit: usize,
    pub memory_limit: usize,
    pub path: Option<Vec<[i32; 2]>>,
    pub path_sampled: bool,
    pub path_scale: f32,
    pub smoothing: u8,
    pub motion_depth: u8,
    pub motion_phase: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 480,
            font_size: 32,
            line_height: 42,
            line_spacing: 8,
            direction: "auto".into(),
            wrap: "word".into(),
            alignment: "start".into(),
            overflow: "clip".into(),
            max_lines: 12,
            letter_spacing: 0,
            word_spacing: 0,
            kern: false,
            liga: true,
            text_limit: 4096,
            memory_limit: 131_072,
            path: None,
            path_sampled: false,
            path_scale: 1.0,
            smoothing: 2,
            motion_depth: 8,
            motion_phase: 0.0,
        }
    }
}

#[derive(Serialize)]
pub struct Response {
    pub scene: String,
    pub ok: bool,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Failure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Data>,
}

#[derive(Serialize)]
pub struct SceneSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub requires: &'static [&'static str],
    pub sample: &'static str,
    pub description: &'static str,
}

pub const SCENES: &[SceneSpec] = &[
    SceneSpec {
        id: "core",
        label: "Line flow",
        requires: &[],
        sample: "A small boat crossed the harbor as the evening lights came on.",
        description: "Line breaks and widths.",
    },
    SceneSpec {
        id: "unicode",
        label: "Unicode",
        requires: &["unicode"],
        sample: "你好 · cafe\u{301} · ภาษาไทย · कक्षा",
        description: "Grapheme clusters, break opportunities and script runs.",
    },
    SceneSpec {
        id: "bidi",
        label: "Bidi",
        requires: &["bidi"],
        sample: "مرحبا · Gate 4 · إلى القاهرة",
        description: "Watch three text runs change places in an RTL paragraph.",
    },
    SceneSpec {
        id: "shaping",
        label: "Glyph layout",
        requires: &["shaping"],
        sample: "To Avery, the office felt brighter with flowers.",
        description: "Glyph positions, visual runs and carets.",
    },
    SceneSpec {
        id: "arabic",
        label: "Arabic",
        requires: &["script-arabic"],
        sample: "باب تبت",
        description: "Contextual joining forms.",
    },
    SceneSpec {
        id: "thai",
        label: "Thai",
        requires: &["script-thai"],
        sample: "ภาษาไทย กิ",
        description: "Clusters and combining marks.",
    },
    SceneSpec {
        id: "devanagari",
        label: "Devanagari",
        requires: &["script-devanagari"],
        sample: "कि कमल",
        description: "Conjuncts and pre-base matras.",
    },
    SceneSpec {
        id: "workspace",
        label: "Memory",
        requires: &["alloc"],
        sample: "Stone paths wind past the old library, beneath windows full of light.",
        description: "Workspace limits and output sizes.",
    },
    SceneSpec {
        id: "geometry",
        label: "Baselines",
        requires: &["shaping"],
        sample: "Lorem ipsum dolor sit amet, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.",
        description: "Draw a curve and place type along it.",
    },
];

#[derive(Serialize)]
pub struct Failure {
    pub kind: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Data {
    Core {
        lines: Vec<CoreLine>,
    },
    Unicode {
        graphemes: Vec<Span>,
        breaks: Vec<Break>,
        scripts: Vec<ScriptSpan>,
    },
    Bidi {
        direction: String,
        logical: Vec<Run>,
        visual: Vec<Run>,
    },
    Layout(LayoutView),
}

#[derive(Serialize)]
pub struct CoreLine {
    pub text: String,
    pub range: [usize; 2],
    pub width: usize,
}

#[derive(Serialize)]
pub struct Span {
    pub text: String,
    pub range: [usize; 2],
}

#[derive(Serialize)]
pub struct Break {
    pub offset: usize,
    pub kind: String,
}

#[derive(Serialize)]
pub struct ScriptSpan {
    pub range: [usize; 2],
    pub script: String,
}

#[derive(Serialize)]
pub struct Run {
    pub range: [usize; 2],
    pub direction: String,
    pub level: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutView {
    pub lines: Vec<LayoutLineView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baselines: Option<Vec<Vec<[i32; 2]>>>,
    pub glyphs: Vec<Glyph>,
    pub carets: Vec<Caret>,
    pub runs: Vec<VisualRunView>,
    pub private_bytes: Option<usize>,
    pub output_bytes: usize,
    pub synthetic_font: bool,
    pub geometry: bool,
}

#[derive(Serialize)]
pub struct LayoutLineView {
    pub text: String,
    pub range: [u32; 2],
    pub glyphs: [u32; 2],
    pub carets: [u32; 2],
    pub origin: [i32; 2],
    pub advance: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Glyph {
    pub character: String,
    pub id: u16,
    pub cluster: [u32; 2],
    pub origin: [i32; 2],
    pub advance: [i32; 2],
    pub offset: [i32; 2],
    pub level: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
}

#[derive(Serialize)]
pub struct Frame {
    pub origin: [i32; 2],
    pub tangent: [i32; 2],
}

#[derive(Serialize)]
pub struct Caret {
    pub offset: u32,
    pub position: [i32; 2],
    pub level: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
}

#[derive(Serialize)]
pub struct VisualRunView {
    pub text: [u32; 2],
    pub glyphs: [u32; 2],
    pub level: u8,
    pub font: u64,
}

#[derive(Clone, Copy)]
struct GeometryView<'a> {
    glyphs: &'a [GlyphFrame],
    carets: &'a [CaretFrame],
    points: &'a [FlowPoint],
}

pub fn analyze(scene: &str, text: &str, options: &Options) -> Response {
    let result = match scene {
        "core" => Ok(core(text, options)),
        "unicode" => Ok(unicode(text)),
        "bidi" => bidi(text, options),
        "shaping" | "arabic" | "thai" | "devanagari" | "workspace" | "geometry" => {
            layout(scene, text, options)
        }
        _ => Err(Failure {
            kind: "UnknownScene".into(),
            message: format!("Unknown scene: {scene}"),
        }),
    };
    match result {
        Ok(data) => Response {
            scene: scene.into(),
            ok: true,
            code: builder(scene, options),
            error: None,
            data: Some(data),
        },
        Err(error) => Response {
            scene: scene.into(),
            ok: false,
            code: builder(scene, options),
            error: Some(error),
            data: None,
        },
    }
}

fn builder(scene: &str, options: &Options) -> String {
    if scene == "unicode" {
        return "graphemes(text) · line_breaks(text) · script_runs(text)".into();
    }
    if scene == "bidi" {
        return "BidiText::resolve(text, 0..text.len(), direction, &mut runs)\n    .visual_runs_into(&mut visual)".into();
    }
    if scene == "geometry" {
        return format!(
            "let baseline = PolylineBaseline::new(POINTS)?;\nlet layout = TextFlow::new(text, baseline.length() as usize)\n    .with_wrap(WrapMode::WordOrGrapheme)\n    .with_overflow(Overflow::{:?})\n    .with_alignment(Alignment::{:?})\n    .with_max_lines(1)\n    .with_letter_spacing({})\n    .with_word_spacing({})\n    .layout_with_scratch(&[&typeface], &mut scratch)?;\nlet placement = layout.place_on(&[baseline]);\nlet required = placement.preflight()?;\nplacement.place_into(\n    PlacementOutput::new(&mut glyph_frames[..required.glyphs])\n        .with_carets(&mut caret_frames[..required.carets])\n)?;",
            overflow(&options.overflow),
            alignment(&options.alignment),
            options.letter_spacing,
            options.word_spacing
        );
    }
    let width = if scene == "core" {
        (options.width / 16).max(1) as usize
    } else {
        to_units(options.width.clamp(32, 1200), options.font_size).max(1)
    };
    let mut code = if scene == "shaping" {
        format!(
            "let providers: [&dyn ScriptProvider; 1] = [&latin_provider];\nlet typeface = ScriptTypeface::new(&font, &shaping).with_scripts(&providers);\nlet features = [FontFeature::new(*b\"kern\", {}), FontFeature::new(*b\"liga\", {})];\nTextFlow::new(text, {width})",
            u8::from(options.kern),
            u8::from(options.liga)
        )
    } else if options.kern && scene != "core" {
        format!("let kern = [FontFeature::new(*b\"kern\", 1)];\nTextFlow::new(text, {width})")
    } else {
        format!("TextFlow::new(text, {width})")
    };
    if scene != "core" {
        code.push_str(&format!(
            "\n    .with_line_height({})\n    .with_line_spacing({})",
            to_units(options.line_height, options.font_size),
            to_units(options.line_spacing, options.font_size)
        ));
        code.push_str(&format!(
            "\n    .with_direction(BaseDirection::{:?})\n    .with_wrap(WrapMode::{:?})\n    .with_alignment(Alignment::{:?})\n    .with_overflow(Overflow::{:?})\n    .with_max_lines({})\n    .with_letter_spacing({})\n    .with_word_spacing({})",
            direction(&options.direction),
            wrap(&options.wrap),
            alignment(&options.alignment),
            overflow(&options.overflow),
            options.max_lines.min(64),
            options.letter_spacing,
            options.word_spacing
        ));
        if scene == "shaping" {
            code.push_str("\n    .with_features(&features)");
        } else if options.kern {
            code.push_str("\n    .with_features(&kern)");
        }
        if scene == "workspace" {
            code.push_str("\n    .layout_into(&[&typeface], &mut workspace, &mut output)");
        } else {
            code.push_str("\n    .layout_with_scratch(&[&typeface], &mut scratch)");
        }
    }
    code
}

fn to_units(pixels: u32, font_size: u32) -> usize {
    (f64::from(pixels) * f64::from(UNITS_PER_EM) / f64::from(font_size.clamp(12, 96))).round()
        as usize
}

const DEFAULT_PATH: [[i32; 2]; 7] = [
    [10, 125],
    [95, 55],
    [180, 20],
    [285, 75],
    [390, 130],
    [500, 100],
    [610, 35],
];

fn path_units(value: f64, font_size: u32) -> i32 {
    (value * f64::from(UNITS_PER_EM) / f64::from(font_size.clamp(12, 96))).round() as i32
}

fn smooth_path(options: &Options) -> Result<Vec<FlowPoint>, Failure> {
    let samples = options.path.as_deref().unwrap_or(&DEFAULT_PATH);
    let limit = if options.path_sampled { 2049 } else { 512 };
    if !options.path_scale.is_finite()
        || options.path_scale <= 0.0
        || !(2..=limit).contains(&samples.len())
        || (options.path_sampled && options.path.is_none())
        || samples
            .iter()
            .flatten()
            .any(|value| value.unsigned_abs() > 30_000)
    {
        return Err(Failure {
            kind: "InvalidPath".into(),
            message: if options.path_sampled {
                "The example path needs 2–2049 points inside the canvas"
            } else {
                "Draw a curve with 2–512 points inside the canvas"
            }
            .into(),
        });
    }
    let mut anchors: Vec<[f64; 2]> = samples
        .iter()
        .map(|point| {
            [
                f64::from(point[0]) * f64::from(options.path_scale),
                f64::from(point[1]) * f64::from(options.path_scale),
            ]
        })
        .collect();
    for _ in 0..if options.path_sampled {
        0
    } else {
        options.smoothing.min(4)
    } {
        let mut filtered = anchors.clone();
        for index in 1..anchors.len() - 1 {
            for axis in 0..2 {
                filtered[index][axis] = (anchors[index.saturating_sub(2)][axis]
                    + 4.0 * anchors[index - 1][axis]
                    + 6.0 * anchors[index][axis]
                    + 4.0 * anchors[(index + 1).min(anchors.len() - 1)][axis]
                    + anchors[(index + 2).min(anchors.len() - 1)][axis])
                    / 16.0;
            }
        }
        anchors = filtered;
    }
    let curve = if options.path_sampled {
        anchors
    } else {
        let mut curve = Vec::with_capacity(anchors.len() * 8);
        for index in 0..anchors.len() - 1 {
            let a = anchors[index];
            let b = anchors[index + 1];
            let before = anchors[index.saturating_sub(1)];
            let after = anchors[(index + 2).min(anchors.len() - 1)];
            let distance = (b[0] - a[0]).hypot(b[1] - a[1]);
            let subdivisions = ((distance / 8.0).ceil() as usize).clamp(2, 8);
            for step in 0..subdivisions {
                let t = step as f64 / subdivisions as f64;
                let t2 = t * t;
                let t3 = t2 * t;
                let mut point = [0.0; 2];
                for axis in 0..2 {
                    let start_tangent = (b[axis] - before[axis]) * 0.5;
                    let end_tangent = (after[axis] - a[axis]) * 0.5;
                    point[axis] = (2.0 * t3 - 3.0 * t2 + 1.0) * a[axis]
                        + (t3 - 2.0 * t2 + t) * start_tangent
                        + (-2.0 * t3 + 3.0 * t2) * b[axis]
                        + (t3 - t2) * end_tangent;
                }
                curve.push(point);
            }
        }
        curve.push(*anchors.last().unwrap());
        curve
    };
    let total: f64 = curve
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
        .sum();
    if total <= 0.0 || !total.is_finite() {
        return Err(Failure {
            kind: "InvalidPath".into(),
            message: "The curve has no measurable length".into(),
        });
    }
    let phase = if options.motion_phase.is_finite() {
        f64::from(options.motion_phase.clamp(-100_000.0, 100_000.0))
    } else {
        0.0
    };
    let depth = f64::from(options.motion_depth.min(20));
    let mut traveled = 0.0;
    let mut points = Vec::with_capacity(curve.len());
    for index in 0..curve.len() {
        if index > 0 {
            traveled += (curve[index][0] - curve[index - 1][0])
                .hypot(curve[index][1] - curve[index - 1][1]);
        }
        let t = traveled / total;
        let before = curve[index.saturating_sub(1)];
        let after = curve[(index + 1).min(curve.len() - 1)];
        let tangent = [after[0] - before[0], after[1] - before[1]];
        let length = tangent[0].hypot(tangent[1]).max(1.0);
        let envelope = (core::f64::consts::PI * t).sin().powi(2);
        let wave = 0.7 * (t * 13.0 + phase).sin() + 0.3 * (t * 29.0 - phase * 0.71).sin();
        let offset = depth * envelope * wave;
        let x = curve[index][0] - tangent[1] / length * offset;
        let y = curve[index][1] + tangent[0] / length * offset;
        if x.abs() > 30_000.0 || y.abs() > 30_000.0 {
            return Err(Failure {
                kind: "InvalidPath".into(),
                message: "The curve exceeds the canvas limits".into(),
            });
        }
        points.push(FlowPoint {
            x: path_units(x, options.font_size),
            y: path_units(y, options.font_size),
        });
    }
    Ok(points)
}

fn core(text: &str, options: &Options) -> Data {
    let columns = (options.width / 16).max(1) as usize;
    let lines = TextFlow::new(text, columns)
        .map(|line| CoreLine {
            text: line.text().into(),
            range: [line.range().start, line.range().end],
            width: line.width(),
        })
        .collect();
    Data::Core { lines }
}

fn unicode(text: &str) -> Data {
    Data::Unicode {
        graphemes: graphemes(text)
            .map(|item| Span {
                text: item.text.into(),
                range: [item.range.start, item.range.end],
            })
            .collect(),
        breaks: line_breaks(text)
            .map(|item| Break {
                offset: item.offset,
                kind: format!("{:?}", item.kind),
            })
            .collect(),
        scripts: script_runs(text)
            .map(|item| ScriptSpan {
                range: [item.text.start, item.text.end],
                script: format!("{:?}", item.script),
            })
            .collect(),
    }
}

fn direction(value: &str) -> BaseDirection {
    match value {
        "ltr" => BaseDirection::LeftToRight,
        "rtl" => BaseDirection::RightToLeft,
        _ => BaseDirection::Auto,
    }
}

fn run(value: &BidiRun) -> Run {
    Run {
        range: [value.text.start, value.text.end],
        direction: match value.direction {
            Direction::LeftToRight => "ltr",
            Direction::RightToLeft => "rtl",
        }
        .into(),
        level: value.level,
    }
}

fn bidi(text: &str, options: &Options) -> Result<Data, Failure> {
    let mut logical = vec![BidiRun::empty(); text.len().clamp(1, 512)];
    let resolved = BidiText::resolve(
        text,
        0..text.len(),
        direction(&options.direction),
        &mut logical,
    )
    .map_err(|error| fail("Bidi", error))?;
    let mut visual = vec![BidiRun::empty(); resolved.logical_runs().len()];
    let visual = resolved
        .visual_runs_into(&mut visual)
        .map_err(|error| fail("Bidi", error))?;
    Ok(Data::Bidi {
        direction: match resolved.direction() {
            Direction::LeftToRight => "ltr",
            Direction::RightToLeft => "rtl",
        }
        .into(),
        logical: resolved.logical_runs().iter().map(run).collect(),
        visual: visual.iter().map(run).collect(),
    })
}

fn fail(kind: &str, error: impl core::fmt::Debug) -> Failure {
    Failure {
        kind: kind.into(),
        message: format!("{error:?}"),
    }
}

fn wrap(value: &str) -> WrapMode {
    match value {
        "none" => WrapMode::NoWrap,
        "grapheme" => WrapMode::Grapheme,
        "word-or-grapheme" => WrapMode::WordOrGrapheme,
        _ => WrapMode::Word,
    }
}

fn alignment(value: &str) -> Alignment {
    match value {
        "center" => Alignment::Center,
        "end" => Alignment::End,
        "justify" => Alignment::Justify,
        _ => Alignment::Start,
    }
}

fn overflow(value: &str) -> Overflow {
    match value {
        "ellipsis" => Overflow::Ellipsis,
        _ => Overflow::Clip,
    }
}

fn layout(scene: &str, text: &str, options: &Options) -> Result<Data, Failure> {
    if text.len() > 4096 {
        return Err(Failure {
            kind: "TextLimit".into(),
            message: "The demo accepts at most 4096 UTF-8 bytes".into(),
        });
    }
    let font = DemoFont;
    let shaping = DemoShaping;
    let simple = SimpleTypeface::new(&font);
    let providers: [&dyn ScriptProvider; 4] = [
        &DemoLatin,
        &scripts::ARABIC,
        &scripts::THAI,
        &scripts::DEVANAGARI,
    ];
    let complex = ScriptTypeface::new(&font, &shaping).with_scripts(&providers);
    let face: &dyn Typeface = match scene {
        "shaping" | "arabic" | "thai" | "devanagari" => &complex,
        _ => &simple,
    };
    let font_size = options.font_size.clamp(12, 96);
    let points = if scene == "geometry" {
        Some(smooth_path(options)?)
    } else {
        None
    };
    let baseline = points
        .as_ref()
        .map(|points| PolylineBaseline::new(points).map_err(|error| fail("Baseline", error)))
        .transpose()?;
    let max_width = baseline
        .as_ref()
        .map(|baseline| baseline.length() as usize)
        .unwrap_or_else(|| to_units(options.width.clamp(32, 1200), font_size).max(1));
    let kern = [FontFeature::new(*b"kern", 1)];
    let latin = [
        FontFeature::new(*b"kern", u32::from(options.kern)),
        FontFeature::new(*b"liga", u32::from(options.liga)),
    ];
    let mut flow = TextFlow::new(text, max_width)
        .with_line_height(to_units(options.line_height, font_size))
        .with_line_spacing(to_units(options.line_spacing, font_size))
        .with_direction(direction(&options.direction))
        .with_wrap(wrap(&options.wrap))
        .with_alignment(alignment(&options.alignment))
        .with_overflow(overflow(&options.overflow))
        .with_max_lines(options.max_lines.min(64))
        .with_letter_spacing(options.letter_spacing)
        .with_word_spacing(options.word_spacing);
    if scene == "geometry" {
        flow = flow.with_wrap(WrapMode::WordOrGrapheme).with_max_lines(1);
    }
    if scene == "shaping" {
        flow = flow.with_features(&latin);
    } else if options.kern && scene != "geometry" {
        flow = flow.with_features(&kern);
    }
    let faces = [face];
    if scene == "workspace" {
        let limits = LayoutLimits {
            text_bytes: options.text_limit,
            runs: 64,
            glyphs: 512,
            scratch_glyphs: 512,
            lines: 64,
            memory_bytes: options.memory_limit,
        };
        let mut workspace = TextWorkspace::new(limits);
        let mut glyphs = vec![PositionedGlyph::default(); 512];
        let mut runs = vec![VisualRun::empty(); 128];
        let mut lines = vec![LayoutLine::empty(); 64];
        let mut carets = vec![CaretStop::default(); 1024];
        let output_bytes = glyphs.len() * core::mem::size_of::<PositionedGlyph>()
            + runs.len() * core::mem::size_of::<VisualRun>()
            + lines.len() * core::mem::size_of::<LayoutLine>()
            + carets.len() * core::mem::size_of::<CaretStop>();
        let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);
        let result = flow
            .layout_into(&faces, &mut workspace, &mut output)
            .map_err(|error| fail("Workspace", error))?;
        return Ok(Data::Layout(view(
            text,
            &result,
            Some(workspace.resident_bytes()),
            output_bytes,
            None,
        )));
    }
    if scene == "geometry" {
        layout_fixed::<1280, 1280>(text, &flow, &faces, points.as_deref(), baseline)
    } else {
        layout_fixed::<512, 1024>(text, &flow, &faces, points.as_deref(), baseline)
    }
}

fn layout_fixed<const GLYPHS: usize, const CARETS: usize>(
    text: &str,
    flow: &TextFlow<'_>,
    faces: &[&dyn Typeface],
    points: Option<&[FlowPoint]>,
    baseline: Option<PolylineBaseline<'_>>,
) -> Result<Data, Failure> {
    let mut scratch = LayoutScratch::<64, GLYPHS, 64, CARETS>::new();
    let result = flow
        .layout_with_scratch(faces, &mut scratch)
        .map_err(|error| fail("Layout", error))?;
    let (frames, caret_frames) = if let Some(baseline) = baseline {
        let baselines = [baseline];
        let operation = result.place_on(&baselines);
        let required = operation.preflight().map_err(|error| match error {
            PlacementError::Baseline {
                error: BaselineError::OutOfRange { .. },
                ..
            } => Failure {
                kind: "NoGlyphFits".into(),
                message: "The curve cannot hold a glyph at this size".into(),
            },
            _ => fail("Baseline", error),
        })?;
        let mut glyph_frames = vec![GlyphFrame::default(); required.glyphs];
        let mut caret_frames = vec![CaretFrame::default(); required.carets];
        operation
            .place_into(PlacementOutput::new(&mut glyph_frames).with_carets(&mut caret_frames))
            .map_err(|error| fail("Baseline", error))?;
        (Some(glyph_frames), Some(caret_frames))
    } else {
        (None, None)
    };
    let output_bytes = core::mem::size_of::<LayoutScratch<64, GLYPHS, 64, CARETS>>();
    let geometry = match (&frames, &caret_frames, &points) {
        (Some(glyphs), Some(carets), Some(points)) => Some(GeometryView {
            glyphs,
            carets,
            points,
        }),
        _ => None,
    };
    Ok(Data::Layout(view(
        text,
        &result,
        None,
        output_bytes,
        geometry,
    )))
}

fn view(
    text: &str,
    layout: &textflow::ParagraphLayout<'_>,
    private_bytes: Option<usize>,
    output_bytes: usize,
    geometry: Option<GeometryView<'_>>,
) -> LayoutView {
    LayoutView {
        lines: layout
            .lines()
            .iter()
            .map(|line| {
                let range = line.text();
                let glyphs = line.glyphs();
                let carets = line.carets();
                let origin = line.origin();
                LayoutLineView {
                    text: text
                        .get(range.start as usize..range.end as usize)
                        .unwrap_or("")
                        .into(),
                    range: [range.start, range.end],
                    glyphs: [glyphs.start, glyphs.end],
                    carets: [carets.start, carets.end],
                    origin: [origin.x, origin.y],
                    advance: line.advance(),
                }
            })
            .collect(),
        baselines: geometry.map(|geometry| {
            vec![geometry
                .points
                .iter()
                .map(|point| [point.x, point.y])
                .collect()]
        }),
        glyphs: layout
            .glyphs()
            .iter()
            .enumerate()
            .map(|(index, glyph)| Glyph {
                character: character(glyph.glyph_id())
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                id: glyph.glyph_id().value(),
                cluster: [glyph.cluster.start, glyph.cluster.end],
                origin: [glyph.origin.x, glyph.origin.y],
                advance: [glyph.advance.x, glyph.advance.y],
                offset: [glyph.offset.x, glyph.offset.y],
                level: glyph.bidi_level,
                frame: geometry
                    .and_then(|geometry| geometry.glyphs.get(index))
                    .map(|frame| Frame {
                        origin: [frame.local_origin.x, frame.local_origin.y],
                        tangent: [frame.unit_tangent.x, frame.unit_tangent.y],
                    }),
            })
            .collect(),
        carets: layout
            .carets()
            .iter()
            .enumerate()
            .map(|(index, caret)| Caret {
                offset: caret.text_offset,
                position: [caret.position.x, caret.position.y],
                level: caret.bidi_level,
                frame: geometry
                    .and_then(|geometry| geometry.carets.get(index))
                    .map(|frame| Frame {
                        origin: [frame.local_origin.x, frame.local_origin.y],
                        tangent: [frame.unit_tangent.x, frame.unit_tangent.y],
                    }),
            })
            .collect(),
        runs: layout
            .runs()
            .iter()
            .map(|run| {
                let text = run.text();
                let glyphs = run.glyphs();
                VisualRunView {
                    text: [text.start, text.end],
                    glyphs: [glyphs.start, glyphs.end],
                    level: run.bidi_level(),
                    font: run.font_id().value(),
                }
            })
            .collect(),
        private_bytes,
        output_bytes,
        synthetic_font: true,
        geometry: geometry.is_some(),
    }
}
