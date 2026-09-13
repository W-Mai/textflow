const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function stageSummary(response) {
  if (!response?.ok) return "ERROR";
  const data = response.data;
  switch (data.kind) {
    case "core": return `${data.lines.length} LINES`;
    case "unicode": return `${data.graphemes.length} GRAPHEMES · ${data.breaks.length} BREAKS`;
    case "bidi": return `${data.visual.length} VISUAL RUNS`;
    case "layout": return `${data.glyphs.length} GLYPHS · ${data.lines.length} LINES`;
    default: return "";
  }
}

function bytes(text, start, end) {
  return decoder.decode(encoder.encode(text).slice(start, end));
}

function rectangle(ctx, x, y, width, height) {
  ctx.beginPath();
  ctx.rect(x, y, width, height);
}

export class Stage {
  constructor(canvas, onInspect) {
    this.canvas = canvas;
    this.onInspect = onInspect;
    this.hitboxes = [];
    this.last = null;
    canvas.addEventListener("pointermove", (event) => this.inspect(event));
    canvas.addEventListener("pointerdown", (event) => this.inspect(event));
    canvas.addEventListener("pointerleave", (event) => {
      if (event.pointerType !== "touch") this.onInspect(null);
    });
  }

  inspect(event) {
    const bounds = this.canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const found = [...this.hitboxes].reverse().find((box) => x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height);
    this.canvas.style.cursor = found ? "crosshair" : "default";
    this.onInspect(found?.value ?? null);
  }

  render(response, source, options, overlays) {
    this.last = [response, source, options, overlays];
    const styles = getComputedStyle(document.documentElement);
    const color = (name) => styles.getPropertyValue(`--stage-${name}`).trim();
    this.palette = {
      bg: color("bg"), grid: color("grid"), cell: color("cell"), cellAlt: color("cell-alt"),
      line: color("line"), text: color("text"), muted: color("muted"), amber: color("amber"),
      runs: Array.from({length: 5}, (_, index) => color(`color-${index}`)),
    };
    const data = response?.data;
    const lineCount = data?.kind === "layout" ? data.lines.length : 0;
    const height = Math.max(375, Math.min(850, lineCount * 58 + 145));
    this.canvas.style.height = `${height}px`;
    const width = Math.max(1, this.canvas.clientWidth);
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    this.canvas.width = Math.round(width * ratio);
    this.canvas.height = Math.round(height * ratio);
    const ctx = this.canvas.getContext("2d");
    ctx.scale(ratio, ratio);
    this.hitboxes = [];
    this.background(ctx, width, height);
    if (!response?.ok || !data) return;
    switch (data.kind) {
      case "core": this.core(ctx, width, data); break;
      case "unicode": this.unicode(ctx, width, data); break;
      case "bidi": this.bidi(ctx, width, data, source); break;
      case "layout": this.layout(ctx, width, data, options, overlays); break;
    }
  }

  redraw() {
    if (this.last) this.render(...this.last);
  }

  background(ctx, width, height) {
    ctx.fillStyle = this.palette.bg;
    ctx.fillRect(0, 0, width, height);
    ctx.strokeStyle = this.palette.grid;
    ctx.lineWidth = 1;
    for (let x = 20; x < width; x += 28) {
      ctx.beginPath(); ctx.moveTo(x + .5, 0); ctx.lineTo(x + .5, height); ctx.stroke();
    }
    for (let y = 20; y < height; y += 28) {
      ctx.beginPath(); ctx.moveTo(0, y + .5); ctx.lineTo(width, y + .5); ctx.stroke();
    }
    ctx.fillStyle = this.palette.amber;
    ctx.fillRect(width - 43, 21, 18, 2);
  }

  core(ctx, width, data) {
    const max = Math.max(1, ...data.lines.map((line) => line.width));
    data.lines.forEach((line, index) => {
      const y = 65 + index * 54;
      const barWidth = Math.min(width - 54, Math.max(130, (width - 54) * line.width / max));
      rectangle(ctx, 24, y, barWidth, 42);
      ctx.fillStyle = index % 2 ? this.palette.cell : this.palette.cellAlt;
      ctx.fill();
      ctx.strokeStyle = index % 2 ? this.palette.line : this.palette.amber;
      ctx.stroke();
      ctx.fillStyle = this.palette.text;
      ctx.font = "500 15px ui-sans-serif, system-ui";
      ctx.fillText(line.text, 38, y + 27, Math.max(70, barWidth - 88));
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillStyle = this.palette.muted;
      ctx.textAlign = "right";
      ctx.fillText(String(line.width).padStart(2, "0"), 24 + barWidth - 13, y + 26);
      ctx.textAlign = "left";
      this.hitboxes.push({x: 24, y, width: barWidth, height: 42, value: {type: "line", ...line}});
    });
  }

  unicode(ctx, width, data) {
    const breaks = new Set(data.breaks.map((item) => item.offset));
    let x = 24;
    let y = 72;
    data.graphemes.forEach((item, index) => {
      const script = data.scripts.find((run) => item.range[0] >= run.range[0] && item.range[0] < run.range[1]);
      const color = this.palette.runs[data.scripts.indexOf(script) % this.palette.runs.length] ?? this.palette.runs[0];
      const chipWidth = Math.max(43, Math.min(140, 30 + item.text.length * 15));
      if (x + chipWidth > width - 24) { x = 24; y += 61; }
      rectangle(ctx, x, y, chipWidth, 43);
      ctx.fillStyle = this.palette.cell; ctx.fill();
      ctx.strokeStyle = color; ctx.stroke();
      ctx.fillStyle = this.palette.text; ctx.font = "500 17px ui-sans-serif, system-ui";
      ctx.fillText(item.text.replaceAll("\n", "↵"), x + 11, y + 27, chipWidth - 17);
      if (breaks.has(item.range[1])) {
        ctx.fillStyle = this.palette.amber; ctx.fillRect(x + chipWidth - 8, y, 8, 8);
      }
      this.hitboxes.push({x, y, width: chipWidth, height: 43, value: {type: "grapheme", ...item, script: script?.script ?? "Common", breakAfter: breaks.has(item.range[1])}});
      x += chipWidth + 8;
    });
    ctx.fillStyle = this.palette.muted; ctx.font = "11px ui-monospace, monospace";
    ctx.fillText("Each frame is one grapheme · amber dots mark break opportunities", 24, Math.min(this.canvas.clientHeight - 20, y + 82));
  }

  bidi(ctx, width, data, source) {
    const gap = 14;
    const columnWidth = (width - 48 - gap) / 2;
    for (const [column, runs] of [["LOGICAL", data.logical], ["VISUAL", data.visual]]) {
      const columnIndex = column === "LOGICAL" ? 0 : 1;
      const x = 24 + columnIndex * (columnWidth + gap);
      ctx.fillStyle = columnIndex ? this.palette.runs[1] : this.palette.runs[0];
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillText(column, x, 62);
      runs.forEach((run, index) => {
        const y = 78 + index * 65;
        rectangle(ctx, x, y, columnWidth, 55);
        ctx.fillStyle = this.palette.cell; ctx.fill();
        ctx.strokeStyle = this.palette.runs[run.level % this.palette.runs.length]; ctx.stroke();
        ctx.fillStyle = this.palette.text; ctx.font = "15px ui-sans-serif, system-ui";
        ctx.fillText(bytes(source, ...run.range), x + 12, y + 25, columnWidth - 25);
        ctx.fillStyle = this.palette.muted; ctx.font = "10px ui-monospace, monospace";
        ctx.fillText(`${run.direction.toUpperCase()} · LEVEL ${run.level}`, x + 12, y + 44);
        this.hitboxes.push({x, y, width: columnWidth, height: 55, value: {type: `${column.toLowerCase()} run`, text: bytes(source, ...run.range), ...run}});
      });
    }
  }

  layout(ctx, width, data, options, overlays) {
    const fit = Math.min(1, (width - 74) / Math.max(1, options.width));
    const scale = data.emScale * fit;
    const x0 = 29;
    const y0 = 98;
    const size = Math.max(12, options.fontSize * fit);
    const runIndex = (index) => data.runs.findIndex((run) => index >= run.glyphs[0] && index < run.glyphs[1]);
    for (const line of data.lines) {
      const y = y0 + line.origin[1] * scale;
      ctx.strokeStyle = this.palette.line; ctx.setLineDash([4, 5]);
      ctx.beginPath(); ctx.moveTo(x0, y + .5); ctx.lineTo(width - 25, y + .5); ctx.stroke(); ctx.setLineDash([]);
      ctx.fillStyle = this.palette.muted; ctx.font = "9px ui-monospace, monospace";
      ctx.fillText(String(data.lines.indexOf(line) + 1).padStart(2, "0"), 8, y - 5);
    }
    data.glyphs.forEach((glyph, index) => {
      const origin = glyph.frame?.origin ?? [glyph.origin[0] + glyph.offset[0], glyph.origin[1] + glyph.offset[1]];
      const x = x0 + origin[0] * scale;
      const y = y0 + origin[1] * scale;
      const advance = Math.max(8, glyph.advance[0] * scale);
      const color = this.palette.runs[Math.max(0, runIndex(index)) % this.palette.runs.length];
      if (overlays.boxes) {
        rectangle(ctx, x - 2, y - size * .82, advance + 4, size + 6);
        ctx.fillStyle = `${color}12`; ctx.fill();
        ctx.strokeStyle = `${color}a0`; ctx.stroke();
      }
      ctx.save();
      if (glyph.frame) {
        ctx.translate(x, y);
        ctx.rotate(Math.atan2(glyph.frame.tangent[1], glyph.frame.tangent[0]));
        ctx.fillStyle = color;
        ctx.font = `${Math.round(size)}px ui-monospace, SFMono-Regular, Menlo, monospace`;
        ctx.fillText(glyph.character || "□", 0, 0);
      } else {
        ctx.fillStyle = color;
        ctx.font = `${Math.round(size)}px ui-monospace, SFMono-Regular, Menlo, monospace`;
        ctx.fillText(glyph.character || "□", x, y);
      }
      ctx.restore();
      this.hitboxes.push({x: x - 3, y: y - size, width: Math.max(advance + 6, 15), height: size + 9, value: {type: "glyph", ...glyph, run: runIndex(index)}});
    });
    if (overlays.carets) {
      for (const caret of data.carets) {
        const x = x0 + caret.position[0] * scale;
        const y = y0 + caret.position[1] * scale;
        ctx.strokeStyle = this.palette.amber; ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(x + .5, y - size); ctx.lineTo(x + .5, y + 5); ctx.stroke();
        this.hitboxes.push({x: x - 3, y: y - size, width: 6, height: size + 5, value: {type: "caret", ...caret}});
      }
    }
  }
}
