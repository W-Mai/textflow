import {readFileSync} from "node:fs";
import init, {analyze_scene, feature_catalog, scene_catalog, scaffold_files, zip_files} from "./pkg/textflow_playground.js";
import {FeatureGraph} from "./features.js";
import {stageSummary} from "./stage.js";
import {rustTokens} from "./rust-highlight.js";

const snippet = 'let value = TextFlow::new("<tag>", 12); // safe\n';
const tokens = rustTokens(snippet);
if (tokens.map((token) => token.text).join("") !== snippet) throw new Error("Rust highlighting changed source text");
for (const kind of ["keyword", "type", "string", "number", "comment"]) {
  if (!tokens.some((token) => token.kind === kind)) throw new Error(`Missing Rust token kind: ${kind}`);
}

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
const scaffold = scaffold_files(["script-devanagari", "alloc"]);
if (!scaffold.ok || scaffold.files.length !== 3) throw new Error("Scaffold failed");
const zip = zip_files(scaffold.files);
if (!(zip instanceof Uint8Array) || zip[0] !== 0x50 || zip[1] !== 0x4b) throw new Error("ZIP failed");
console.log(`${scenes.length} real WASM scenes, ${features.features.length} features, scaffold and ZIP passed`);
