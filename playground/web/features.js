export class FeatureGraph {
  constructor(catalog) {
    this.byName = new Map(catalog.features.map((feature) => [feature.name, feature]));
    const depths = new Map();
    const depth = (name, path = new Set()) => {
      if (depths.has(name)) return depths.get(name);
      if (path.has(name)) throw new Error(`Feature dependency cycle at ${name}`);
      path.add(name);
      const requires = this.byName.get(name)?.requires ?? [];
      const value = requires.length ? 1 + Math.max(...requires.map((item) => depth(item, path))) : 0;
      path.delete(name);
      depths.set(name, value);
      return value;
    };
    this.features = [...catalog.features].sort((a, b) => depth(a.name) - depth(b.name) || a.name.localeCompare(b.name));
    this.explicit = new Set();
  }

  closure(names = this.explicit) {
    const enabled = new Set();
    const visit = (name) => {
      if (enabled.has(name)) return;
      enabled.add(name);
      for (const dependency of this.byName.get(name)?.requires ?? []) visit(dependency);
    };
    for (const name of names) visit(name);
    return enabled;
  }

  active(name) {
    return this.closure().has(name);
  }

  enable(name) {
    if (this.byName.has(name)) this.explicit.add(name);
  }

  toggle(name) {
    if (!this.byName.has(name)) return;
    if (this.active(name)) {
      for (const root of [...this.explicit]) {
        if (this.closure([root]).has(name)) this.explicit.delete(root);
      }
    } else {
      this.explicit.add(name);
    }
  }

  selected() {
    return this.features.map((feature) => feature.name).filter((name) => this.explicit.has(name));
  }

  command() {
    const features = this.selected();
    return features.length ? `cargo add textflow-rs --features ${features.join(",")}` : "cargo add textflow-rs";
  }
}

export function renderFeatures(target, graph, descriptions, onToggle) {
  target.replaceChildren();
  const enabled = graph.closure();
  for (const feature of graph.features) {
    const label = document.createElement("label");
    label.className = `feature-row${enabled.has(feature.name) && !graph.explicit.has(feature.name) ? " feature-required" : ""}`;
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = enabled.has(feature.name);
    input.addEventListener("change", () => onToggle(feature.name));
    const content = document.createElement("span");
    const title = document.createElement("span");
    title.className = "feature-name";
    title.textContent = feature.name;
    const description = document.createElement("span");
    description.className = "feature-description";
    description.textContent = descriptions.get(feature.name) ?? (feature.requires.length ? `requires ${feature.requires.join(", ")}` : "independent capability");
    content.append(title, description);
    label.append(input, content);
    target.append(label);
  }
}
