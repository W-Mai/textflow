export const ODYSSEY_VERSE = [
  "ναιετάω δ' Ἰθάκην εὐδείελον· ἐν δ' ὄρος αὐτῇ,",
  "Νήριτον εἰνοσίφυλλον, ἀριπρεπές· ἀμφὶ δὲ νῆσοι",
  "πολλαὶ ναιετάουσι μάλα σχεδὸν ἀλλήλῃσι,",
  "Δουλίχιόν τε Σάμη τε καὶ ὑλήεσσα Ζάκυνθος.",
  "αὐτὴ δὲ χθαμαλὴ πανυπερτάτη εἰν ἁλὶ κεῖται",
  "πρὸς ζόφον, αἱ δέ τ' ἄνευθε πρὸς ἠῶ τ' ἠέλιόν τε,",
  "τρηχεῖ', ἀλλ' ἀγαθὴ κουροτρόφος· οὔ τι ἐγώ γε",
  "ἧς γαίης δύναμαι γλυκερώτερον ἄλλο ἰδέσθαι.",
].join(" ");

export const ODYSSEY_TEXT = Array(3).fill(ODYSSEY_VERSE).join(" ");

export const ODYSSEY_SOURCE = "https://el.wikisource.org/wiki/Οδύσσεια/ι";

export const HEART_PATH = Array.from({length: 65}, (_, index) => {
  const angle = index * Math.PI / 32;
  const vertical = 45 + (-13 * Math.cos(angle) + 5 * Math.cos(2 * angle)
    + 2 * Math.cos(3 * angle) + Math.cos(4 * angle)) * 6.3;
  return [Math.round(345 + 90 * Math.sin(angle) ** 3 + (vertical - 45) * .2), Math.round(vertical)];
});
const heartXs = HEART_PATH.map(([x]) => x);
const heartYs = HEART_PATH.map(([, y]) => y);
export const HEART_BOUNDS = [Math.min(...heartXs), Math.min(...heartYs),
  Math.max(...heartXs) - Math.min(...heartXs), Math.max(...heartYs) - Math.min(...heartYs)];

export function samplePath(path, segments = 1024) {
  if (!Number.isInteger(segments) || segments < 1 || segments > 2048) throw new Error("Invalid portrait sample count");
  const length = path.getTotalLength();
  if (!Number.isFinite(length) || length <= 0) throw new Error("Portrait path has no measurable length");
  const points = [];
  for (let index = 0; index <= segments; index++) {
    const point = path.getPointAtLength(length * index / segments);
    const x = Math.round(point.x);
    const y = Math.round(point.y);
    if (!Number.isFinite(x) || !Number.isFinite(y) || Math.abs(x) > 30000 || Math.abs(y) > 30000) {
      throw new Error("Portrait path exceeds the canvas limits");
    }
    if (points.length && points.at(-1)[0] === x && points.at(-1)[1] === y) continue;
    points.push([x, y]);
  }
  if (points.length < 2) throw new Error("Portrait path has too few distinct points");
  return points;
}

let portraitPromise;
export function loadPortrait() {
  if (!portraitPromise) portraitPromise = fetch(new URL("./single_portrait_fourier.svg", import.meta.url))
    .then((response) => {
      if (!response.ok) throw new Error("Portrait example is unavailable");
      return response.text();
    })
    .then((source) => {
      const xml = new DOMParser().parseFromString(source, "image/svg+xml");
      const svg = xml.documentElement;
      const sourcePath = svg.querySelector("path#portrait");
      const viewBox = svg.getAttribute("viewBox")?.trim().split(/[\s,]+/).map(Number);
      if (xml.querySelector("parsererror") || !sourcePath || !sourcePath.getAttribute("d")
        || viewBox?.length !== 4 || viewBox.some((value) => !Number.isFinite(value))
        || viewBox[2] <= 0 || viewBox[3] <= 0) {
        throw new Error("Portrait example is invalid");
      }
      const mount = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      mount.setAttribute("width", "0");
      mount.setAttribute("height", "0");
      mount.style.position = "absolute";
      mount.style.pointerEvents = "none";
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", sourcePath.getAttribute("d"));
      mount.append(path);
      document.body.append(mount);
      try { return {points: samplePath(path), viewBox}; }
      finally { mount.remove(); }
    })
    .catch((error) => { portraitPromise = null; throw error; });
  return portraitPromise;
}
