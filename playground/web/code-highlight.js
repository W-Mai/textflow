const KEYWORDS = new Set([
  "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
  "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
  "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
  "trait", "true", "type", "unsafe", "use", "where", "while",
]);

const LEXEME = /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])'|\b(?:0x[\da-fA-F_]+|\d[\d_]*)\b|[A-Za-z_][A-Za-z_0-9]*!?|::|=>|->|[^\s]/g;

function plain(text) {
  return {kind: "plain", text};
}

export function rustTokens(source) {
  const tokens = [];
  let cursor = 0;
  for (const match of source.matchAll(LEXEME)) {
    if (match.index > cursor) tokens.push(plain(source.slice(cursor, match.index)));
    const value = match[0];
    let kind = "plain";
    if (value.startsWith("//") || value.startsWith("/*")) kind = "comment";
    else if (value.startsWith('"') || value.startsWith("'")) kind = "string";
    else if (/^(?:0x|\d)/.test(value)) kind = "number";
    else if (KEYWORDS.has(value)) kind = "keyword";
    else if (value.endsWith("!")) kind = "macro";
    else if (/^[A-Z]/.test(value)) kind = "type";
    tokens.push({kind, text: value});
    cursor = match.index + value.length;
  }
  if (cursor < source.length) tokens.push(plain(source.slice(cursor)));
  return tokens;
}

function tomlValueTokens(source) {
  const tokens = [];
  const pattern = /#[^\n]*|"(?:\\.|[^"\\])*"|'[^']*'|\b(?:true|false|\d[\d_.]*)\b/g;
  let cursor = 0;
  for (const match of source.matchAll(pattern)) {
    if (match.index > cursor) tokens.push(plain(source.slice(cursor, match.index)));
    const value = match[0];
    const kind = value.startsWith("#") ? "comment" : value.startsWith('"') || value.startsWith("'") ? "string" : "number";
    tokens.push({kind, text: value});
    cursor = match.index + value.length;
  }
  if (cursor < source.length) tokens.push(plain(source.slice(cursor)));
  return tokens;
}

export function tomlTokens(source) {
  const tokens = [];
  for (const line of source.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
    const section = /^(\s*)(\[\[?[^\]\n]+\]\]?)/.exec(line);
    const assignment = /^(\s*)([A-Za-z_][\w.-]*)(\s*=\s*)/.exec(line);
    if (section) {
      tokens.push(plain(section[1]), {kind: "section", text: section[2]});
      tokens.push(...tomlValueTokens(line.slice(section[0].length)));
    } else if (assignment) {
      tokens.push(plain(assignment[1]), {kind: "key", text: assignment[2]}, plain(assignment[3]));
      tokens.push(...tomlValueTokens(line.slice(assignment[0].length)));
    } else {
      tokens.push(...tomlValueTokens(line));
    }
  }
  return tokens;
}

export function markdownTokens(source) {
  const tokens = [];
  for (const line of source.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
    const heading = /^(#{1,6})(\s+)/.exec(line);
    if (heading) {
      tokens.push({kind: "heading", text: heading[0]});
      tokens.push(...markdownInlineTokens(line.slice(heading[0].length), "headingText"));
      continue;
    }
    const list = /^(\s*)([-*+]\s+|\d+\.\s+)/.exec(line);
    if (list) {
      tokens.push(plain(list[1]), {kind: "marker", text: list[2]});
      tokens.push(...markdownInlineTokens(line.slice(list[0].length)));
    } else {
      tokens.push(...markdownInlineTokens(line));
    }
  }
  return tokens;
}

function markdownInlineTokens(source, baseKind = "plain") {
  const tokens = [];
  const pattern = /`[^`\n]+`|\*\*[^*\n]+\*\*|\[[^\]\n]+\]\([^\)\n]+\)/g;
  let cursor = 0;
  for (const match of source.matchAll(pattern)) {
    if (match.index > cursor) tokens.push({kind: baseKind, text: source.slice(cursor, match.index)});
    const value = match[0];
    tokens.push({kind: value.startsWith("`") ? "code" : value.startsWith("[") ? "link" : "emphasis", text: value});
    cursor = match.index + value.length;
  }
  if (cursor < source.length) tokens.push({kind: baseKind, text: source.slice(cursor)});
  return tokens;
}

export function renderCode(element, source, language) {
  const tokens = language === "toml" ? tomlTokens(source) : language === "markdown" ? markdownTokens(source) : rustTokens(source);
  const fragment = document.createDocumentFragment();
  for (const token of tokens) {
    if (token.kind === "plain") {
      fragment.append(document.createTextNode(token.text));
    } else {
      const span = document.createElement("span");
      span.className = `code-token-${token.kind}`;
      span.textContent = token.text;
      fragment.append(span);
    }
  }
  element.replaceChildren(fragment);
}

export function renderRust(element, source) {
  renderCode(element, source, "rust");
}
