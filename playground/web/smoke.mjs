import {readFileSync} from "node:fs";
import init, {analyze_scene, feature_catalog, scene_catalog, scaffold_files, zip_files} from "./pkg/textflow_playground.js";
import {FeatureGraph} from "./features.js";
import {Stage, VIEWPORT_WIDTH, clipClusters, containsHitbox, stageSummary, viewportWidthAt} from "./stage.js";
import {rustTokens, tomlTokens, markdownTokens} from "./code-highlight.js";
import {BudgetView, profileKey, referenceBudget} from "./budget.js";
import {ODYSSEY_TEXT, ODYSSEY_VERSE, samplePath} from "./baseline-examples.js";
import {formatBaselinePoints} from "./baseline-code.js";
import {buildTiles, glyphTarget, revealPulse, stepGlyph} from "./atmosphere.js";

const page = readFileSync(new URL("./index.html", import.meta.url), "utf8");
if (page.indexOf('id="baseline-examples"') < 0
  || page.indexOf('id="baseline-examples"') > page.indexOf('class="surface code-surface"')
  || !page.includes('id="example-draw"') || !page.includes('id="example-yuuu"')
  || (page.match(/class="example-logo /g) ?? []).length !== 2) {
  throw new Error("Baseline example buttons are missing above the Rust API");
}
const atmosphere = page.match(/<canvas class="atmosphere"[^>]*><\/canvas>/)?.[0];
if (!atmosphere?.includes('aria-hidden="true"')
  || page.indexOf(atmosphere) > page.indexOf("<header class=\"site-header\"")) {
  throw new Error("Interactive typography is not a page-level canvas");
}
const overlayStyle = readFileSync(new URL("./terminal.css", import.meta.url), "utf8")
  .match(/\.atmosphere\s*\{([^}]+)\}/)?.[1];
if (!overlayStyle?.includes("z-index: 0") || !overlayStyle.includes("pointer-events: none")) {
  throw new Error("Atmospheric text is not behind the page content");
}
const effectSource = readFileSync(new URL("./atmosphere.js", import.meta.url), "utf8");
if (/textPath|flowPath|bezierCurve|quadraticCurve|\.arc\(/.test(effectSource)
  || /textPath|rights-path/.test(page)) {
  throw new Error("Curved text returned to the page effect");
}
for (const [width, height] of [[320, 640], [1280, 720], [2560, 1440]]) {
  const tiles = buildTiles(width, height);
  if (tiles.length < width * height / (180 * 74)
    || !tiles.some((tile) => tile.x < 0) || !tiles.some((tile) => tile.x > width)
    || !tiles.some((tile) => tile.y < 0) || !tiles.some((tile) => tile.y > height)
    || tiles.some((tile) => !tile.text || !Number.isFinite(tile.x) || !Number.isFinite(tile.y))) {
    throw new Error("Interactive typography does not cover the viewport");
  }
  const tile = tiles.find((candidate) => candidate.x > 0 && candidate.x < width) ?? tiles[0];
  const glyph = {centerX: tile.x, centerY: tile.y, index: tile.index, dx: 0, dy: 0, vx: 0, vy: 0};
  const near = glyphTarget(glyph, {x: glyph.centerX, y: glyph.centerY, strength: 1});
  const far = glyphTarget(glyph, {x: glyph.centerX + 400, y: glyph.centerY, strength: 1});
  const idle = glyphTarget(glyph, {x: glyph.centerX, y: glyph.centerY, strength: 0});
  if (near.proximity !== 1 || far.proximity !== 0 || idle.x !== 0 || idle.y !== 0
    || Math.hypot(near.x, near.y) > 96 || Math.hypot(near.x, near.y) < 10) {
    throw new Error("Typography glyphs no longer retreat within their motion budget");
  }
  for (let step = 0; step < 20; step++) stepGlyph(glyph, near);
  if (Math.hypot(glyph.dx, glyph.dy) < 25) {
    throw new Error("The pointer does not push individual glyphs");
  }
  for (let step = 0; step < 100; step++) stepGlyph(glyph, idle);
  if (glyph.dx !== 0 || glyph.dy !== 0) {
    throw new Error("Pushed glyphs do not return to their original positions");
  }
}
const glyphAfterSecond = (fps) => {
  const glyph = {dx: 0, dy: 0, vx: 0, vy: 0};
  for (let frame = 0; frame < fps; frame++) stepGlyph(glyph, {x: 60, y: -30, proximity: 1}, 24 / fps);
  return glyph;
};
const referenceGlyph = glyphAfterSecond(24);
for (const fps of [60, 120]) {
  const glyph = glyphAfterSecond(fps);
  if (Math.hypot(glyph.dx - referenceGlyph.dx, glyph.dy - referenceGlyph.dy) > 2) {
    throw new Error(`Typography motion changed speed at ${fps} FPS`);
  }
}
if (revealPulse(0) !== 0 || revealPulse(16) !== 1 || revealPulse(23) !== 0) {
  throw new Error("Typography highlight changed its bounded cycle");
}

const bidiText = "مرحبا · Gate 4 · إلى القاهرة";
const bidiRuns = {
  direction: "rtl",
  logical: [{range: [0, 14], direction: "rtl", level: 1},
    {range: [14, 20], direction: "ltr", level: 2},
    {range: [20, 45], direction: "rtl", level: 1}],
};
bidiRuns.visual = [...bidiRuns.logical].reverse();
for (const height of [200, 375]) {
  const ctx = new Proxy({}, {get: () => () => {}, set: () => true});
  const stage = {canvas: {clientHeight: height}, hitboxes: [],
    palette: {muted: "#888", line: "#777", amber: "#f80", cell: "#222", text: "#fff", runs: ["#0f0", "#0ff", "#f0f"]}};
  Stage.prototype.bidi.call(stage, ctx, 600, bidiRuns, bidiText);
  if (stage.hitboxes.length !== 6 || stage.hitboxes.slice(3).some((box) => box.y + box.height > height - 5)) {
    throw new Error("Bidi screen order is clipped by a short desktop stage");
  }
}

const pointCode = formatBaselinePoints([[12, -8], [80, 24]]);
if (!pointCode.includes("const POINTS: &[FlowPoint]") || !pointCode.includes("FlowPoint { x: 12, y: -8 }")
  || pointCode !== formatBaselinePoints([[12, -8], [80, 24]])) {
  throw new Error("Baseline point export changed");
}
for (const invalid of [[], [[1, 2]], [[1, 2], [NaN, 3]], [[1, 2], [2147483648, 3]]]) {
  let rejected = false;
  try { formatBaselinePoints(invalid); }
  catch { rejected = true; }
  if (!rejected) throw new Error("Invalid baseline points were exported");
}

const sampled = samplePath({
  getTotalLength: () => 200,
  getPointAtLength: (length) => ({x: length, y: 25}),
}, 4);
if (JSON.stringify(sampled) !== JSON.stringify([[0, 25], [50, 25], [100, 25], [150, 25], [200, 25]])) {
  throw new Error("SVG path sampling changed route order");
}
const fullPath = samplePath({
  getTotalLength: () => 1024,
  getPointAtLength: (length) => ({x: length, y: 0}),
});
if (fullPath.length !== 1025 || fullPath.at(-1)[0] !== 1024) {
  throw new Error("Portrait sampling dropped path sections");
}
let rejected = false;
try { samplePath({getTotalLength: () => 0}, 4); }
catch { rejected = true; }
if (!rejected) throw new Error("Degenerate SVG path was accepted");
const portraitProjection = Stage.prototype.projection(600, 400, {
  fontSize: 16, example: "yuuu", pathBounds: [25, 12, 203, 224],
}, true);
if (portraitProjection.fit <= 1 || portraitProjection.y0 + 12 * portraitProjection.fit <= 0) {
  throw new Error("Portrait path did not fit the stage");
}
const fontSize = 14;
const drawProjection = Stage.prototype.projection(600, 400, {fontSize, example: "draw"}, true);
const yuuuProjection = Stage.prototype.projection(600, 400, {
  fontSize, example: "yuuu", pathBounds: [25, 12, 203, 224],
}, true);
if (drawProjection.fit === yuuuProjection.fit
  || drawProjection.scale !== fontSize / 1000
  || yuuuProjection.scale !== fontSize / 1000) {
  throw new Error("Baseline examples changed the 14 px font scale");
}

const snippet = 'let value = TextFlow::new("<tag>", 12); // safe\n';
const tokens = rustTokens(snippet);
if (tokens.map((token) => token.text).join("") !== snippet) throw new Error("Rust highlighting changed source text");
for (const kind of ["keyword", "type", "string", "number", "comment"]) {
  if (!tokens.some((token) => token.kind === kind)) throw new Error(`Missing Rust token kind: ${kind}`);
}
for (const [source, lexer, kinds] of [
  ['[dependencies]\ntextflow-rs = { version = "1.2.3", features = [] }\n', tomlTokens, ["section", "key", "string"]],
  ['# TextFlow demo\nRun `cargo run` with **features**.\n', markdownTokens, ["heading", "headingText", "code", "emphasis"]],
]) {
  const highlighted = lexer(source);
  if (highlighted.map((token) => token.text).join("") !== source || kinds.some((kind) => !highlighted.some((token) => token.kind === kind))) {
    throw new Error("Scaffold syntax highlighting changed source or missed a token kind");
  }
}

const layout = {
  geometry: false,
  lines: [{origin: [0, 0]}],
  runs: [{glyphs: [0, 1]}],
  glyphs: [{origin: [0, 0], offset: [0, 0], advance: [2000, 0], character: "A"}],
  carets: [],
};
function geometryFont(example, showCurve = false) {
  const fonts = [];
  let curves = 0;
  const ctx = new Proxy({}, {
    get: (_, method) => method === "measureText"
      ? () => ({actualBoundingBoxLeft: 0, actualBoundingBoxRight: 8, actualBoundingBoxAscent: 10, actualBoundingBoxDescent: 3})
      : () => {},
    set: (_, property, value) => {
      if (property === "font") fonts.push(value);
      return true;
    },
  });
  const stage = {
    projection: Stage.prototype.projection,
    canvas: {clientHeight: 400},
    palette: {line: "#888", muted: "#888", runs: ["#888"]},
    hitboxes: [],
    curve: () => {curves++;},
  };
  Stage.prototype.layout.call(stage, ctx, 600, {...layout, geometry: true, baselines: [[[0, 0], [100, 0]]]}, {
    fontSize: 14, example, showCurve, pathBounds: [25, 12, 203, 224], overflow: "ellipsis",
  }, {boxes: false, carets: false});
  return {font: fonts.at(-1), curves};
}
const freeDraw = geometryFont("draw");
const visibleCurve = geometryFont("draw", true);
const yuuuCurve = geometryFont("yuuu", true);
if (!freeDraw.font?.startsWith("14px ") || !yuuuCurve.font?.startsWith("14px ")) {
  throw new Error("Baseline examples rendered different font sizes at 14 px");
}
if (freeDraw.curves !== 0 || visibleCurve.curves !== 1 || yuuuCurve.curves !== 0) {
  throw new Error("The Free draw curve visibility control changed another baseline example");
}
let measurements = 0;
const metricsContext = new Proxy({}, {
  get: (_, method) => method === "measureText"
    ? () => { measurements++; return {actualBoundingBoxLeft: 0, actualBoundingBoxRight: 8, actualBoundingBoxAscent: 10, actualBoundingBoxDescent: 3}; }
    : () => {},
  set: () => true,
});
const metricsStage = {projection: Stage.prototype.projection, canvas: {clientHeight: 200},
  palette: {line: "#888", muted: "#888", runs: ["#888"]}, hitboxes: []};
for (const fontSize of [32, 32, 40]) {
  Stage.prototype.layout.call(metricsStage, metricsContext, 600, layout,
    {fontSize, width: 600, overflow: "ellipsis"}, {boxes: false, carets: false});
}
if (measurements !== 2) throw new Error("Repeated frames measured the same glyph again");
const clipLayout = {
  ...layout,
  runs: [{glyphs: [0, 2]}],
  glyphs: [
    {...layout.glyphs[0], cluster: [0, 1], advance: [600, 0]},
    {...layout.glyphs[0], character: "B", cluster: [1, 2], origin: [600, 0], advance: [600, 0]},
  ],
};
if (clipClusters(clipLayout.glyphs, [{left: 29, right: 48}, {left: 48, right: 68}], 29, 61).join() !== "true,false"
  || clipClusters([{cluster: [0, 1]}, {cluster: [0, 1]}],
    [{left: 29, right: 48}, {left: 48, right: 68}], 29, 61).some(Boolean)) {
  throw new Error("Clip did not preserve complete text clusters");
}
function paint(overflow) {
  const calls = [];
  const ctx = new Proxy({}, {
    get: (_, method) => method === "measureText"
      ? () => ({actualBoundingBoxLeft: 0, actualBoundingBoxRight: 16, actualBoundingBoxAscent: 10, actualBoundingBoxDescent: 3})
      : (...args) => calls.push([method, ...args]),
    set: () => true,
  });
  const stage = {
    projection: Stage.prototype.projection,
    canvas: {clientHeight: 200},
    palette: {line: "#888", muted: "#888", runs: ["#888"]},
    hitboxes: [],
  };
  Stage.prototype.layout.call(stage, ctx, 600, clipLayout, {fontSize: 32, width: 32, overflow}, {boxes: false, carets: false});
  return {calls, hitboxes: stage.hitboxes, visibleGlyphs: stage.visibleGlyphs};
}
const clipped = paint("clip");
if (!clipped.calls.some(([method]) => method === "clip")) throw new Error("Clip did not constrain canvas paint");
if (clipped.visibleGlyphs !== 1 || clipped.hitboxes.length !== 1 || clipped.hitboxes[0].x !== 29
  || clipped.calls.some(([method, text]) => method === "fillText" && text === "B")) {
  throw new Error("Clip painted or hit-tested a partial glyph");
}
if (stageSummary({ok: true, data: {...clipLayout, kind: "layout"}}, clipped.visibleGlyphs) !== "1 / 2 GLYPHS · 1 LINE") {
  throw new Error("Clip summary hides the visible glyph count");
}
const ellipsized = paint("ellipsis");
if (ellipsized.calls.some(([method]) => method === "clip")
  || !ellipsized.calls.some(([method, text]) => method === "fillText" && text === "B")) {
  throw new Error("Ellipsis was clipped like the clip mode");
}
const guideCalls = [];
const guideContext = new Proxy({}, {
  get: (_, method) => method === "measureText" ? () => ({width: 76})
    : (...args) => guideCalls.push([method, ...args]),
  set: () => true,
});
Stage.prototype.widthGuide.call({palette: {amber: "#f80", bg: "#111"}},
  guideContext, 600, 200, 509, "480 PX");
if (!guideCalls.some(([method, x, y]) => method === "moveTo" && x === 509.5 && y === 52)
  || !guideCalls.some(([method, x, y]) => method === "lineTo" && x === 509.5 && y === 182)) {
  throw new Error("Viewport guide does not align to the layout boundary");
}
for (const canvasWidth of [320, 600, 1280]) {
  const projection = Stage.prototype.projection(canvasWidth, 400, {fontSize: 32, width: 480}, false);
  let previous = -Infinity;
  for (let width = VIEWPORT_WIDTH.min; width <= VIEWPORT_WIDTH.max; width += VIEWPORT_WIDTH.step) {
    const x = projection.x0 + width * projection.fit;
    if (x <= previous || viewportWidthAt(x, projection) !== width || x > canvasWidth - 44) {
      throw new Error("Viewport guide is not draggable across the full width range");
    }
    previous = x;
  }
  if (viewportWidthAt(-100, projection) !== VIEWPORT_WIDTH.min
    || viewportWidthAt(canvasWidth + 100, projection) !== VIEWPORT_WIDTH.max) {
    throw new Error("Viewport drag escaped the width control limits");
  }
}
const coreCalls = [];
const coreContext = new Proxy({}, {
  get: (_, method) => (...args) => coreCalls.push([method, ...args]),
  set: () => true,
});
const coreStage = {canvas: {clientHeight: 200},
  palette: {text: "#fff", line: "#888", amber: "#f80", muted: "#aaa"}, hitboxes: []};
const coreOptions = {width: 480, fontSize: 32};
const coreProjection = Stage.prototype.projection(600, 400, coreOptions, false);
Stage.prototype.core.call(coreStage, coreContext, 600, {lines: [{width: 24, text: "A small boat"}]}, coreOptions, coreProjection);
const coreBars = coreCalls.filter(([method, , y, , height]) => method === "fillRect" && y === 97 && height === 4);
if (coreBars.length !== 2 || Math.abs(coreBars[0][3] - coreOptions.width * coreProjection.fit) > 0.001
  || Math.abs(coreBars[1][3] - 24 * VIEWPORT_WIDTH.step * coreProjection.fit) > 0.001) {
  throw new Error("Line flow width bars do not share the viewport guide scale");
}
coreCalls.length = 0;
coreStage.hitboxes = [];
const coreLine = {width: 8, text: "came on."};
Stage.prototype.core.call(coreStage, coreContext, 600, {lines: Array(4).fill(coreLine)}, coreOptions, coreProjection);
if (coreStage.hitboxes.length !== 4
  || !coreCalls.some(([method, , y, , height]) => method === "fillRect" && y === 184 && height === 4)) {
  throw new Error("Four Line flow widths did not fit the output canvas");
}
coreCalls.length = 0;
coreStage.hitboxes = [];
Stage.prototype.core.call(coreStage, coreContext, 600, {lines: Array(8).fill(coreLine)}, coreOptions, coreProjection);
if (coreStage.hitboxes.length !== 3
  || !coreCalls.some(([method, label]) => method === "fillText" && label === "+ 5 MORE LINES")) {
  throw new Error("Overflowing Line flow rows are not accounted for");
}
const listeners = new Map();
const dragCanvas = {
  style: {},
  addEventListener: (name, callback) => listeners.set(name, callback),
  getBoundingClientRect: () => ({left: 100, top: 50, width: 600, height: 400}),
  setPointerCapture: () => {},
};
const dragWidths = [];
const dragStage = new Stage(dragCanvas, () => {}, () => {}, (width) => dragWidths.push(width));
dragStage.last = [{ok: true, data: {kind: "core"}}, "", coreOptions, {}];
const guideX = coreProjection.x0 + coreOptions.width * coreProjection.fit;
listeners.get("pointerdown")({button: 0, pointerId: 1, clientX: 100 + guideX, clientY: 150, preventDefault() {}});
listeners.get("pointermove")({pointerId: 1, clientX: 100 + coreProjection.x0 + 320 * coreProjection.fit, clientY: 150});
listeners.get("pointerup")({pointerId: 1, clientX: 100 + coreProjection.x0 + 320 * coreProjection.fit, clientY: 150});
if (dragWidths.join() !== "480,320,320" || dragStage.widthDragging) {
  throw new Error("Dragging the guide did not update viewport width");
}

const oriented = {origin: [100, 100], angle: Math.PI / 2, local: [0, -10, 20, 0]};
if (!containsHitbox(oriented, 105, 110) || containsHitbox(oriented, 120, 100)) throw new Error("Oriented hit testing is incorrect");
const calls = [];
const rotatedContext = new Proxy({}, {
  get: (_, method) => method === "measureText"
    ? () => ({actualBoundingBoxLeft: 1, actualBoundingBoxRight: 18, actualBoundingBoxAscent: 20, actualBoundingBoxDescent: 4})
    : (...args) => calls.push([method, ...args]),
  set: () => true,
});
const rotatedStage = {
  projection: Stage.prototype.projection,
  canvas: {clientHeight: 200},
  palette: {line: "#888", muted: "#888", runs: ["#888"]},
  hitboxes: [],
};
Stage.prototype.layout.call(rotatedStage, rotatedContext, 600, {
  ...layout, geometry: true,
  glyphs: [{...layout.glyphs[0], frame: {origin: [100, 100], tangent: [0, 256]}}],
}, {fontSize: 32, width: 600, overflow: "ellipsis"}, {boxes: true, carets: false});
const rotation = calls.findIndex(([method]) => method === "rotate");
const box = calls.findIndex(([method]) => method === "rect");
const ink = calls.findIndex(([method], index) => index > box && method === "fillText");
if (!(rotation >= 0 && box > rotation && ink > box)) throw new Error("Bounds did not share the glyph transform");
if (Math.abs(rotatedStage.hitboxes[0].angle - Math.PI / 2) > 0.001) throw new Error("Glyph hit region did not rotate");

await init({module_or_path: readFileSync(new URL("./pkg/textflow_playground_bg.wasm", import.meta.url))});
const features = feature_catalog();
const scenes = scene_catalog();
const budget = JSON.parse(readFileSync(new URL("./data/budget.json", import.meta.url), "utf8"));
if (budget.crateVersion !== features.version || Object.keys(budget.profiles).length !== 21) {
  throw new Error("WASM reference budget does not match the crate feature graph");
}
const budgetGraph = new FeatureGraph(features);
const names = budgetGraph.features.map((feature) => feature.name);
for (let mask = 0; mask < 1 << names.length; mask++) {
  const selected = names.filter((_, index) => mask & (1 << index));
  const active = [...budgetGraph.closure(selected)].sort();
  const profile = budget.profiles[profileKey(active)];
  if (!profile || profile.features.join(",") !== active.join(",")
    || profile.wasmBytes < 1024 || profile.initialPages < 1 || !profile.work.core?.meteredOperations) {
    throw new Error(`Missing WASM budget for ${active.join(",")}`);
  }
}
budgetGraph.enable("script-arabic");
const inspected = referenceBudget(budget, budgetGraph, "arabic", "مرحبا", {
  ok: true, data: {kind: "layout", outputBytes: 4096, privateBytes: null},
}, {buffer: {byteLength: 1_310_720}});
if (inspected.inputBytes !== 10 || inspected.outputBytes !== 4096
  || inspected.loadedBytes !== 1_310_720 || inspected.impact.length !== 1
  || inspected.impact[0].bytes == null || !inspected.work?.meteredOperations) {
  throw new Error("WASM budget lost a live-memory or selected-feature measurement");
}
const sparse = referenceBudget({...budget, profiles: {base: budget.profiles.base}}, budgetGraph, "arabic", "مرحبا", null, null);
if (sparse.profile !== undefined || sparse.loadedBytes !== null || sparse.impact[0].bytes !== null) {
  throw new Error("Unavailable WASM reference data must remain unknown");
}
const cancelled = Object.create(BudgetView.prototype);
cancelled.snapshot = {scene: "core", text: "Hello", options: {}, response: {ok: true}};
cancelled.revision = 1;
cancelled.measuring = false;
cancelled.button = {disabled: false, textContent: ""};
let measurementCalls = 0;
cancelled.engine = {analyze_scene: () => {
  if (++measurementCalls === 1) setTimeout(() => cancelled.revision++, 0);
  return {ok: true};
}};
cancelled.update = () => {};
globalThis.document = {hidden: false};
await cancelled.measure();
delete globalThis.document;
if (measurementCalls !== 2 || cancelled.speed != null || cancelled.measuring) {
  throw new Error("An invalidated throughput measurement must be discarded");
}
if (!features.features.some((feature) => feature.name === "script-devanagari")) throw new Error("Devanagari feature missing");
const ordered = new FeatureGraph(features).features.map((feature) => feature.name);
for (const feature of features.features) {
  for (const required of feature.requires) {
    if (ordered.indexOf(required) >= ordered.indexOf(feature.name)) throw new Error(`${required} should precede ${feature.name}`);
  }
}
for (const scene of scenes) {
  const result = analyze_scene(scene.id, scene.sample, {width: 480, fontSize: 32});
  if (!result.ok || !result.data) throw new Error(`${scene.id}: ${JSON.stringify(result.error)}`);
  if (!stageSummary(result)) throw new Error(`${scene.id}: summary missing`);
}
const bidiScene = scenes.find((scene) => scene.id === "bidi");
const bidiExample = analyze_scene("bidi", bidiScene.sample, {direction: "auto"});
const screenOrder = bidiExample.data.visual.map((run) => bidiExample.data.logical.findIndex((source) =>
  source.range[0] === run.range[0] && source.range[1] === run.range[1]) + 1);
if (bidiExample.data.direction !== "rtl" || screenOrder.join() !== "3,2,1") {
  throw new Error("Bidi example does not visibly reorder source runs");
}
const greek = analyze_scene("geometry", ODYSSEY_TEXT, {
  fontSize: 16, path: sampled, pathSampled: true, smoothing: 0, motionDepth: 0, overflow: "clip",
});
if (!greek.ok || !greek.data.glyphs.length || greek.data.baselines[0].length !== sampled.length) {
  throw new Error(`Odyssey example failed: ${JSON.stringify(greek.error)}`);
}
const longPath = samplePath({
  getTotalLength: () => 6000,
  getPointAtLength: (length) => ({x: length, y: 0}),
});
const longOptions = {
  fontSize: 16, path: longPath, pathSampled: true, smoothing: 0, motionDepth: 0, overflow: "clip",
};
const singleVerse = analyze_scene("geometry", ODYSSEY_VERSE, longOptions);
const repeatedVerse = analyze_scene("geometry", ODYSSEY_TEXT, longOptions);
if (!singleVerse.ok || !repeatedVerse.ok || repeatedVerse.data.glyphs.length <= singleVerse.data.glyphs.length) {
  throw new Error("Repeated Odyssey text did not extend along the full path");
}
const routeEnd = repeatedVerse.data.baselines[0].at(-1)[0];
const finalGlyph = repeatedVerse.data.glyphs.at(-1).frame.origin[0];
if (routeEnd - finalGlyph > 12500 || finalGlyph > routeEnd) {
  throw new Error(`Odyssey text stopped before the path ended: ${routeEnd - finalGlyph} units`);
}
const scaffold = scaffold_files(["script-devanagari", "alloc"]);
if (!scaffold.ok || scaffold.files.length !== 3) throw new Error("Scaffold failed");
const zip = zip_files(scaffold.files);
if (!(zip instanceof Uint8Array) || zip[0] !== 0x50 || zip[1] !== 0x4b) throw new Error("ZIP failed");
console.log(`${scenes.length} real WASM scenes, ${features.features.length} features, scaffold and ZIP passed`);
