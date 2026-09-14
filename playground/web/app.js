import { FeatureGraph, renderFeatures } from "./features.js";
import { Stage, stageSummary } from "./stage.js";
import { DocsView } from "./docs.js";
import { renderRust } from "./rust-highlight.js";
import { loadPortrait, ODYSSEY_SOURCE, ODYSSEY_TEXT } from "./baseline-examples.js";
import { formatBaselinePoints } from "./baseline-code.js";

const $ = (id) => document.getElementById(id);
const state = {
  engine: null,
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
  baselineDrafts: {draw: null, odyssey: null},
  portrait: null,
  options: {
    width: 480, fontSize: 32, lineHeight: 42, lineSpacing: 8,
    direction: "auto", wrap: "word", alignment: "start", overflow: "clip",
    maxLines: 12, letterSpacing: 0, wordSpacing: 0, kern: false,
    textLimit: 4096, memoryLimit: 131072,
    path: null, smoothing: 2, motionDepth: 8, motionPhase: 0,
    motionEnabled: true, motionSpeed: 0.7, geometryOverflow: "ellipsis",
  },
};

const controls = [
  {key: "width", label: "Viewport width", type: "range", min: 32, max: 800, step: 16},
  {key: "fontSize", label: "Font size", type: "range", min: 16, max: 64, step: 1, needs: "shaping"},
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
  {key: "smoothing", label: "Smoothing", type: "range", min: 0, max: 4, step: 1, only: "geometry"},
  {key: "motionEnabled", label: "Animate curve", type: "check", only: "geometry"},
  {key: "motionDepth", label: "Motion depth · px", type: "range", min: 0, max: 20, step: 1, only: "geometry"},
  {key: "motionSpeed", label: "Motion speed", type: "range", min: 0.2, max: 2, step: 0.1, only: "geometry"},
  {key: "textLimit", label: "Text limit · bytes", type: "range", min: 4, max: 4096, step: 4, only: "workspace"},
  {key: "memoryLimit", label: "Private buffer limit", type: "range", min: 256, max: 131072, step: 256, only: "workspace"},
];
const baselineFields = ["fontSize", "alignment", "geometryOverflow", "letterSpacing", "wordSpacing", "smoothing", "motionEnabled", "motionDepth", "motionSpeed"];

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

async function selectBaselineExample(example) {
  if (state.scene !== "geometry" || example === state.baselineExample || !["draw", "odyssey"].includes(example)) return;
  let portrait;
  if (example === "odyssey") {
    try { portrait = await loadPortrait(); }
    catch (error) { toast(String(error)); renderControls(); return; }
    if (state.scene !== "geometry") return;
  }
  state.baselineDrafts[state.baselineExample] = captureBaseline();
  state.baselineExample = example;
  if (portrait) state.portrait = portrait;
  let draft = state.baselineDrafts[example];
  if (!draft) {
    draft = {
      text: ODYSSEY_TEXT,
      path: state.portrait.points,
      settings: {...captureBaseline().settings, fontSize: 16, smoothing: 0, motionDepth: 2},
    };
    state.baselineDrafts[example] = draft;
  }
  restoreBaseline(draft);
  stage.setEditable(example === "draw");
  $("reset-path").hidden = example !== "draw";
  renderControls();
  refresh();
  syncBaselineMotion();
}

let toastTimer;
let updateFrame;
const stage = new Stage($("stage"), inspect, (points) => {
  state.options.path = points;
  schedule();
});
const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
let themeMode = "auto";

function startAtmosphere() {
  const surface = document.querySelector(".atmosphere");
  const paths = ["zh", "en", "ja"].map((language) => $(`rights-path-${language}`));
  const heights = [20, 45, 70];
  const phases = heights.map(() => Math.random() * Math.PI * 2);
  let frame = 0;
  let last = 0;
  let visible = false;
  const draw = (time) => {
    paths.forEach((path, index) => {
      const y = heights[index];
      let d = "";
      for (let x = 0; x <= 640; x += 16) {
        const wave = 9 * Math.sin(x * .016 + time * .00024 + phases[0])
          + 4 * Math.sin(x * .037 - time * .00017 + phases[1])
          + 2 * Math.sin(x * .025 + time * .00011 + phases[2] + index * 1.9);
        d += `${x ? "L" : "M"}${x} ${(y + wave).toFixed(1)} `;
      }
      path.setAttribute("d", d);
    });
  };
  const tick = (time) => {
    if (time - last >= 33) { draw(time); last = time; }
    frame = requestAnimationFrame(tick);
  };
  const sync = () => {
    cancelAnimationFrame(frame);
    frame = 0;
    draw(reducedMotion.matches ? 0 : performance.now());
    if (!reducedMotion.matches && !document.hidden && visible) frame = requestAnimationFrame(tick);
  };
  document.addEventListener("visibilitychange", sync);
  reducedMotion.addEventListener("change", sync);
  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    sync();
  }).observe(surface);
  sync();
}

startAtmosphere();

let baselineFrame = 0;
let baselineTime = 0;
let baselinePaint = 0;
let baselineVisible = false;
function baselineActive() {
  return state.scene === "geometry" && baselineVisible && state.options.motionEnabled
    && !reducedMotion.matches && !document.hidden && $("view-playground").classList.contains("is-active");
}
function baselineTick(time) {
  if (!baselineActive()) { baselineFrame = 0; baselineTime = 0; return; }
  const elapsed = baselineTime ? Math.min(0.1, (time - baselineTime) / 1000) : 0;
  baselineTime = time;
  if (!stage.drawing) {
    state.options.motionPhase = (state.options.motionPhase + elapsed * state.options.motionSpeed) % (200 * Math.PI);
    if (time - baselinePaint >= 1000 / 24) {
      baselinePaint = time;
      refresh(true);
    }
  }
  baselineFrame = requestAnimationFrame(baselineTick);
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
    if (data.privateBytes != null) target.append(row("Private buffers", `${data.privateBytes.toLocaleString()} B`));
    target.append(row("Font source", "synthetic demo adapter"));
  } else if (data.kind === "unicode") {
    target.append(row("Graphemes", data.graphemes.length), row("Breaks", data.breaks.length), row("Script runs", data.scripts.length));
  } else if (data.kind === "bidi") {
    target.append(row("Paragraph direction", data.direction), row("Logical runs", data.logical.length), row("Visual runs", data.visual.length));
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

function renderControls() {
  const target = $("controls");
  target.replaceChildren();
  if (state.scene === "geometry") {
    const label = document.createElement("label");
    label.className = "control";
    const name = document.createElement("span");
    name.className = "control-top";
    name.textContent = "Example";
    const select = document.createElement("select");
    for (const [value, text] of [["draw", "Free draw"], ["odyssey", "Odyssey · Ithaca"]]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = text;
      select.append(option);
    }
    select.value = state.baselineExample;
    select.addEventListener("change", () => {
      select.disabled = true;
      selectBaselineExample(select.value);
    });
    label.append(name, select);
    target.append(label);
    if (state.baselineExample === "odyssey") {
      const source = document.createElement("a");
      source.className = "example-source";
      source.href = ODYSSEY_SOURCE;
      source.target = "_blank";
      source.rel = "noreferrer";
      source.textContent = "Homer · Odyssey 9.21–28 ↗";
      target.append(source);
    }
  }
  for (const definition of controls) {
    if (definition.only && definition.only !== state.scene) continue;
    if (state.scene === "geometry" && !["fontSize", "geometryOverflow", "alignment", "letterSpacing", "wordSpacing", "smoothing", "motionEnabled", "motionDepth", "motionSpeed"].includes(definition.key)) continue;
    if (state.baselineExample === "odyssey" && definition.key === "smoothing") continue;
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
      input.addEventListener("input", () => {
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
  return {
    ...state.options,
    pathSampled: state.baselineExample === "odyssey",
    example: state.baselineExample,
    pathBounds: state.baselineExample === "odyssey" ? state.portrait?.viewBox : null,
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
  $("stage-summary").textContent = state.scene === "geometry" && !state.response.ok ? state.response.error?.message ?? "Draw a curve" : stageSummary(state.response);
  if (!motionOnly) {
    $("copy-points").disabled = state.scene !== "geometry" || !state.response.ok || !data?.baselines?.[0]?.length;
    $("unit-label").hidden = !state.response.ok || data.kind !== "layout";
    $("inspector-count").textContent = state.response.ok ? data.kind.toUpperCase() : "ERROR";
    inspect(null);
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
  $("scaffold-dependency").textContent = state.files[0]?.content ?? "";
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
  $("file-source").textContent = selected?.content ?? "";
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
    await wasm.default();
    state.engine = wasm;
    state.catalog = wasm.feature_catalog();
    state.graph = new FeatureGraph(state.catalog);
    state.scenes = wasm.scene_catalog();
    $("version-pill").textContent = `v${state.catalog.version}`;
    $("engine-status").hidden = true;
    const [features, docs] = await Promise.allSettled([
      fetch("./data/features.json").then((response) => { if (!response.ok) throw new Error("features.json unavailable"); return response.json(); }),
      fetch("./data/docs.json").then((response) => { if (!response.ok) throw new Error("docs.json unavailable"); return response.json(); }),
    ]);
    if (features.status === "fulfilled") state.descriptions = new Map(features.value.features.map((feature) => [feature.name, feature.description]));
    if (docs.status === "fulfilled") {
      state.docs = new DocsView($("docs-list"), $("docs-detail"), $("docs-search"), docs.value, state.catalog.source_rev);
    } else {
      $("docs-detail").textContent = `Documentation index unavailable: ${docs.reason}`;
    }
    selectScene("core");
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
$("source").addEventListener("input", schedule);
$("show-boxes").addEventListener("change", schedule);
$("show-carets").addEventListener("change", schedule);
$("reset-path").addEventListener("click", () => {
  state.options.path = null;
  schedule();
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
new ResizeObserver(() => stage.redraw()).observe($("stage").parentElement);
initialize();
