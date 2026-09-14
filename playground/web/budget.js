const number = new Intl.NumberFormat("en-US");
const bytes = (value) => value == null ? "—" : value < 1024 ? `${number.format(value)} B` : value < 1024 * 1024 ? `${(value / 1024).toFixed(1)} KiB` : `${(value / 1048576).toFixed(2)} MiB`;

export function profileKey(features) {
  return [...features].sort().join(",") || "base";
}

export function referenceBudget(manifest, graph, scene, text, response, memory) {
  const selected = graph.selected();
  const profile = manifest.profiles[profileKey(graph.closure())];
  const base = manifest.profiles.base;
  const impact = selected.map((name) => {
    const without = manifest.profiles[profileKey(graph.closure(selected.filter((item) => item !== name)))];
    return {name, bytes: profile && without ? profile.wasmBytes - without.wasmBytes : null};
  });
  const data = response?.ok ? response.data : null;
  return {
    profile,
    delta: profile && base ? profile.wasmBytes - base.wasmBytes : null,
    impact,
    inputBytes: new TextEncoder().encode(text).length,
    outputBytes: data?.kind === "layout" ? data.outputBytes : null,
    privateBytes: data?.kind === "layout" ? data.privateBytes : null,
    loadedBytes: memory?.buffer.byteLength ?? null,
    work: profile?.work[scene] ?? null,
  };
}

export class BudgetView {
  constructor(root, manifest, engine, memory) {
    this.root = root;
    this.manifest = manifest;
    this.engine = engine;
    this.memory = memory;
    this.button = root.querySelector("#budget-measure");
    this.button.addEventListener("click", () => this.measure());
    document.addEventListener("visibilitychange", () => {
      if (this.snapshot) this.update(this.graph, this.snapshot.scene, this.snapshot.text, this.snapshot.options, this.snapshot.response);
    });
    this.revision = 0;
    this.key = null;
    this.speed = null;
    this.measuring = false;
  }

  set(id, value) {
    this.root.querySelector(`#budget-${id}`).textContent = value;
  }

  update(graph, scene, text, options, response) {
    this.graph = graph;
    const key = JSON.stringify([profileKey(graph.closure()), scene, text, options]);
    if (key !== this.key) {
      this.key = key;
      this.revision++;
      this.speed = null;
    }
    this.snapshot = {scene, text, options, response};
    this.data = referenceBudget(this.manifest, graph, scene, text, response, this.memory);
    const {profile, delta, impact, inputBytes, outputBytes, privateBytes, loadedBytes, work} = this.data;
    this.set("summary", profile ? bytes(profile.wasmBytes) : "—");
    this.set("binary", profile ? `${delta < 0 ? "−" : "+"}${bytes(Math.abs(delta))}` : "not measured");
    this.set("initial", profile ? bytes(profile.initialPages * 65_536) : "not measured");
    this.set("loaded", bytes(loadedBytes));
    this.set("input", bytes(inputBytes));
    this.set("output", bytes(outputBytes));
    this.set("private", bytes(privateBytes));
    this.set("work", work ? `${number.format(work.meteredOperations)} / ${work.inputChars} chars` : "not measured");
    this.set("speed", this.speed ? `${number.format(this.speed)} chars/s` : "—");
    const target = this.root.querySelector("#budget-impact");
    target.replaceChildren();
    for (const item of impact) {
      const row = document.createElement("div");
      const name = document.createElement("span");
      name.textContent = item.name;
      const cost = document.createElement("span");
      cost.textContent = item.bytes == null ? "—" : `${item.bytes < 0 ? "−" : "+"}${bytes(Math.abs(item.bytes))}`;
      row.append(name, cost);
      target.append(row);
    }
    this.button.disabled = this.measuring || !response?.ok || !text || document.hidden;
    this.button.textContent = this.measuring ? "MEASURING…" : "MEASURE CURRENT TEXT ↗";
    this.set("note", `Reference v${this.manifest.crateVersion} · binary size approximates Flash. Work counts instrumented WASM events, not CPU instructions. Speed includes the WASM bridge.`);
  }

  async measure() {
    if (this.measuring || !this.snapshot?.response?.ok || !this.snapshot.text || document.hidden) return;
    const {scene, text, options} = this.snapshot;
    const revision = this.revision;
    this.measuring = true;
    this.button.disabled = true;
    this.button.textContent = "MEASURING…";
    try {
      for (let warmup = 0; warmup < 2; warmup++) {
        if (!this.engine.analyze_scene(scene, text, options).ok) throw new Error("Layout unavailable");
      }
      const samples = [];
      for (let sample = 0; sample < 3; sample++) {
        await new Promise((resolve) => setTimeout(resolve, 0));
        if (this.revision !== revision || document.hidden) return;
        const start = performance.now();
        let calls = 0;
        do {
          if (!this.engine.analyze_scene(scene, text, options).ok) throw new Error("Layout unavailable");
          calls++;
        } while (calls < 128 && performance.now() - start < 32);
        const elapsed = performance.now() - start;
        samples.push(Math.round(calls * Array.from(text).length * 1000 / elapsed));
      }
      if (this.revision === revision) this.speed = samples.sort((a, b) => a - b)[1];
    } catch {
      if (this.revision === revision) this.speed = null;
    } finally {
      this.measuring = false;
      if (this.snapshot) this.update(this.graph, this.snapshot.scene, this.snapshot.text, this.snapshot.options, this.snapshot.response);
    }
  }
}
