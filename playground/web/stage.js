const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function stageSummary(response) {
  if (!response?.ok) return "ERROR";
  const data = response.data;
  switch (data.kind) {
    case "core": return `${data.lines.length} LINES`;
    case "unicode": return `${data.graphemes.length} GRAPHEMES · ${data.breaks.length} BREAKS`;
    case "bidi": return `${data.visual.length} VISUAL RUNS`;
    case "layout": return `${data.glyphs.length} GLYPHS · ${data.lines.length} ${data.lines.length === 1 ? "LINE" : "LINES"}`;
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

export function containsHitbox(box, x, y) {
  if (box.local) {
    const dx = x - box.origin[0];
    const dy = y - box.origin[1];
    const cosine = Math.cos(box.angle);
    const sine = Math.sin(box.angle);
    const localX = dx * cosine + dy * sine;
    const localY = -dx * sine + dy * cosine;
    return localX >= box.local[0] && localX <= box.local[2]
      && localY >= box.local[1] && localY <= box.local[3];
  }
  return x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height;
}

export class Stage {
  constructor(canvas, onInspect, onPathChange) {
    this.canvas = canvas;
    this.onInspect = onInspect;
    this.onPathChange = onPathChange;
    this.hitboxes = [];
    this.last = null;
    this.editable = false;
    this.drawing = false;
    this.samples = [];
    canvas.addEventListener("pointermove", (event) => {
      if (this.drawing) {
        for (const sample of event.getCoalescedEvents?.() ?? [event]) this.record(sample);
      } else this.inspect(event);
    });
    canvas.addEventListener("pointerdown", (event) => {
      if (!this.editable) return this.inspect(event);
      if (event.button !== 0) return;
      event.preventDefault();
      this.drawing = true;
      this.samples = [];
      canvas.setPointerCapture(event.pointerId);
      this.record(event);
    });
    canvas.addEventListener("pointerup", (event) => {
      if (!this.drawing) return;
      this.record(event);
      this.drawing = false;
    });
    canvas.addEventListener("pointercancel", () => { this.drawing = false; });
    canvas.addEventListener("pointerleave", (event) => {
      if (event.pointerType !== "touch") this.onInspect(null);
    });
  }

  setEditable(editable) {
    this.editable = editable;
    if (!editable) {
      this.drawing = false;
      this.lastBaseline = null;
    }
    this.canvas.style.touchAction = editable ? "none" : "";
    this.canvas.style.cursor = editable ? "crosshair" : "default";
  }

  projection(width, height, options, geometry) {
    const emScale = options.fontSize / 1000;
    if (geometry && options.example === "yuuu" && options.pathBounds) {
      const [left, top, boxWidth, boxHeight] = options.pathBounds;
      const fit = Math.min((width - 48) / boxWidth, (height - Math.min(54, height * .18)) / boxHeight);
      return {
        x0: (width - boxWidth * fit) / 2 - left * fit,
        y0: (height - boxHeight * fit) / 2 - top * fit,
        scale: emScale,
        fit,
      };
    }
    const extent = geometry ? 640 : options.width;
    const fit = Math.min(1, Math.max(1, width - 74) / Math.max(1, extent));
    return {x0: 29, y0: 98, scale: emScale * (geometry ? 1 : fit), fit};
  }

  record(event) {
    const options = this.last?.[2];
    if (!options) return;
    const bounds = this.canvas.getBoundingClientRect();
    const x = Math.max(0, Math.min(bounds.width, event.clientX - bounds.left));
    const y = Math.max(0, Math.min(bounds.height, event.clientY - bounds.top));
    const {x0, y0, fit} = this.projection(bounds.width, bounds.height, options, true);
    const point = [Math.max(-30_000, Math.min(30_000, Math.round((x - x0) / fit))), Math.max(-30_000, Math.min(30_000, Math.round((y - y0) / fit)))];
    const previous = this.samples.at(-1);
    if (previous && Math.hypot(point[0] - previous[0], point[1] - previous[1]) < 3) return;
    if (this.samples.length >= 512) this.samples = this.samples.filter((_, index) => index % 2 === 0);
    this.samples.push(point);
    this.onPathChange(this.samples.slice());
  }

  inspect(event) {
    const bounds = this.canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const found = [...this.hitboxes].reverse().find((box) => containsHitbox(box, x, y));
    this.canvas.style.cursor = this.editable || found ? "crosshair" : "default";
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
    const height = Math.max(1, this.canvas.clientHeight);
    const width = Math.max(1, this.canvas.clientWidth);
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    this.canvas.width = Math.round(width * ratio);
    this.canvas.height = Math.round(height * ratio);
    const ctx = this.canvas.getContext("2d");
    ctx.scale(ratio, ratio);
    this.hitboxes = [];
    this.background(ctx, width, height);
    if (this.editable && !response?.ok) {
      if (options.path?.length) {
        const {x0, y0, fit} = this.projection(width, height, options, true);
        this.curve(ctx, options.path, {x0, y0, scale: fit}, false);
      } else if (this.lastBaseline) {
        const projection = this.projection(width, height, {fontSize: this.lastBaseline.fontSize}, true);
        this.curve(ctx, this.lastBaseline.points, projection);
      }
    }
    if (!response?.ok || !data) return;
    if (this.editable && data.kind === "layout" && data.baselines?.[0]) {
      this.lastBaseline = {points: data.baselines[0], fontSize: options.fontSize};
    }
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

  curve(ctx, points, projection, smooth = true) {
    if (!points.length) return;
    if (points.length === 1) {
      ctx.fillStyle = this.palette.amber;
      ctx.beginPath();
      ctx.arc(projection.x0 + points[0][0] * projection.scale, projection.y0 + points[0][1] * projection.scale, 3, 0, Math.PI * 2);
      ctx.fill();
      return;
    }
    ctx.beginPath();
    ctx.moveTo(projection.x0 + points[0][0] * projection.scale, projection.y0 + points[0][1] * projection.scale);
    for (let index = 1; index < points.length; index++) {
      ctx.lineTo(projection.x0 + points[index][0] * projection.scale, projection.y0 + points[index][1] * projection.scale);
    }
    ctx.strokeStyle = this.palette.amber;
    ctx.lineWidth = smooth ? 2 : 1.5;
    ctx.stroke();
    ctx.lineWidth = 1;
  }

  core(ctx, width, data) {
    const max = Math.max(1, ...data.lines.map((line) => line.width));
    data.lines.forEach((line, index) => {
      const y = 65 + index * 54;
      const rowWidth = width - 48;
      rectangle(ctx, 24, y, rowWidth, 42);
      ctx.fillStyle = index % 2 ? this.palette.cell : this.palette.cellAlt;
      ctx.fill();
      ctx.strokeStyle = index % 2 ? this.palette.line : this.palette.amber;
      ctx.stroke();
      ctx.fillStyle = this.palette.text;
      ctx.font = "500 15px ui-sans-serif, system-ui";
      ctx.save();
      rectangle(ctx, 38, y + 3, Math.max(0, rowWidth - 112), 34);
      ctx.clip();
      ctx.fillText(line.text, 38, y + 25);
      ctx.restore();
      ctx.fillStyle = this.palette.amber;
      ctx.fillRect(24, y + 39, Math.max(2, rowWidth * line.width / max), 3);
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillStyle = this.palette.muted;
      ctx.textAlign = "right";
      ctx.fillText(`${line.width} COL`, 24 + rowWidth - 13, y + 26);
      ctx.textAlign = "left";
      this.hitboxes.push({x: 24, y, width: rowWidth, height: 42, value: {type: "line", ...line}});
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
    const {x0, y0, scale, fit} = this.projection(width, this.canvas.clientHeight, options, data.geometry);
    const size = data.geometry ? options.fontSize : Math.max(12, options.fontSize * fit);
    const clipRight = options.overflow === "clip" && !data.geometry ? x0 + options.width * fit : null;
    const addHitbox = (box) => {
      if (clipRight !== null) {
        const left = Math.max(x0, box.x);
        const right = Math.min(clipRight, box.x + box.width);
        if (right <= left) return;
        box.x = left;
        box.width = right - left;
      }
      this.hitboxes.push(box);
    };
    const runIndex = (index) => data.runs.findIndex((run) => index >= run.glyphs[0] && index < run.glyphs[1]);
    for (const [index, line] of data.lines.entries()) {
      const baseline = data.baselines?.[index];
      if (baseline && options.example !== "yuuu") {
        this.curve(ctx, baseline, {x0, y0, scale});
      } else if (!baseline) {
        const y = y0 + line.origin[1] * scale;
        ctx.strokeStyle = this.palette.line; ctx.setLineDash([4, 5]);
        ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(width - 25, y); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = this.palette.muted; ctx.font = "9px ui-monospace, monospace";
        ctx.fillText(String(index + 1).padStart(2, "0"), 8, y - 5);
      }
    }
    if (clipRight !== null) {
      ctx.save();
      rectangle(ctx, x0, 0, Math.max(0, clipRight - x0), this.canvas.clientHeight);
      ctx.clip();
    }
    data.glyphs.forEach((glyph, index) => {
      const origin = glyph.frame?.origin ?? [glyph.origin[0] + glyph.offset[0], glyph.origin[1] + glyph.offset[1]];
      const x = x0 + origin[0] * scale;
      const y = y0 + origin[1] * scale;
      const advance = Math.max(8, glyph.advance[0] * scale);
      const color = this.palette.runs[Math.max(0, runIndex(index)) % this.palette.runs.length];
      const character = glyph.character || "□";
      const angle = glyph.frame ? Math.atan2(glyph.frame.tangent[1], glyph.frame.tangent[0]) : 0;
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.font = options.example === "yuuu"
        ? `${Math.round(size)}px "Times New Roman", Georgia, serif`
        : `${Math.round(size)}px ui-monospace, SFMono-Regular, Menlo, monospace`;
      const metrics = ctx.measureText(character);
      const left = -(metrics?.actualBoundingBoxLeft ?? 0);
      const right = metrics?.actualBoundingBoxRight ?? metrics?.width ?? advance;
      const top = -(metrics?.actualBoundingBoxAscent ?? size * .82);
      const bottom = metrics?.actualBoundingBoxDescent ?? size * .18;
      if (overlays.boxes) {
        rectangle(ctx, left - 2, top - 2, Math.max(1, right - left + 4), Math.max(1, bottom - top + 4));
        ctx.fillStyle = `${color}12`; ctx.fill();
        ctx.strokeStyle = `${color}a0`; ctx.stroke();
      }
      ctx.fillStyle = color;
      ctx.fillText(character, 0, 0);
      ctx.restore();
      const value = {type: "glyph", ...glyph, run: runIndex(index)};
      if (glyph.frame) {
        addHitbox({origin: [x, y], angle, local: [left - 3, top - 3, right + 3, bottom + 3], value});
      } else {
        addHitbox({x: x + left - 3, y: y + top - 3, width: Math.max(1, right - left + 6), height: Math.max(1, bottom - top + 6), value});
      }
    });
    if (overlays.carets) {
      for (const caret of data.carets) {
        const origin = caret.frame?.origin ?? caret.position;
        const x = x0 + origin[0] * scale;
        const y = y0 + origin[1] * scale;
        ctx.strokeStyle = this.palette.amber; ctx.lineWidth = 1;
        ctx.save();
        ctx.translate(x, y);
        const angle = caret.frame ? Math.atan2(caret.frame.tangent[1], caret.frame.tangent[0]) : 0;
        ctx.rotate(angle);
        ctx.beginPath(); ctx.moveTo(.5, -size); ctx.lineTo(.5, 5); ctx.stroke();
        ctx.restore();
        const value = {type: "caret", ...caret};
        if (caret.frame) addHitbox({origin: [x, y], angle, local: [-3, -size, 3, 5], value});
        else addHitbox({x: x - 3, y: y - size, width: 6, height: size + 5, value});
      }
    }
    if (clipRight !== null) ctx.restore();
  }
}
