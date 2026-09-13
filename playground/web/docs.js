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

function docText(target, markdown) {
  if (!markdown?.trim()) {
    return;
  }
  let code = false;
  let buffer = [];
  const flush = () => {
    if (!buffer.length) return;
    target.append(element(code ? "pre" : "p", "", buffer.join("\n")));
    buffer = [];
  };
  for (const line of markdown.split("\n")) {
    if (line.trim().startsWith("```")) {
      flush();
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
  }

  visible(features, enabled) {
    return features.every((feature) => enabled.has(feature));
  }

  render(enabled) {
    this.active = enabled;
    const query = this.search.value.trim().toLowerCase();
    this.list.replaceChildren();
    let count = 0;
    for (const module of this.docs.modules) {
      if (!this.visible(module.features, enabled)) continue;
      const items = module.items.filter((item) =>
        this.visible(item.features, enabled) &&
        (!query || `${module.name} ${item.name} ${item.kind} ${item.doc}`.toLowerCase().includes(query))
      );
      if (!items.length) continue;
      const group = element("div", "docs-group");
      group.append(element("h3", "", module.name || "crate root"));
      for (const item of items) {
        const key = `${module.name}::${item.name}`;
        const button = element("button", key === this.selected ? "is-active" : "", item.name);
        button.type = "button";
        button.title = `${item.kind} ${key}`;
        button.addEventListener("click", () => this.select(module, item));
        group.append(button);
        count += 1;
      }
      this.list.append(group);
    }
    if (!count) this.list.append(element("p", "docs-placeholder", "No items match the active features and query."));
    if (this.selected) {
      const stillVisible = [...this.list.querySelectorAll("button")].some((button) => button.title.endsWith(this.selected));
      if (!stillVisible) {
        this.selected = null;
        this.detail.replaceChildren(element("div", "docs-placeholder", "Select an API item to inspect its signature and source."));
      }
    }
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
    this.detail.append(element("pre", "", item.signature));
    docText(this.detail, item.doc);
    this.detail.append(sourceLink(this.rev, item.source ?? module.source, item.line));
    for (const method of item.methods ?? []) {
      if (!this.visible(method.features, this.active)) continue;
      const block = element("section", "doc-method");
      block.append(element("h4", "", method.name));
      block.append(element("pre", "", method.signature));
      docText(block, method.doc);
      block.append(sourceLink(this.rev, item.source ?? module.source, method.line, `Source line ${method.line} ↗`));
      this.detail.append(block);
    }
  }
}
