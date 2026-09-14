import {readFileSync} from "node:fs";
import init, {analyze_scene, feature_catalog, scene_catalog, scaffold_files, zip_files} from "./pkg/textflow_playground.js";
import {FeatureGraph} from "./features.js";
import {Stage, containsHitbox, stageSummary} from "./stage.js";
import {rustTokens} from "./rust-highlight.js";
import {ODYSSEY_TEXT, ODYSSEY_VERSE, samplePath} from "./baseline-examples.js";
import {formatBaselinePoints} from "./baseline-code.js";

const page = readFileSync(new URL("./index.html", import.meta.url), "utf8");
if (page.indexOf('id="baseline-examples"') < 0
  || page.indexOf('id="baseline-examples"') > page.indexOf('class="surface code-surface"')
  || !page.includes('id="example-draw"') || !page.includes('id="example-yuuu"')
  || (page.match(/class="example-logo /g) ?? []).length !== 2) {
  throw new Error("Baseline example buttons are missing above the Rust API");
}
const atmosphere = page.match(/<svg class="atmosphere"[\s\S]*?<\/svg>/)?.[0];
if (!atmosphere?.includes('aria-hidden="true"')
  || page.indexOf(atmosphere) > page.indexOf("<header class=\"site-header\"")) {
  throw new Error("Atmospheric text is not a page-level background");
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

const layout = {
  geometry: false,
  lines: [{origin: [0, 0]}],
  runs: [{glyphs: [0, 1]}],
  glyphs: [{origin: [0, 0], offset: [0, 0], advance: [2000, 0], character: "A"}],
  carets: [],
};
function geometryFont(example) {
  const fonts = [];
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
  };
  Stage.prototype.layout.call(stage, ctx, 600, {...layout, geometry: true}, {
    fontSize: 14, example, pathBounds: [25, 12, 203, 224], overflow: "ellipsis",
  }, {boxes: false, carets: false});
  return fonts.at(-1);
}
if (!geometryFont("draw")?.startsWith("14px ") || !geometryFont("yuuu")?.startsWith("14px ")) {
  throw new Error("Baseline examples rendered different font sizes at 14 px");
}
function paint(overflow) {
  const calls = [];
  const ctx = new Proxy({}, {
    get: (_, method) => (...args) => calls.push([method, ...args]),
    set: () => true,
  });
  const stage = {
    projection: Stage.prototype.projection,
    canvas: {clientHeight: 200},
    palette: {line: "#888", muted: "#888", runs: ["#888"]},
    hitboxes: [],
  };
  Stage.prototype.layout.call(stage, ctx, 600, layout, {fontSize: 32, width: 32, overflow}, {boxes: false, carets: false});
  return {calls, hitboxes: stage.hitboxes};
}
const clipped = paint("clip");
if (!clipped.calls.some(([method]) => method === "clip")) throw new Error("Clip did not constrain canvas paint");
if (clipped.hitboxes[0].x !== 29 || clipped.hitboxes[0].width !== 32) throw new Error("Clip did not constrain hit testing");
if (paint("ellipsis").calls.some(([method]) => method === "clip")) throw new Error("Ellipsis was clipped twice");

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
