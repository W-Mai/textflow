const encoder = new TextEncoder();
const decoder = new TextDecoder();
export const VIEWPORT_WIDTH = Object.freeze({min: 32, max: 800, step: 16});

export function viewportWidthAt(x, projection) {
  const raw = (x - projection.x0) / projection.fit;
  const snapped = Math.round(raw / VIEWPORT_WIDTH.step) * VIEWPORT_WIDTH.step;
  return Math.max(VIEWPORT_WIDTH.min, Math.min(VIEWPORT_WIDTH.max, snapped));
}

export function stageSummary(response, visibleGlyphs = null) {
  if (!response?.ok) return "ERROR";
  const data = response.data;
  switch (data.kind) {
    case "core": return `${data.lines.length} LINES`;
    case "unicode": return `${data.graphemes.length} GRAPHEMES · ${data.breaks.length} BREAKS`;
    case "bidi": return `${data.visual.length} VISUAL RUNS`;
    case "layout": {
      const glyphs = visibleGlyphs !== null && visibleGlyphs < data.glyphs.length
        ? `${visibleGlyphs} / ${data.glyphs.length} GLYPHS` : `${data.glyphs.length} GLYPHS`;
      return `${glyphs} · ${data.lines.length} ${data.lines.length === 1 ? "LINE" : "LINES"}`;
    }
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

export function clipClusters(glyphs, spans, left, right) {
  const clusters = new Map();
  glyphs.forEach((glyph, index) => {
    const key = glyph.cluster?.join(":") ?? String(index);
    const span = spans[index];
    const bounds = clusters.get(key) ?? {left: Infinity, right: -Infinity};
    bounds.left = Math.min(bounds.left, span.left);
    bounds.right = Math.max(bounds.right, span.right);
    clusters.set(key, bounds);
  });
  return glyphs.map((glyph, index) => {
    const bounds = clusters.get(glyph.cluster?.join(":") ?? String(index));
    return bounds.left >= left - .5 && bounds.right <= right + .5;
  });
}

export class Stage {
  constructor(canvas, onInspect, onPathChange, onWidthChange) {
    this.canvas = canvas;
    this.onInspect = onInspect;
    this.onPathChange = onPathChange;
    this.onWidthChange = onWidthChange;
    this.hitboxes = [];
    this.last = null;
    this.editable = false;
    this.drawing = false;
    this.widthDragging = false;
    this.samples = [];
    canvas.addEventListener("pointermove", (event) => {
      if (this.widthDragging) this.resizeWidth(event);
      else if (this.drawing) {
        for (const sample of event.getCoalescedEvents?.() ?? [event]) this.record(sample);
      } else this.inspect(event);
    });
    canvas.addEventListener("pointerdown", (event) => {
      if (event.button === 0 && this.nearWidthGuide(event)) {
        event.preventDefault();
        this.widthDragging = true;
        canvas.setPointerCapture(event.pointerId);
        this.resizeWidth(event);
        return;
      }
      if (!this.editable) return this.inspect(event);
      if (event.button !== 0) return;
      event.preventDefault();
      this.drawing = true;
      this.samples = [];
      canvas.setPointerCapture(event.pointerId);
      this.record(event);
    });
    canvas.addEventListener("pointerup", (event) => {
      if (this.widthDragging) {
        this.resizeWidth(event);
        this.widthDragging = false;
        this.inspect(event);
        return;
      }
      if (!this.drawing) return;
      this.record(event);
      this.drawing = false;
    });
    canvas.addEventListener("pointercancel", () => { this.drawing = false; this.widthDragging = false; });
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
    this.canvas.style.touchAction = editable || this.hasWidthGuide() ? "none" : "";
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
    const extent = geometry ? 640 : VIEWPORT_WIDTH.max;
    const fit = Math.min(1, Math.max(1, width - 74) / Math.max(1, extent));
    return {x0: 29, y0: 98, scale: emScale * (geometry ? 1 : fit), fit};
  }

  hasWidthGuide() {
    const response = this.last?.[0];
    const data = response?.ok && response.data;
    return data?.kind === "core" || data?.kind === "layout" && !data.geometry;
  }

  nearWidthGuide(event) {
    if (!this.hasWidthGuide()) return false;
    const bounds = this.canvas.getBoundingClientRect();
    const options = this.last[2];
    const guide = this.projection(bounds.width, bounds.height, options, false);
    const x = guide.x0 + options.width * guide.fit;
    return Math.abs(event.clientX - bounds.left - x) <= 14
      && event.clientY - bounds.top >= 20 && event.clientY - bounds.top <= bounds.height - 6;
  }

  resizeWidth(event) {
    const bounds = this.canvas.getBoundingClientRect();
    const guide = this.projection(bounds.width, bounds.height, this.last[2], false);
    this.onWidthChange(viewportWidthAt(event.clientX - bounds.left, guide));
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
    this.canvas.style.cursor = this.widthDragging || this.nearWidthGuide(event) ? "ew-resize"
      : this.editable || found ? "crosshair" : "default";
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
    this.visibleGlyphs = null;
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
    const guide = data.kind === "core" || data.kind === "layout" && !data.geometry
      ? this.projection(width, height, options, false) : null;
    this.canvas.style.touchAction = this.editable || guide ? "none" : "";
    if (guide) this.widthShade(ctx, width, height, guide.x0 + options.width * guide.fit);
    switch (data.kind) {
      case "core": this.core(ctx, width, data, options, guide); break;
      case "unicode": this.unicode(ctx, width, data); break;
      case "bidi": this.bidi(ctx, width, data, source); break;
      case "layout": this.layout(ctx, width, data, options, overlays); break;
    }
    if (guide) {
      const columns = data.kind === "core" ? ` · ${Math.max(1, Math.floor(options.width / 16))} COL` : "";
      this.widthGuide(ctx, width, height, guide.x0 + options.width * guide.fit,
        `${options.width} PX${columns}`);
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

  widthShade(ctx, width, height, x) {
    ctx.save();
    ctx.fillStyle = this.palette.amber;
    ctx.globalAlpha = .045;
    ctx.fillRect(x, 52, Math.max(0, width - x), height - 52);
    ctx.restore();
  }

  widthGuide(ctx, width, height, x, label) {
    const edge = Math.round(x) + .5;
    ctx.save();
    ctx.strokeStyle = this.palette.amber;
    ctx.shadowColor = this.palette.amber;
    ctx.shadowBlur = 13;
    ctx.globalAlpha = .32;
    ctx.lineWidth = 5;
    ctx.beginPath(); ctx.moveTo(edge, 52); ctx.lineTo(edge, height - 18); ctx.stroke();
    ctx.shadowBlur = 0;
    ctx.globalAlpha = .9;
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(edge, 52); ctx.lineTo(edge, height - 18); ctx.stroke();
    ctx.lineWidth = 2;
    for (const y of [52, height - 18]) {
      ctx.beginPath(); ctx.moveTo(edge - 5, y + .5); ctx.lineTo(edge + 5, y + .5); ctx.stroke();
    }
    ctx.font = "700 10px ui-monospace, monospace";
    const labelWidth = (ctx.measureText(label)?.width ?? label.length * 6) + 18;
    const labelX = Math.max(8, Math.min(width - labelWidth - 8, x - labelWidth / 2));
    ctx.fillStyle = this.palette.bg;
    ctx.fillRect(labelX, 24, labelWidth, 22);
    ctx.strokeStyle = this.palette.amber;
    ctx.lineWidth = 1;
    ctx.strokeRect(labelX + .5, 24.5, labelWidth - 1, 21);
    ctx.fillStyle = this.palette.amber;
    ctx.fillText(label, labelX + 9, 39);
    const handleY = 57;
    ctx.fillStyle = this.palette.bg;
    ctx.fillRect(edge - 9, handleY - 7, 18, 14);
    ctx.strokeStyle = this.palette.amber;
    ctx.strokeRect(edge - 8.5, handleY - 6.5, 17, 13);
    for (const offset of [-3, 0, 3]) {
      ctx.beginPath(); ctx.moveTo(edge - 3, handleY + offset + .5);
      ctx.lineTo(edge + 3, handleY + offset + .5); ctx.stroke();
    }
    ctx.restore();
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

  core(ctx, width, data, options, guide) {
    const columns = Math.max(1, Math.floor(options.width / VIEWPORT_WIDTH.step));
    const left = guide.x0;
    const boundary = left + options.width * guide.fit;
    const height = this.canvas.clientHeight;
    const capacity = Math.max(2, Math.floor((height - 80) / 27));
    const truncated = data.lines.length > capacity;
    const visible = truncated ? capacity - 1 : data.lines.length;
    const room = height - (truncated ? 65 : 46) - 67;
    const rowStep = Math.max(22, Math.min(56, room / Math.max(1, visible - 1)));
    data.lines.slice(0, visible).forEach((line, index) => {
      const y = 67 + index * rowStep;
      const end = left + line.width * VIEWPORT_WIDTH.step * guide.fit;
      ctx.fillStyle = this.palette.text;
      ctx.font = "500 15px ui-sans-serif, system-ui";
      ctx.save();
      rectangle(ctx, left + 5, y - 2, Math.max(0, boundary - left - 10), 28);
      ctx.clip();
      ctx.fillText(line.text, left + 5, y + 20);
      ctx.restore();
      ctx.fillStyle = this.palette.line;
      ctx.fillRect(left, y + 30, Math.max(0, boundary - left), 4);
      ctx.fillStyle = this.palette.amber;
      ctx.fillRect(left, y + 30, Math.max(2, end - left), 4);
      ctx.fillRect(end - 1, y + 26, 2, 12);
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillStyle = this.palette.muted;
      ctx.fillText(`${line.width} / ${columns} COL`, Math.min(width - 74, boundary + 10), y + 36);
      this.hitboxes.push({x: left, y: y - 2, width: Math.max(0, boundary - left), height: 42,
        value: {type: "line", ...line}});
    });
    if (truncated) {
      ctx.fillStyle = this.palette.muted;
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillText(`+ ${data.lines.length - visible} MORE LINES`, left, height - 16);
    }
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
    const base = data.direction.toUpperCase();
    const height = this.canvas.clientHeight;
    const compact = height < 310;
    const cardHeight = compact ? 52 : 88;
    const sourceY = compact ? 42 : 71;
    const screenY = compact ? height - 65 : Math.min(229, height - cardHeight - 18);
    const middleY = (sourceY + cardHeight + screenY) / 2;
    const screenOrder = data.visual.map((run) => data.logical.findIndex((item) =>
      item.range[0] === run.range[0] && item.range[1] === run.range[1]) + 1);
    const orderLabel = (order) => `${order.slice(0, 4).join(" → ")}${order.length > 4 ? ` · +${order.length - 4}` : ""}`;
    ctx.fillStyle = this.palette.muted;
    ctx.font = "700 10px ui-monospace, monospace";
    ctx.fillText(`PARAGRAPH READS ${base === "RTL" ? "RIGHT TO LEFT" : "LEFT TO RIGHT"}`, 24, compact ? 19 : 36);
    for (const [label, runs, y] of [
      [`TYPED ORDER · ${orderLabel(data.logical.map((_, index) => index + 1))}`, data.logical, sourceY],
      [`ON SCREEN · ${orderLabel(screenOrder)}`, data.visual, screenY],
    ]) {
      ctx.fillStyle = this.palette.muted;
      ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillText(label, 24, y - (compact ? 8 : 11));
      const shown = runs.slice(0, 4);
      const remaining = runs.length - shown.length;
      const count = shown.length + (remaining > 0 ? 1 : 0);
      const gap = 9;
      const cardWidth = (width - 48 - gap * (count - 1)) / Math.max(1, count);
      shown.forEach((run, index) => {
        const x = 24 + index * (cardWidth + gap);
        const sourceIndex = data.logical.findIndex((item) => item.range[0] === run.range[0] && item.range[1] === run.range[1]);
        const color = this.palette.runs[(sourceIndex + 1) % this.palette.runs.length];
        const text = bytes(source, ...run.range).trim() || "SPACE";
        rectangle(ctx, x, y, cardWidth, cardHeight);
        ctx.fillStyle = this.palette.cell; ctx.fill();
        ctx.strokeStyle = color; ctx.stroke();
        ctx.fillStyle = color; ctx.font = "700 10px ui-monospace, monospace";
        ctx.fillText(`RUN ${String(sourceIndex + 1).padStart(2, "0")}`, x + 11, y + (compact ? 12 : 18));
        ctx.save();
        ctx.beginPath(); ctx.rect(x + 10, y + (compact ? 17 : 25), Math.max(0, cardWidth - 20), compact ? 20 : 31); ctx.clip();
        ctx.direction = run.direction;
        ctx.textAlign = run.direction === "rtl" ? "right" : "left";
        ctx.fillStyle = this.palette.text;
        ctx.font = `${compact ? 13 : 16}px ui-sans-serif, system-ui`;
        ctx.fillText(text, run.direction === "rtl" ? x + cardWidth - 11 : x + 11, y + (compact ? 33 : 47));
        ctx.restore();
        ctx.fillStyle = this.palette.muted; ctx.font = "10px ui-monospace, monospace";
        ctx.fillText(run.direction.toUpperCase(), x + 11, y + (compact ? 46 : 73));
        this.hitboxes.push({x, y, width: cardWidth, height: cardHeight,
          value: {type: label.startsWith("TYPED") ? "source run" : "screen run", text, sourceIndex: sourceIndex + 1, ...run}});
      });
      if (remaining > 0) {
        const x = 24 + shown.length * (cardWidth + gap);
        rectangle(ctx, x, y, cardWidth, cardHeight);
        ctx.strokeStyle = this.palette.line; ctx.stroke();
        ctx.fillStyle = this.palette.muted; ctx.font = "700 11px ui-monospace, monospace";
        ctx.fillText(`+${remaining} RUNS`, x + 10, y + cardHeight / 2 + 4, cardWidth - 20);
      }
    }
    if (compact) {
      const center = width / 2;
      ctx.strokeStyle = this.palette.amber;
      ctx.beginPath();
      ctx.moveTo(center, middleY - 9); ctx.lineTo(center, middleY + 5);
      ctx.moveTo(center - 4, middleY + 1); ctx.lineTo(center, middleY + 5);
      ctx.lineTo(center + 4, middleY + 1); ctx.stroke();
    } else {
      ctx.strokeStyle = this.palette.line;
      ctx.beginPath(); ctx.moveTo(24, middleY + 4.5); ctx.lineTo(width - 24, middleY + 4.5); ctx.stroke();
      ctx.fillStyle = this.palette.amber; ctx.font = "700 10px ui-monospace, monospace";
      ctx.fillText("SAME COLORS · NEW POSITIONS", 24, middleY);
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
    const glyphs = data.glyphs.map((glyph, index) => {
      const origin = glyph.frame?.origin ?? [glyph.origin[0] + glyph.offset[0], glyph.origin[1] + glyph.offset[1]];
      const x = x0 + origin[0] * scale;
      const y = y0 + origin[1] * scale;
      const advance = Math.max(8, glyph.advance[0] * scale);
      const font = options.example === "yuuu"
        ? `${Math.round(size)}px "Times New Roman", Georgia, serif`
        : `${Math.round(size)}px ui-monospace, SFMono-Regular, Menlo, monospace`;
      const character = glyph.character || "□";
      ctx.font = font;
      const metrics = ctx.measureText(character);
      const left = -(metrics?.actualBoundingBoxLeft ?? 0);
      const right = metrics?.actualBoundingBoxRight ?? metrics?.width ?? advance;
      return {
        glyph, index, x, y, advance, font, character, left, right,
        top: -(metrics?.actualBoundingBoxAscent ?? size * .82),
        bottom: metrics?.actualBoundingBoxDescent ?? size * .18,
        angle: glyph.frame ? Math.atan2(glyph.frame.tangent[1], glyph.frame.tangent[0]) : 0,
        run: runIndex(index),
      };
    });
    const visible = clipRight === null ? null : clipClusters(data.glyphs,
      glyphs.map((view) => ({left: Math.min(view.x, view.x + view.left),
        right: Math.max(view.x + view.right, view.x + view.advance)})), x0, clipRight);
    this.visibleGlyphs = visible ? visible.filter(Boolean).length : glyphs.length;
    for (const [index, line] of data.lines.entries()) {
      const baseline = data.baselines?.[index];
      if (baseline && options.example !== "yuuu") {
        this.curve(ctx, baseline, {x0, y0, scale});
      } else if (!baseline) {
        const y = y0 + line.origin[1] * scale;
        ctx.strokeStyle = this.palette.line; ctx.setLineDash([4, 5]);
        ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(data.geometry ? width - 25 : x0 + options.width * fit, y); ctx.stroke(); ctx.setLineDash([]);
        ctx.fillStyle = this.palette.muted; ctx.font = "9px ui-monospace, monospace";
        ctx.fillText(String(index + 1).padStart(2, "0"), 8, y - 5);
      }
    }
    if (clipRight !== null) {
      ctx.save();
      rectangle(ctx, x0, 0, Math.max(0, clipRight - x0), this.canvas.clientHeight);
      ctx.clip();
    }
    glyphs.forEach((view) => {
      if (visible && !visible[view.index]) return;
      const {glyph, x, y, font, character, left, right, top, bottom, angle} = view;
      const color = this.palette.runs[Math.max(0, view.run) % this.palette.runs.length];
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.font = font;
      if (overlays.boxes) {
        rectangle(ctx, left - 2, top - 2, Math.max(1, right - left + 4), Math.max(1, bottom - top + 4));
        ctx.fillStyle = `${color}12`; ctx.fill();
        ctx.strokeStyle = `${color}a0`; ctx.stroke();
      }
      ctx.fillStyle = color;
      ctx.fillText(character, 0, 0);
      ctx.restore();
      const value = {type: "glyph", ...glyph, run: view.run};
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
        if (clipRight !== null && (x < x0 || x > clipRight)) continue;
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
