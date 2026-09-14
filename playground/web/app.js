import { FeatureGraph, renderFeatures } from "./features.js";
import { Stage, VIEWPORT_WIDTH, stageSummary } from "./stage.js";
import { DocsView } from "./docs.js";
import { renderCode, renderRust } from "./code-highlight.js";
import { BudgetView } from "./budget.js";
import { HEART_BOUNDS, HEART_PATH, loadPortrait, ODYSSEY_SOURCE, ODYSSEY_TEXT } from "./baseline-examples.js";
import { formatBaselinePoints } from "./baseline-code.js";
import { buildTiles, glyphTarget, revealPulse, stepGlyph } from "./atmosphere.js";

const $ = (id) => document.getElementById(id);
const state = {
  engine: null,
  memory: null,
  budget: null,
  catalog: null,
  graph: null,
  scenes: [],
  scene: "core",
  response: null,
  files: [],
  selectedFile: "src/main.rs",
  docs: null,
  descriptions: new Map(),
  baselineExample: "draw",
  baselineDrafts: {draw: null, yuuu: null},
  portrait: null,
  options: {
    width: 480, fontSize: 32, lineHeight: 42, lineSpacing: 8,
    direction: "auto", wrap: "word", alignment: "start", overflow: "clip",
    maxLines: 12, letterSpacing: 0, wordSpacing: 0, kern: false,
    textLimit: 4096, memoryLimit: 131072,
    path: HEART_PATH, showCurve: false, smoothing: 2, motionDepth: 8, motionPhase: 0,
    motionEnabled: true, motionSpeed: 0.7, geometryOverflow: "ellipsis",
  },
};

const controls = [
  {key: "width", label: "Viewport width", type: "range", ...VIEWPORT_WIDTH},
  {key: "fontSize", label: "Font size", type: "range", min: 12, max: 64, step: 1, needs: "shaping"},
  {key: "lineHeight", label: "Line height", type: "range", min: 20, max: 88, step: 1, needs: "shaping"},
  {key: "lineSpacing", label: "Line spacing", type: "range", min: 0, max: 32, step: 1, needs: "shaping"},
  {key: "direction", label: "Direction", type: "select", values: ["auto", "ltr", "rtl"], needs: "bidi"},
  {key: "wrap", label: "Wrap mode", type: "select", values: ["word", "word-or-grapheme", "grapheme", "none"], needs: "shaping"},
  {key: "alignment", label: "Alignment", type: "select", values: ["start", "center", "end", "justify"], needs: "shaping"},
  {key: "overflow", label: "Overflow", type: "select", values: ["clip", "ellipsis"], needs: "shaping"},
  {key: "geometryOverflow", label: "Overflow", type: "select", values: ["clip", "ellipsis"], only: "geometry"},
  {key: "maxLines", label: "Maximum lines", type: "range", min: 1, max: 20, step: 1, needs: "shaping"},
  {key: "letterSpacing", label: "Letter spacing · units", type: "range", min: -80, max: 180, step: 10, needs: "shaping"},
  {key: "wordSpacing", label: "Word spacing · units", type: "range", min: -100, max: 250, step: 10, needs: "shaping"},
  {key: "kern", label: "Enable kern feature", type: "check", needs: "shaping"},
  {key: "showCurve", label: "Show curve", type: "check", only: "geometry"},
  {key: "smoothing", label: "Smoothing", type: "range", min: 0, max: 4, step: 1, only: "geometry"},
  {key: "motionEnabled", label: "Animate curve", type: "check", only: "geometry"},
  {key: "motionDepth", label: "Motion depth · px", type: "range", min: 0, max: 20, step: 1, only: "geometry"},
  {key: "motionSpeed", label: "Motion speed", type: "range", min: 0.2, max: 2, step: 0.1, only: "geometry"},
  {key: "textLimit", label: "Text limit · bytes", type: "range", min: 4, max: 4096, step: 4, only: "workspace"},
  {key: "memoryLimit", label: "Private buffer limit", type: "range", min: 256, max: 131072, step: 256, only: "workspace"},
];
const baselineFields = ["fontSize", "alignment", "geometryOverflow", "letterSpacing", "wordSpacing", "showCurve", "smoothing", "motionEnabled", "motionDepth", "motionSpeed"];

function captureBaseline() {
  return {
    text: $("source").value,
    path: state.options.path,
    settings: Object.fromEntries(baselineFields.map((key) => [key, state.options[key]])),
  };
}

function restoreBaseline(draft) {
  $("source").value = draft.text;
  state.options.path = draft.path;
  Object.assign(state.options, draft.settings);
}

function yuuuDraft() {
  return {
    text: ODYSSEY_TEXT,
    path: state.portrait.points,
    settings: {...captureBaseline().settings, fontSize: 16, smoothing: 0, motionEnabled: true, motionDepth: 16, motionSpeed: 0.6, geometryOverflow: "clip"},
  };
}

async function selectBaselineExample(example) {
  if (state.scene !== "geometry" || example === state.baselineExample || !["draw", "yuuu"].includes(example) || exampleLoading) return;
  exampleLoading = true;
  renderExampleSwitch();
  let portrait;
  if (example === "yuuu") {
    try { portrait = await loadPortrait(); }
    catch (error) { toast(String(error)); exampleLoading = false; renderExampleSwitch(); return; }
    if (state.scene !== "geometry") { exampleLoading = false; renderExampleSwitch(); return; }
  }
  state.baselineDrafts[state.baselineExample] = captureBaseline();
  state.baselineExample = example;
  if (portrait) state.portrait = portrait;
  let draft = state.baselineDrafts[example];
  if (!draft) {
    draft = yuuuDraft();
    state.baselineDrafts[example] = draft;
  }
  restoreBaseline(draft);
  drawRevealStart = example === "draw" && draft.path === HEART_PATH && !reducedMotion.matches ? performance.now() : 0;
  stage.setEditable(example === "draw");
  $("reset-path").hidden = example !== "draw";
  renderControls();
  refresh();
  syncBaselineMotion();
  exampleLoading = false;
  renderExampleSwitch();
}

let exampleLoading = false;
let freeDrawInviteTimer;
let drawRevealStart = 0;
const DRAW_REVEAL_MS = 1800;
const DRAW_SPRING_END = 1 - 1.5 * Math.exp(-6) + 0.5 * Math.exp(-18);

function drawSpringProgress(elapsed) {
  const time = Math.max(0, Math.min(1, elapsed / DRAW_REVEAL_MS));
  return (1 - 1.5 * Math.exp(-6 * time) + 0.5 * Math.exp(-18 * time)) / DRAW_SPRING_END;
}
let toastTimer;
let updateFrame;
const stage = new Stage($("stage"), inspect, (points) => {
  drawRevealStart = 0;
  state.options.path = points;
  schedule();
}, setViewportWidth);
const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
let themeMode = "auto";

function startAtmosphere() {
  const surface = document.querySelector(".atmosphere");
  const ctx = surface.getContext("2d");
  let tiles = [];
  let colors = [];
  let width = 0;
  let height = 0;
  let frame = 0;
  let last = 0;
  let visible = false;
  const target = {x: 0, y: 0, strength: 0};
  const pointer = {x: 0, y: 0, strength: 0};
  const build = () => {
    width = Math.max(1, window.innerWidth);
    height = Math.max(1, window.innerHeight);
    const ratio = Math.min(2, window.devicePixelRatio || 1);
    surface.width = Math.round(width * ratio);
    surface.height = Math.round(height * ratio);
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.font = "700 10px ui-monospace, SFMono-Regular, Consolas, monospace";
    const widths = new Map();
    tiles = buildTiles(width, height).map((tile) => {
      let x = tile.x;
      const glyphs = Array.from(tile.text, (text, index) => {
        if (!widths.has(text)) widths.set(text, ctx.measureText(text).width);
        const advance = widths.get(text);
        const glyph = {
          text, x, y: tile.y, centerX: x + advance / 2, centerY: tile.y - 4,
          index: tile.index + index, dx: 0, dy: 0, vx: 0, vy: 0,
        };
        x += advance;
        return glyph;
      });
      return {...tile, glyphs};
    });
  };
  const draw = (time) => {
    const seconds = time / 1000;
    const pulse = revealPulse(seconds);
    const sweepX = -180 + (width + 360) * (seconds % 31 / 31);
    const focusX = pointer.strength * pointer.x + (1 - pointer.strength) * sweepX;
    const focusY = pointer.strength * pointer.y + (1 - pointer.strength) * height / 2;
    ctx.clearRect(0, 0, width, height);
    ctx.font = "700 10px ui-monospace, SFMono-Regular, Consolas, monospace";
    ctx.textBaseline = "alphabetic";
    if (pointer.strength > .01 || pulse > .01) {
      const light = ctx.createRadialGradient(focusX, focusY, 0, focusX, focusY, 260);
      light.addColorStop(0, colors[1]);
      light.addColorStop(1, "transparent");
      ctx.globalAlpha = .11 * pointer.strength + .025 * pulse;
      ctx.fillStyle = light;
      ctx.fillRect(focusX - 260, focusY - 260, 520, 520);
    }
    for (const tile of tiles) {
      let moving = false;
      let proximity = 0;
      for (const glyph of tile.glyphs) {
        const desired = glyphTarget(glyph, pointer);
        moving = stepGlyph(glyph, desired) || moving;
        proximity = Math.max(proximity, desired.proximity);
      }
      const scan = pulse * Math.max(0, 1 - Math.abs(tile.x - sweepX) / 280);
      ctx.globalAlpha = Math.min(.76, .19 + .12 * scan + .5 * proximity);
      ctx.fillStyle = colors[tile.tone];
      if (moving) {
        for (const glyph of tile.glyphs) {
          ctx.fillText(glyph.text, glyph.x + glyph.dx, glyph.y + glyph.dy);
        }
      } else {
        ctx.fillText(tile.text, tile.x, tile.y);
      }
    }
  };
  const refreshPalette = () => {
    const style = getComputedStyle(document.documentElement);
    colors = ["--muted", "--rust", "--mint"].map((token) => style.getPropertyValue(token).trim());
    draw(reducedMotion.matches ? 0 : performance.now());
  };
  const tick = (time) => {
    if (time - last >= 1000 / 24) {
      pointer.x += (target.x - pointer.x) * .14;
      pointer.y += (target.y - pointer.y) * .14;
      pointer.strength += (target.strength - pointer.strength) * .12;
      draw(time);
      last = time;
    }
    frame = requestAnimationFrame(tick);
  };
  const sync = () => {
    cancelAnimationFrame(frame);
    frame = 0;
    if (reducedMotion.matches) {
      pointer.strength = 0;
      target.strength = 0;
      for (const tile of tiles) {
        for (const glyph of tile.glyphs) {
          glyph.dx = glyph.dy = glyph.vx = glyph.vy = 0;
        }
      }
    }
    draw(reducedMotion.matches ? 0 : performance.now());
    if (!reducedMotion.matches && !document.hidden && visible) frame = requestAnimationFrame(tick);
  };
  window.addEventListener("pointermove", (event) => {
    if (event.pointerType === "touch" || reducedMotion.matches) return;
    target.x = event.clientX;
    target.y = event.clientY;
    target.strength = 1;
  }, {passive: true});
  window.addEventListener("pointerout", (event) => {
    if (!event.relatedTarget) target.strength = 0;
  });
  window.addEventListener("blur", () => { target.strength = 0; });
  document.addEventListener("visibilitychange", sync);
  reducedMotion.addEventListener("change", sync);
  window.addEventListener("resize", () => {
    target.x = pointer.x = window.innerWidth / 2;
    target.y = pointer.y = window.innerHeight / 2;
    build();
    sync();
  });
  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    sync();
  }).observe(surface);
  target.x = pointer.x = window.innerWidth / 2;
  target.y = pointer.y = window.innerHeight / 2;
  build();
  refreshPalette();
  sync();
  return refreshPalette;
}

const refreshAtmosphereTheme = startAtmosphere();

let baselineFrame = 0;
let baselineTime = 0;
let baselinePaint = 0;
let baselineVisible = false;
function baselineActive() {
  return state.scene === "geometry" && baselineVisible && (state.options.motionEnabled || drawRevealStart)
    && !reducedMotion.matches && !document.hidden && $("view-playground").classList.contains("is-active");
}
function baselineTick(time) {
  if (!baselineActive()) { baselineFrame = 0; baselineTime = 0; return; }
  const elapsed = baselineTime ? Math.min(0.1, (time - baselineTime) / 1000) : 0;
  baselineTime = time;
  if (!stage.drawing) {
    const revealFinished = drawRevealStart && time - drawRevealStart >= DRAW_REVEAL_MS;
    if (revealFinished) drawRevealStart = 0;
    if (state.options.motionEnabled) state.options.motionPhase = (state.options.motionPhase + elapsed * state.options.motionSpeed) % (200 * Math.PI);
    if (revealFinished || time - baselinePaint >= 1000 / 24) {
      baselinePaint = time;
      refresh(!revealFinished);
    }
  }
  if (baselineActive()) baselineFrame = requestAnimationFrame(baselineTick);
  else { baselineFrame = 0; baselineTime = 0; }
}
function syncBaselineMotion() {
  cancelAnimationFrame(baselineFrame);
  baselineFrame = 0;
  baselineTime = 0;
  if (baselineActive()) baselineFrame = requestAnimationFrame(baselineTick);
}
document.addEventListener("visibilitychange", syncBaselineMotion);
reducedMotion.addEventListener("change", () => { syncBaselineMotion(); schedule(); });
new IntersectionObserver(([entry]) => {
  baselineVisible = entry.isIntersecting;
  syncBaselineMotion();
}).observe($("stage"));

try {
  const saved = localStorage.getItem("textflow-theme");
  if (saved === "light" || saved === "dark") themeMode = saved;
} catch {}

function applyTheme() {
  if (themeMode === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = themeMode;
  const button = $("theme-toggle");
  button.textContent = themeMode.toUpperCase();
  button.setAttribute("aria-label", `Color theme: ${themeMode}. Click to change`);
  document.querySelector('meta[name="theme-color"]').content = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
  refreshAtmosphereTheme();
  stage.redraw();
}

$("theme-toggle").addEventListener("click", () => {
  const preferred = systemTheme.matches ? "dark" : "light";
  const opposite = preferred === "dark" ? "light" : "dark";
  themeMode = themeMode === "auto" ? opposite : themeMode === opposite ? preferred : "auto";
  try { localStorage.setItem("textflow-theme", themeMode); } catch {}
  applyTheme();
});
systemTheme.addEventListener("change", () => { if (themeMode === "auto") applyTheme(); });
applyTheme();

function toast(message) {
  const node = $("toast");
  node.textContent = message;
  node.classList.add("is-visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => node.classList.remove("is-visible"), 2500);
}

async function copy(text, label = "Copied") {
  try {
    await navigator.clipboard.writeText(text);
    toast(label);
  } catch {
    toast("Clipboard access is unavailable in this browser");
  }
}

function row(label, value) {
  const node = document.createElement("div");
  node.className = "inspect-row";
  const key = document.createElement("span");
  key.textContent = label;
  const data = document.createElement("b");
  data.textContent = String(value);
  node.append(key, data);
  return node;
}

function inspect(hit) {
  const target = $("inspector");
  target.replaceChildren();
  if (hit) {
    const title = document.createElement("strong");
    title.textContent = hit.type.toUpperCase();
    target.append(title);
    if (hit.type === "glyph") {
      target.append(row("Character", hit.character || "unmapped"), row("Glyph ID", hit.id), row("UTF-8 cluster", hit.cluster.join("…")), row("Origin · units", hit.origin.join(", ")), row("Advance · units", hit.advance.join(", ")), row("Bidi level", hit.level));
      if (hit.frame) target.append(row("Baseline frame", hit.frame.origin.join(", ")));
    } else if (hit.type === "caret") {
      target.append(row("Text offset", hit.offset), row("Position", hit.position.join(", ")), row("Bidi level", hit.level));
    } else if (hit.type === "line") {
      target.append(row("Text", hit.text), row("UTF-8 bytes", hit.range.join("…")), row("Width", hit.width));
    } else if (hit.type === "grapheme") {
      target.append(row("Cluster", hit.text), row("UTF-8 bytes", hit.range.join("…")), row("Script", hit.script), row("Break after", hit.breakAfter ? "yes" : "no"));
    } else if (hit.type === "source run" || hit.type === "screen run") {
      target.append(row("Source run", hit.sourceIndex), row("Text", hit.text), row("Direction", hit.direction.toUpperCase()), row("Embedding level", hit.level));
    } else {
      target.append(row("Text", hit.text), row("UTF-8 bytes", hit.range.join("…")), row("Direction", hit.direction), row("Level", hit.level));
    }
    return;
  }
  const response = state.response;
  if (!response?.ok) {
    const title = document.createElement("strong");
    title.textContent = response?.error?.kind ?? "Engine unavailable";
    const message = document.createElement("p");
    message.textContent = response?.error?.message ?? "The WebAssembly module did not load.";
    target.append(title, message);
    return;
  }
  const data = response.data;
  const title = document.createElement("strong");
  title.textContent = "SUMMARY";
  target.append(title);
  if (data.kind === "layout") {
    target.append(row("Lines", data.lines.length), row("Glyphs", data.glyphs.length), row("Visual runs", data.runs.length), row("Caret stops", data.carets.length), row(data.privateBytes == null ? "Scratch capacity" : "Caller output", `${data.outputBytes.toLocaleString()} B`));
    if (stage.visibleGlyphs !== null && stage.visibleGlyphs < data.glyphs.length) target.append(row("Visible glyphs", stage.visibleGlyphs));
    if (data.privateBytes != null) target.append(row("Private buffers", `${data.privateBytes.toLocaleString()} B`));
    target.append(row("Font source", "synthetic demo adapter"));
  } else if (data.kind === "unicode") {
    target.append(row("Graphemes", data.graphemes.length), row("Breaks", data.breaks.length), row("Script runs", data.scripts.length));
  } else if (data.kind === "bidi") {
    target.append(row("Paragraph direction", data.direction), row("Logical runs", data.logical.length), row("Visual runs", data.visual.length));
    const order = data.visual.map((run) => data.logical.findIndex((source) => source.range[0] === run.range[0] && source.range[1] === run.range[1]) + 1);
    target.append(row("Screen order · left to right", order.join(" → ")));
  } else {
    target.append(row("Lines", data.lines.length), row("Total width", data.lines.reduce((sum, line) => sum + line.width, 0)));
  }
  const note = document.createElement("p");
  note.textContent = "Select an element for details.";
  target.append(note);
}

function currentScene() {
  return state.scenes.find((scene) => scene.id === state.scene) ?? state.scenes[0];
}

function shapingScene() {
  return !["core", "unicode", "bidi"].includes(state.scene);
}

function setViewportWidth(width) {
  if (state.options.width === width) return;
  state.options.width = width;
  const input = $("controls").querySelector('input[data-key="width"]');
  if (input) {
    input.value = String(width);
    input.closest("label").querySelector("output").textContent = width.toLocaleString();
  }
  schedule();
}

function renderControls() {
  const target = $("controls");
  $("view-playground").classList.toggle("is-yuuu", state.scene === "geometry" && state.baselineExample === "yuuu");
  renderExampleSwitch();
  target.replaceChildren();
  if (state.scene === "geometry") {
    if (state.baselineExample === "yuuu") {
      const source = document.createElement("a");
      source.className = "example-source";
      source.href = ODYSSEY_SOURCE;
      source.target = "_blank";
      source.rel = "noreferrer";
      source.textContent = "Homer · Book 9, 21–28 ↗";
      target.append(source);
    }
  }
  for (const definition of controls) {
    if (definition.only && definition.only !== state.scene) continue;
    if (definition.key === "width" && ["unicode", "bidi"].includes(state.scene)) continue;
    if (state.scene === "geometry" && !["fontSize", "geometryOverflow", "alignment", "letterSpacing", "wordSpacing", "showCurve", "smoothing", "motionEnabled", "motionDepth", "motionSpeed"].includes(definition.key)) continue;
    if (definition.key === "showCurve" && state.baselineExample !== "draw") continue;
    if (state.baselineExample === "yuuu" && definition.key === "smoothing") continue;
    if (definition.needs === "shaping" && !shapingScene()) continue;
    if (definition.needs === "bidi" && ["core", "unicode"].includes(state.scene)) continue;
    const label = document.createElement("label");
    label.className = definition.type === "check" ? "control-check" : "control";
    const top = document.createElement("span");
    top.className = "control-top";
    const name = document.createElement("span");
    name.textContent = definition.label;
    top.append(name);
    let input;
    if (definition.type === "range") {
      const output = document.createElement("output");
      output.textContent = Number(state.options[definition.key]).toLocaleString();
      top.append(output);
      input = document.createElement("input");
      Object.assign(input, {type: "range", min: definition.min, max: definition.max, step: definition.step, value: state.options[definition.key]});
      input.dataset.key = definition.key;
      input.addEventListener("input", () => {
        if (definition.key === "width") return setViewportWidth(Number(input.value));
        state.options[definition.key] = Number(input.value);
        output.textContent = Number(input.value).toLocaleString();
        schedule();
      });
      label.append(top, input);
    } else if (definition.type === "select") {
      input = document.createElement("select");
      for (const value of definition.values) {
        const option = document.createElement("option");
        option.value = value;
        option.textContent = value;
        input.append(option);
      }
      input.value = state.options[definition.key];
      input.addEventListener("change", () => {
        state.options[definition.key] = input.value;
        schedule();
      });
      label.append(top, input);
    } else {
      input = document.createElement("input");
      input.type = "checkbox";
      input.checked = state.options[definition.key];
      input.addEventListener("change", () => {
        state.options[definition.key] = input.checked;
        syncBaselineMotion();
        schedule();
      });
      label.append(name, input);
    }
    target.append(label);
  }
}

function renderExampleSwitch() {
  const visible = state.scene === "geometry";
  $("baseline-examples").hidden = !visible;
  $("detail-column").classList.toggle("has-example-switch", visible);
  if (!visible || state.baselineExample !== "yuuu") {
    clearTimeout(freeDrawInviteTimer);
    $("example-draw").classList.remove("is-invited");
  }
  for (const example of ["draw", "yuuu"]) {
    const button = $(`example-${example}`);
    const active = state.baselineExample === example;
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-pressed", String(active));
    button.disabled = exampleLoading;
  }
}

function inviteFreeDraw() {
  if (state.scene !== "geometry" || state.baselineExample !== "yuuu") return;
  const button = $("example-draw");
  button.classList.add("is-invited");
  clearTimeout(freeDrawInviteTimer);
  freeDrawInviteTimer = setTimeout(() => button.classList.remove("is-invited"), 3200);
}

function renderSceneTabs() {
  const target = $("scene-tabs");
  target.replaceChildren();
  state.scenes.forEach((scene, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.role = "tab";
    button.textContent = scene.label;
    button.className = `scene-tab${scene.id === "geometry" ? " scene-tab-baselines" : ""}${scene.id === state.scene ? " is-active" : ""}${scene.requires.some((feature) => !state.graph.active(feature)) ? " is-locked" : ""}`;
    if (scene.id === "geometry") {
      const mark = document.createElement("span");
      mark.className = "baseline-mark";
      mark.setAttribute("aria-hidden", "true");
      mark.innerHTML = '<svg viewBox="0 0 30 15"><path d="M1 12 C7 2 11 3 16 9 S25 13 29 3"/></svg>';
      button.prepend(mark);
    }
    button.setAttribute("aria-selected", scene.id === state.scene ? "true" : "false");
    if (scene.requires.length) button.title = `Requires ${scene.requires.join(", ")}`;
    button.addEventListener("click", () => selectScene(scene.id));
    target.append(button);
  });
}

function renderFeatureState() {
  renderFeatures($("features"), state.graph, state.descriptions, (name) => {
    state.graph.toggle(name);
    if (currentScene().requires.some((feature) => !state.graph.active(feature))) state.scene = "core";
    renderFeatureState();
    renderSceneTabs();
    renderControls();
    refresh();
  });
  $("add-command").textContent = state.graph.command();
  state.docs?.render(state.graph.closure());
  renderScaffold();
}

function selectScene(id) {
  const scene = state.scenes.find((item) => item.id === id);
  if (!scene) return;
  if (state.scene === "geometry") state.baselineDrafts[state.baselineExample] = captureBaseline();
  if (id !== "geometry") drawRevealStart = 0;
  for (const feature of scene.requires) state.graph.enable(feature);
  state.scene = id;
  stage.setEditable(id === "geometry" && state.baselineExample === "draw");
  $("reset-path").hidden = id !== "geometry" || state.baselineExample !== "draw";
  $("copy-points").hidden = id !== "geometry";
  const baselineDraft = state.baselineDrafts[state.baselineExample];
  if (id === "geometry" && baselineDraft) restoreBaseline(baselineDraft);
  else $("source").value = scene.sample;
  $("scene-title").textContent = scene.label;
  $("scene-description").textContent = scene.description;
  $("scene-counter").textContent = `${String(state.scenes.indexOf(scene) + 1).padStart(2, "0")} / ${String(state.scenes.length).padStart(2, "0")}`;
  renderFeatureState();
  renderSceneTabs();
  renderControls();
  refresh();
  syncBaselineMotion();
}

function schedule() {
  cancelAnimationFrame(updateFrame);
  updateFrame = requestAnimationFrame(() => refresh());
}

function sceneOptions() {
  if (state.scene !== "geometry") return {...state.options, path: null};
  const revealElapsed = drawRevealStart && !reducedMotion.matches ? performance.now() - drawRevealStart : Infinity;
  const pathBounds = state.baselineExample === "yuuu" ? state.portrait?.viewBox
    : state.options.path === HEART_PATH ? HEART_BOUNDS : null;
  const projection = stage.projection(stage.canvas.clientWidth, stage.canvas.clientHeight, {
    fontSize: state.options.fontSize, example: state.baselineExample, pathBounds,
  }, true);
  return {
    ...state.options,
    pathSampled: state.baselineExample === "yuuu",
    example: state.baselineExample,
    pathBounds,
    pathScale: projection.fit,
    reveal: drawSpringProgress(revealElapsed),
    overflow: state.options.geometryOverflow,
    motionDepth: state.options.motionEnabled && !reducedMotion.matches && !stage.drawing ? state.options.motionDepth : 0,
  };
}

function refresh(motionOnly = false) {
  if (!state.engine) return;
  const text = $("source").value;
  const options = sceneOptions();
  if (!motionOnly) $("byte-count").textContent = `${new TextEncoder().encode(text).length} UTF-8 BYTES`;
  try {
    state.response = state.engine.analyze_scene(state.scene, text, options);
  } catch (error) {
    state.response = {scene: state.scene, ok: false, error: {kind: "WasmError", message: String(error)}};
  }
  if (!motionOnly) renderRust($("code-preview"), state.response.code ?? "Engine unavailable");
  const empty = $("canvas-empty");
  empty.hidden = state.scene === "geometry" || !!state.response.ok;
  empty.textContent = state.response.ok ? "" : `${state.response.error?.kind ?? "Error"}: ${state.response.error?.message ?? "Unknown failure"}`;
  stage.render(state.response, text, options, {boxes: $("show-boxes").checked, carets: $("show-carets").checked});
  const data = state.response.data;
  $("stage-summary").textContent = state.scene === "geometry" && !state.response.ok ? state.response.error?.message ?? "Draw a curve" : stageSummary(state.response, stage.visibleGlyphs);
  if (!motionOnly) {
    $("copy-points").disabled = state.scene !== "geometry" || !state.response.ok || !data?.baselines?.[0]?.length;
    $("unit-label").hidden = !state.response.ok || data.kind !== "layout";
    $("inspector-count").textContent = state.response.ok ? data.kind.toUpperCase() : "ERROR";
    inspect(null);
    state.budget?.update(state.graph, state.scene, text, options, state.response);
  }
}

function renderScaffold() {
  if (!state.engine || !state.graph) return;
  const response = state.engine.scaffold_files(state.graph.selected());
  if (!response.ok) {
    $("file-source").textContent = response.error;
    return;
  }
  state.files = response.files;
  renderCode($("scaffold-dependency"), state.files[0]?.content ?? "", "toml");
  const list = $("file-list");
  list.replaceChildren();
  if (!state.files.some((file) => file.path === state.selectedFile)) state.selectedFile = state.files[0]?.path;
  for (const file of state.files) {
    const button = document.createElement("button");
    button.textContent = file.path;
    button.className = file.path === state.selectedFile ? "is-active" : "";
    button.addEventListener("click", () => {
      state.selectedFile = file.path;
      renderScaffold();
    });
    list.append(button);
  }
  const selected = state.files.find((file) => file.path === state.selectedFile);
  $("file-name").textContent = selected?.path ?? "";
  if (!selected) return;
  const language = selected.path.endsWith(".rs") ? "rust" : selected.path.endsWith(".toml") ? "toml" : selected.path.endsWith(".md") ? "markdown" : "plain";
  if (language === "plain") $("file-source").textContent = selected.content;
  else renderCode($("file-source"), selected.content, language);
}

function switchView(name) {
  for (const button of document.querySelectorAll(".view-tab")) {
    const active = button.dataset.view === name;
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-selected", String(active));
  }
  for (const view of document.querySelectorAll(".view")) {
    const active = view.id === `view-${name}`;
    view.classList.toggle("is-active", active);
    view.hidden = !active;
  }
  if (name === "playground") requestAnimationFrame(() => stage.redraw());
  syncBaselineMotion();
}

async function initialize() {
  try {
    const wasm = await import("./pkg/textflow_playground.js");
    const exports = await wasm.default();
    state.engine = wasm;
    state.memory = exports.memory;
    state.catalog = wasm.feature_catalog();
    state.graph = new FeatureGraph(state.catalog);
    state.scenes = wasm.scene_catalog();
    $("version-pill").textContent = `v${state.catalog.version}`;
    $("engine-status").hidden = true;
    const requestedView = new URLSearchParams(window.location.search).get("view");
    const otherView = requestedView === "scaffold" || requestedView === "docs";
    const [features, docs, budget, portrait] = await Promise.allSettled([
      fetch("./data/features.json").then((response) => { if (!response.ok) throw new Error("features.json unavailable"); return response.json(); }),
      fetch("./data/docs.json").then((response) => { if (!response.ok) throw new Error("docs.json unavailable"); return response.json(); }),
      fetch("./data/budget.json").then((response) => { if (!response.ok) throw new Error("budget.json unavailable"); return response.json(); }),
      otherView ? Promise.resolve(null) : loadPortrait(),
    ]);
    if (features.status === "fulfilled") state.descriptions = new Map(features.value.features.map((feature) => [feature.name, feature.description]));
    if (docs.status === "fulfilled") {
      state.docs = new DocsView($("docs-list"), $("docs-detail"), $("docs-search"), docs.value, state.catalog.source_rev);
    } else {
      $("docs-detail").textContent = `Documentation index unavailable: ${docs.reason}`;
    }
    if (budget.status === "fulfilled" && budget.value.crateVersion === state.catalog.version) {
      state.budget = new BudgetView($("budget-panel"), budget.value, wasm, state.memory);
    } else {
      $("budget-note").textContent = budget.status === "fulfilled" ? "Reference measurements do not match this release." : `Reference measurements unavailable: ${budget.reason}`;
    }
    if (otherView) {
      selectScene("core");
      switchView(requestedView);
    } else {
      if (portrait.status === "fulfilled") {
        state.portrait = portrait.value;
        state.baselineDrafts.draw = {...captureBaseline(), text: state.scenes.find((scene) => scene.id === "geometry").sample};
        state.baselineExample = "yuuu";
        state.baselineDrafts.yuuu = yuuuDraft();
      }
      selectScene("geometry");
      if (portrait.status === "rejected") toast(String(portrait.reason));
    }
  } catch (error) {
    $("engine-status").classList.add("is-error");
    $("engine-status").innerHTML = "<i></i> Could not load";
    state.response = {ok: false, error: {kind: "InitializationError", message: String(error)}};
    $("canvas-empty").hidden = false;
    $("canvas-empty").textContent = `Could not start: ${error}`;
    inspect(null);
  }
}

for (const tab of document.querySelectorAll(".view-tab")) tab.addEventListener("click", () => switchView(tab.dataset.view));
for (const example of ["draw", "yuuu"]) $("example-" + example).addEventListener("click", () => selectBaselineExample(example));
stage.canvas.addEventListener("click", inviteFreeDraw);
$("source").addEventListener("input", schedule);
$("show-boxes").addEventListener("change", schedule);
$("show-carets").addEventListener("change", schedule);
$("reset-path").addEventListener("click", () => {
  state.options.path = HEART_PATH;
  drawRevealStart = reducedMotion.matches ? 0 : performance.now();
  schedule();
  syncBaselineMotion();
});
$("copy-command").addEventListener("click", () => copy($("add-command").textContent, "Command copied"));
$("copy-code").addEventListener("click", () => copy($("code-preview").textContent, "API path copied"));
$("copy-points").addEventListener("click", () => {
  if (!state.engine || state.scene !== "geometry") return;
  const result = state.engine.analyze_scene("geometry", $("source").value,
    {...sceneOptions(), motionDepth: 0, motionPhase: 0});
  if (!result.ok) { toast(result.error?.message ?? "Curve points are unavailable"); return; }
  try { copy(formatBaselinePoints(result.data?.baselines?.[0]), "Curve points copied"); }
  catch (error) { toast(error.message); }
});
$("copy-dependency").addEventListener("click", () => copy($("scaffold-dependency").textContent, "Manifest copied"));
$("copy-file").addEventListener("click", () => copy($("file-source").textContent, "Source copied"));
$("download-zip").addEventListener("click", () => {
  if (!state.engine || !state.files.length) return;
  try {
    const bytes = state.engine.zip_files(state.files);
    const url = URL.createObjectURL(new Blob([bytes], {type: "application/zip"}));
    const link = document.createElement("a");
    link.href = url;
    link.download = "textflow-demo.zip";
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 30_000);
    toast("Project archive downloaded");
  } catch (error) {
    toast(`Archive failed: ${error}`);
  }
});
new ResizeObserver(() => state.scene === "geometry" ? refresh() : stage.redraw()).observe($("stage").parentElement);
initialize();
