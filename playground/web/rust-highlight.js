const KEYWORDS = new Set([
  "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
  "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
  "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
  "trait", "true", "type", "unsafe", "use", "where", "while",
]);

const LEXEME = /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])'|\b(?:0x[\da-fA-F_]+|\d[\d_]*)\b|[A-Za-z_][A-Za-z_0-9]*!?|::|=>|->|[^\s]/g;

export function rustTokens(source) {
  const tokens = [];
  let cursor = 0;
  for (const match of source.matchAll(LEXEME)) {
    if (match.index > cursor) tokens.push({kind: "plain", text: source.slice(cursor, match.index)});
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
  if (cursor < source.length) tokens.push({kind: "plain", text: source.slice(cursor)});
  return tokens;
}

export function renderRust(element, source) {
  const fragment = document.createDocumentFragment();
  for (const token of rustTokens(source)) {
    if (token.kind === "plain") {
      fragment.append(document.createTextNode(token.text));
    } else {
      const span = document.createElement("span");
      span.className = `rust-token-${token.kind}`;
      span.textContent = token.text;
      fragment.append(span);
    }
  }
  element.replaceChildren(fragment);
}
