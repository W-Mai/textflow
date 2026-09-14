import { renderRust } from "./code-highlight.js";

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

function sourceLink(rev, source, line, label = "View source ↗") {
  const link = element("a", "", label);
  link.href = `https://github.com/W-Mai/textflow/blob/${encodeURIComponent(rev)}/${source}#L${line}`;
  link.target = "_blank";
  link.rel = "noreferrer";
  return link;
}

function rustCode(source) {
  const pre = element("pre", "");
  renderRust(pre, source);
  return pre;
}

function docText(target, markdown) {
  if (!markdown?.trim()) {
    return;
  }
  let code = false;
  let rust = false;
  let buffer = [];
  const flush = () => {
    if (!buffer.length) return;
    const source = buffer.join("\n");
    target.append(code && rust ? rustCode(source) : element(code ? "pre" : "p", "", source));
    buffer = [];
  };
  for (const line of markdown.split("\n")) {
    if (line.trim().startsWith("```")) {
      flush();
      const language = line.trim().slice(3).trim();
      rust = !code && (!language || /^(?:rust|rs)(?:[,\s]|$)/.test(language));
      code = !code;
      continue;
    }
    if (!code && !line.trim()) {
      flush();
    } else {
      buffer.push(line);
    }
  }
  flush();
}

const guides = [
  {
    id: "lines",
    title: "Break lines",
    features: "Default features",
    summary: "Use the iterator when a display-cell width and borrowed line slices are enough.",
    code: `use textflow::TextFlow;

let text = "A small UI can still set type well.";
for line in TextFlow::new(text, 12) {
    println!("{:>2}  {}", line.width(), line.text());
}`,
    notes: ["The iterator returns slices of the original UTF-8 text. Width is measured in display cells; no font adapter is involved."],
  },
  {
    id: "glyphs",
    title: "Position glyphs",
    features: "shaping",
    summary: "Provide a Typeface, lay out into fixed-capacity storage, then read lines, visual runs, glyphs, and carets.",
    code: `use textflow::{LayoutScratch, TextFlow, WrapMode};
use textflow::shaping::Typeface;

fn positions(text: &str, font: &dyn Typeface) -> Result<(), textflow::LayoutError> {
    let mut scratch = LayoutScratch::<32, 256, 16, 512>::new();
    let layout = TextFlow::new(text, 4800)
        .with_line_height(1200)
        .with_wrap(WrapMode::WordOrGrapheme)
        .layout_with_scratch(&[font], &mut scratch)?;
    for glyph in layout.glyphs() {
        let range = glyph.cluster.start as usize..glyph.cluster.end as usize;
        let x = glyph.origin.x + glyph.offset.x;
        let y = glyph.origin.y + glyph.offset.y;
        println!("{:?} @ ({x}, {y})", &text[range]);
    }
    Ok(())
}`,
    notes: ["Glyph order is visual order. Cluster ranges are UTF-8 byte offsets into the input; widths, line heights, origins, and advances use the font adapter's coordinate units.", "Use layout.lines(), layout.runs(), and layout.carets() for line origins, font selection, bidi levels, and cursor positions."],
  },
  {
    id: "scripts",
    title: "Connect font data",
    features: "shaping · complex-shaping · script-arabic",
    summary: "A GlyphSource maps characters and metrics; ShapingData supplies the font's substitution and positioning lookups.",
    code: `use textflow::{LayoutError, LayoutScratch, TextFlow, scripts};
use textflow::shaping::{GlyphSource, ScriptProvider, ScriptTypeface, ShapingData};

fn shape_arabic(
    glyphs: &dyn GlyphSource,
    lookups: &dyn ShapingData,
) -> Result<(), LayoutError> {
    let providers: [&dyn ScriptProvider; 1] = [&scripts::ARABIC];
    let font = ScriptTypeface::new(glyphs, lookups).with_scripts(&providers);
    let mut scratch = LayoutScratch::<8, 64, 4, 128>::new();
    let layout = TextFlow::new("مرحبا", 4800)
        .layout_with_scratch(&[&font], &mut scratch)?;
    println!("{} glyphs", layout.glyphs().len());
    Ok(())
}`,
    notes: ["Pass the resulting font to TextFlow::layout_with_scratch. The same Typeface slice may contain fallback fonts for other scripts.", "For scalar cmap, advance, .notdef, and pair kerning without script lookups, wrap a GlyphSource with SimpleTypeface. Arabic and Devanagari require a script-capable Typeface."],
    source: "playground/src/font.rs",
    sourceLabel: "Working font adapter ↗",
  },
  {
    id: "directions",
    title: "Bidi and overflow",
    features: "shaping",
    summary: "Auto direction resolves the paragraph from its text. Alignment::Start follows that direction.",
    code: `use textflow::{Alignment, LayoutError, LayoutScratch, Overflow, TextFlow, WrapMode};
use textflow::bidi::BaseDirection;
use textflow::shaping::Typeface;

fn inspect_runs(font: &dyn Typeface) -> Result<(), LayoutError> {
    let mut scratch = LayoutScratch::<8, 64, 4, 128>::new();
    let flow = TextFlow::new("שלום, hello", 4800)
        .with_direction(BaseDirection::Auto)
        .with_alignment(Alignment::Start)
        .with_wrap(WrapMode::NoWrap)
        .with_overflow(Overflow::Clip);
    let layout = flow.layout_with_scratch(&[font], &mut scratch)?;
    for run in layout.runs() {
        println!("level {}: {:?}", run.bidi_level(), run.text());
    }
    Ok(())
}`,
    notes: ["When a no-wrap line exceeds the width, RTL Start places its leading text at the right edge; LTR Start places it at the left edge. Clip is a viewport policy: renderers clip positioned glyphs to the requested width.", "Justify expands eligible wrapped lines; lines without expandable gaps remain start-aligned.", "Ellipsis reshapes the final visible line and inserts a synthetic run. Inspect run.is_synthetic() when mapping output back to source text."],
  },
  {
    id: "baselines",
    title: "Place on a baseline",
    features: "shaping",
    summary: "Map an existing ParagraphLayout onto borrowed geometry using caller-owned frame buffers.",
    code: `use textflow::ParagraphLayout;
use textflow::placement::{GlyphFrame, PlacementError, PlacementOutput, PolylineBaseline};
use textflow::shaping::FlowPoint;

fn place(layout: &ParagraphLayout<'_>) -> Result<(), PlacementError> {
    let points = [FlowPoint { x: 0, y: 0 }, FlowPoint { x: 4800, y: 800 }];
    let baseline = PolylineBaseline::new(&points).expect("valid line");
    let baselines = [baseline];
    let placement = layout.place_on(&baselines);
    placement.preflight()?;
    let mut frames = [GlyphFrame::default(); 256];
    let placed = placement.place_into(PlacementOutput::new(&mut frames))?;
    for frame in placed.glyph_frames() {
        println!("{:?} {:?}", frame.local_origin, frame.unit_tangent);
    }
    Ok(())
}`,
    notes: ["Supply one baseline per laid-out line. Use with_line_widths() to match each line's available path length; preflight reports invalid geometry and required output capacity."],
  },
  {
    id: "memory",
    title: "Bound memory",
    features: "shaping · alloc",
    summary: "A TextWorkspace can grow reusable private buffers within explicit limits while final output stays caller-owned.",
    code: `use textflow::TextFlow;
use textflow::layout::{LayoutLine, VisualRun};
use textflow::shaping::{CaretStop, PositionedGlyph, Typeface};
use textflow::workspace::{LayoutLimits, LayoutOutput, TextWorkspace, WorkspaceError};

fn layout_bounded(text: &str, font: &dyn Typeface) -> Result<(), WorkspaceError> {
    let limits = LayoutLimits {
        text_bytes: 4096, runs: 64, glyphs: 256,
        scratch_glyphs: 512, lines: 16, memory_bytes: 128 * 1024,
    };
    let mut workspace = TextWorkspace::new(limits);
    let mut glyphs = [PositionedGlyph::default(); 256];
    let mut runs = [VisualRun::empty(); 64];
    let mut lines = [LayoutLine::empty(); 16];
    let mut carets = [CaretStop::default(); 512];
    let mut output = LayoutOutput::new(&mut glyphs, &mut runs, &mut lines, &mut carets);
    let layout = TextFlow::new(text, 4800)
        .layout_into(&[font], &mut workspace, &mut output)?;
    println!("{} glyphs", layout.glyphs().len());
    Ok(())
}`,
    notes: ["LayoutLimits::memory_bytes bounds only workspace intermediates, not final output or allocator bookkeeping. Reuse the workspace between paragraphs to retain capacity.", "For stack-only storage, use LayoutScratch and layout_with_scratch instead."],
  },
];

export class DocsView {
  constructor(list, detail, search, docs, rev) {
    this.list = list;
    this.detail = detail;
    this.search = search;
    this.docs = docs;
    this.rev = rev;
    this.active = new Set();
    this.selected = null;
    search.addEventListener("input", () => this.render(this.active));
    this.selectGuide(guides[0]);
  }

  visible(features, enabled) {
    return features.every((feature) => enabled.has(feature));
  }

  render(enabled) {
    this.active = enabled;
    const query = this.search.value.trim().toLowerCase();
    this.list.replaceChildren();
    let count = 0;
    const matches = new Set();
    const visibleGuides = guides.filter((guide) =>
      !query || `${guide.title} ${guide.summary} ${guide.features} ${guide.notes.join(" ")}`.toLowerCase().includes(query)
    );
    if (visibleGuides.length) {
      const group = element("div", "docs-group");
      group.append(element("h3", "", "Guides"));
      for (const guide of visibleGuides) {
        const key = `guide:${guide.id}`;
        const button = element("button", key === this.selected ? "is-active" : "", guide.title);
        button.type = "button";
        button.addEventListener("click", () => this.selectGuide(guide));
        group.append(button);
        matches.add(key);
        count += 1;
      }
      this.list.append(group);
    }
    for (const module of this.docs.modules) {
      if (!this.visible(module.features, enabled)) continue;
      const items = module.items.filter((item) => {
        if (!this.visible(item.features, enabled)) return false;
        const methods = (item.methods ?? [])
          .filter((method) => this.visible(method.features, enabled))
          .map((method) => `${method.name} ${method.doc} ${method.signature}`)
          .join(" ");
        return !query || `${module.name} ${item.name} ${item.kind} ${item.doc} ${item.signature} ${methods}`.toLowerCase().includes(query);
      });
      if (!items.length) continue;
      const group = element("div", "docs-group");
      group.append(element("h3", "", module.name || "crate root"));
      for (const item of items) {
        const key = `${module.name}::${item.name}`;
        const button = element("button", key === this.selected ? "is-active" : "", item.name);
        button.type = "button";
        button.addEventListener("click", () => this.select(module, item));
        group.append(button);
        matches.add(key);
        count += 1;
      }
      this.list.append(group);
    }
    if (!count) this.list.append(element("p", "docs-placeholder", "No items match the active features and query."));
    if (this.selected) {
      if (!matches.has(this.selected)) {
        this.selected = null;
        this.detail.replaceChildren(element("div", "docs-placeholder", "Select a guide or API item."));
      }
    }
  }

  selectGuide(guide) {
    this.selected = `guide:${guide.id}`;
    this.render(this.active);
    this.detail.replaceChildren();
    this.detail.append(element("span", "doc-kicker", "GUIDE · " + guide.features.toUpperCase()));
    this.detail.append(element("h3", "", guide.title));
    this.detail.append(element("p", "", guide.summary));
    this.detail.append(rustCode(guide.code));
    for (const note of guide.notes) this.detail.append(element("p", "", note));
    if (guide.source) this.detail.append(sourceLink(this.rev, guide.source, 1, guide.sourceLabel));
  }

  select(module, item) {
    this.selected = `${module.name}::${item.name}`;
    this.render(this.active);
    this.detail.replaceChildren();
    this.detail.append(element("span", "doc-kicker", `${item.kind.toUpperCase()} · ${module.name || "CRATE ROOT"}`));
    this.detail.append(element("h3", "", item.name));
    const features = element("div", "");
    for (const feature of item.features) features.append(element("span", "doc-feature", feature));
    if (item.features.length) this.detail.append(features);
    this.detail.append(rustCode(item.signature));
    docText(this.detail, item.doc);
    this.detail.append(sourceLink(this.rev, item.source ?? module.source, item.line));
    const query = this.search.value.trim().toLowerCase();
    const itemMatches = `${module.name} ${item.name} ${item.kind} ${item.doc} ${item.signature}`.toLowerCase().includes(query);
    for (const method of item.methods ?? []) {
      if (!this.visible(method.features, this.active)) continue;
      if (query && !itemMatches && !`${method.name} ${method.doc} ${method.signature}`.toLowerCase().includes(query)) continue;
      const block = element("section", "doc-method");
      block.append(element("h4", "", method.name));
      block.append(rustCode(method.signature));
      docText(block, method.doc);
      block.append(sourceLink(this.rev, item.source ?? module.source, method.line, `Source line ${method.line} ↗`));
      this.detail.append(block);
    }
  }
}
