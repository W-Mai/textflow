export function formatBaselinePoints(points) {
  if (!Array.isArray(points) || points.length < 2 || points.some((point) =>
    !Array.isArray(point) || point.length !== 2 || point.some((value) =>
      !Number.isInteger(value) || value < -2147483648 || value > 2147483647))) {
    throw new Error("Baseline points are unavailable");
  }
  return `use textflow::shaping::FlowPoint;\n\nconst POINTS: &[FlowPoint] = &[\n${points.map(([x, y]) =>
    `    FlowPoint { x: ${x}, y: ${y} },`).join("\n")}\n];\n`;
}
