import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
import {mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync} from "node:fs";
import {tmpdir} from "node:os";
import {dirname, join, resolve} from "node:path";
import {fileURLToPath} from "node:url";
import {FeatureGraph} from "../playground/web/features.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const probe = join(root, "tools/budget-probe");
const artifact = join(probe, "target/wasm32-unknown-unknown/release/textflow_budget_probe.wasm");
const output = join(root, "playground/web/data/budget.json");
const catalog = JSON.parse(readFileSync(join(root, "playground/web/data/features.json"), "utf8"));
const graph = new FeatureGraph({features: catalog.features.filter((feature) => feature.name !== "default")});

function files(path) {
  if (statSync(path).isDirectory()) return readdirSync(path).sort().flatMap((name) => files(join(path, name)));
  return [path];
}

function sourceHash() {
  const hash = createHash("sha256");
  const sources = ["Cargo.toml", "tools/budget-probe/Cargo.toml", "tools/budget.mjs",
    "playground/web/data/features.json", ...files(join(root, "src")), ...files(join(probe, "src"))];
  for (const source of sources) {
    const absolute = source.startsWith(root) ? source : join(root, source);
    hash.update(absolute.slice(root.length));
    hash.update(readFileSync(absolute));
  }
  return hash.digest("hex");
}

function profiles() {
  const unique = new Map();
  const names = graph.features.map((feature) => feature.name);
  for (let mask = 0; mask < 1 << names.length; mask++) {
    const requested = names.filter((_, index) => mask & (1 << index));
    const active = [...graph.closure(requested)].sort();
    unique.set(active.join(",") || "base", active);
  }
  if (unique.size !== 21) throw new Error(`Expected 21 feature profiles, got ${unique.size}`);
  return [...unique].sort(([left], [right]) => left.localeCompare(right));
}

function run(program, args, options = {}) {
  const result = execFileSync(program, args, {cwd: root, encoding: "utf8", stdio: options.stdio ?? ["ignore", "pipe", "inherit"]});
  return typeof result === "string" ? result.trim() : "";
}

const work = [
  ["core", "run_core", "latin_chars"],
  ["unicode", "run_unicode", "latin_chars"],
  ["bidi", "run_bidi", "bidi_chars"],
  ["shaping", "run_shaping", "latin_chars"],
  ["workspace", "run_workspace", "latin_chars"],
  ["complex-shaping", "run_complex", "latin_chars"],
  ["arabic", "run_arabic", "arabic_chars"],
  ["thai", "run_thai", "thai_chars"],
  ["devanagari", "run_devanagari", "devanagari_chars"],
];

function measure(bytes, meteredPath) {
  const original = new WebAssembly.Instance(new WebAssembly.Module(bytes), {}).exports;
  run("wasm-opt", ["--log-execution", "--no-stack-ir", artifact, "-o", meteredPath]);
  const metered = new WebAssembly.Module(readFileSync(meteredPath));
  const imports = WebAssembly.Module.imports(metered);
  if (imports.length !== 1 || imports[0].name !== "log_execution" || imports[0].kind !== "function") {
    throw new Error(`Unexpected instrumentation imports: ${JSON.stringify(imports)}`);
  }
  let operations = 0;
  const instrumented = new WebAssembly.Instance(metered, {
    [imports[0].module]: {log_execution: () => {operations++;}},
  }).exports;
  const samples = {};
  for (const [name, functionName, charsName] of work) {
    if (typeof original[functionName] !== "function") continue;
    const expected = original[functionName]();
    operations = 0;
    const result = instrumented[functionName]();
    if (result !== expected || result <= 0 || operations <= 0) {
      throw new Error(`Invalid ${name} reference workload: ${result}, expected ${expected}, ${operations} events`);
    }
    samples[name] = {inputChars: original[charsName](), meteredOperations: operations};
  }
  return {initialPages: original.memory.buffer.byteLength / 65_536, work: samples};
}

const expected = {schema: 1, sourceHash: sourceHash(), crateVersion: catalog.crate.version};
if (process.argv[2] === "--check") {
  const current = JSON.parse(readFileSync(output, "utf8"));
  if (current.schema !== expected.schema || current.sourceHash !== expected.sourceHash
    || current.crateVersion !== expected.crateVersion || Object.keys(current.profiles).length !== profiles().length) {
    throw new Error("WASM budget data is stale; run cargo xtask budget");
  }
  console.log("WASM budget data is current");
} else if (process.argv.length === 2) {
  const temporary = mkdtempSync(join(tmpdir(), "textflow-budget-"));
  try {
    const result = {...expected, toolchain: {
      rustc: run("rustc", ["--version"]),
      binaryen: run("wasm-opt", ["--version"]),
      target: "wasm32-unknown-unknown",
      optimization: "opt-level=z, lto=true, panic=abort",
      meter: "Binaryen --log-execution callback events",
    }, profiles: {}};
    for (const [key, active] of profiles()) {
      const args = ["build", "--manifest-path", join(probe, "Cargo.toml"), "--target", "wasm32-unknown-unknown", "--release", "--offline", "--no-default-features"];
      if (active.length) args.push("--features", active.join(","));
      run("cargo", args, {stdio: "inherit"});
      const bytes = readFileSync(artifact);
      result.profiles[key] = {features: active, wasmBytes: bytes.length, ...measure(bytes, join(temporary, "metered.wasm"))};
      console.log(`${String(Object.keys(result.profiles).length).padStart(2)}/21  ${key}  ${bytes.length} B`);
    }
    writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
    console.log(`wrote ${output}`);
  } finally {
    rmSync(temporary, {recursive: true, force: true});
  }
} else {
  throw new Error("Usage: node tools/budget.mjs [--check]");
}
