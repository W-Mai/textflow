const fragments = [
  "人人生而自由",
  "尊严和权利上一律平等",
  "ALL HUMAN BEINGS",
  "BORN FREE AND EQUAL",
  "IN DIGNITY AND RIGHTS",
  "すべての人間は",
  "生まれながらにして自由",
  "尊厳と権利とについて平等",
];

const directions = [
  [1, 0], [.7, .7], [0, 1], [-.7, .7],
  [-1, 0], [-.7, -.7], [0, -1], [.7, -.7],
];

export function buildTiles(width, height) {
  const tiles = [];
  const columns = Math.ceil(width / 180) + 2;
  const rows = Math.ceil(height / 74) + 2;
  for (let row = -1; row < rows; row++) {
    for (let column = -1; column < columns; column++) {
      const index = tiles.length;
      tiles.push({
        x: column * 180 + (row & 1 ? 70 : 0) + 24,
        y: row * 74 + 40,
        text: fragments[(row + 1 + 3 * (column + 1)) % fragments.length],
        tone: index % 3,
        index,
      });
    }
  }
  return tiles;
}

export function glyphTarget(glyph, pointer) {
  const dx = glyph.centerX - pointer.x;
  const dy = glyph.centerY - pointer.y;
  const distance = Math.hypot(dx, dy);
  const proximity = Math.max(0, 1 - distance / 170) ** 2 * pointer.strength;
  const [seedX, seedY] = directions[glyph.index % directions.length];
  const vx = dx + seedX * 12;
  const vy = dy + seedY * 12;
  const length = Math.hypot(vx, vy, 12);
  return {
    x: 96 * proximity * vx / length,
    y: 96 * proximity * vy / length,
    proximity,
  };
}

export function stepGlyph(glyph, target) {
  glyph.vx = (glyph.vx + (target.x - glyph.dx) * .18) * .74;
  glyph.vy = (glyph.vy + (target.y - glyph.dy) * .18) * .74;
  glyph.dx += glyph.vx;
  glyph.dy += glyph.vy;
  if (!target.proximity && Math.abs(glyph.dx) + Math.abs(glyph.dy)
    + Math.abs(glyph.vx) + Math.abs(glyph.vy) < .08) {
    glyph.dx = glyph.dy = glyph.vx = glyph.vy = 0;
  }
  return Math.abs(glyph.dx) + Math.abs(glyph.dy) > .05;
}

export function revealPulse(time) {
  const phase = (time % 23 + 23) % 23;
  const peak = Math.max(0, 1 - Math.abs(phase - 16) / 3);
  return peak * peak * (3 - 2 * peak);
}
