/** The Sparkle mark from the design canvas: a gem-cut four-point star on a 17×17 grid. */

type Cell = [number, number];

const px = (cells: Cell[]) => cells.map(([x, y]) => `M${x} ${y}h1v1h-1z`).join("");

const R = 7;
const layers = (() => {
  const main: Cell[] = [];
  const light: Cell[] = [];
  const dark: Cell[] = [];
  for (let dy = -R; dy <= R; dy++) {
    for (let dx = -R; dx <= R; dx++) {
      if ((Math.abs(dx) + 1) * (Math.abs(dy) + 1) > R + 1 || (dx === 0 && dy === 0)) continue;
      const cell: Cell = [dx + R + 1, dy + R + 1];
      if (dx + dy < 0) light.push(cell);
      else if (dx + dy > 0) dark.push(cell);
      else main.push(cell);
    }
  }
  return { main: px(main), light: px(light), dark: px(dark), core: px([[R + 1, R + 1]]) };
})();

const mini = px([[14, 1], [13, 2], [14, 2], [15, 2], [14, 3]]);

export function SparkleMark({ size = 34 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 17 17" shapeRendering="crispEdges" aria-hidden="true">
      <path d={layers.main} fill="var(--accent)" />
      <path d={layers.light} fill="var(--accent-light)" />
      <path d={layers.dark} fill="var(--accent-dark)" />
      <path d={layers.core} fill="#fff" />
      <path className="twinkle" d={mini} fill="#fff" />
    </svg>
  );
}

const WORD = [
  "...........s............",
  "..........sss...........",
  "......##...s.........#..",
  ".......#.............#..",
  ".####..#..##..####..####",
  "#...#..#...#..#...#..#..",
  "#...#..#...#..#...#..#..",
  "#...#..#...#..#...#..#..",
  ".####..##.###.#...#...##",
  "....#...................",
  ".###....................",
];

const wordCells = (ch: string) =>
  WORD.flatMap((row, y) => [...row].flatMap((c, x): Cell[] => (c === ch ? [[x, y]] : [])));

const word = { text: px(wordCells("#")), spark: px(wordCells("s")) };

export function Wordmark({ height = 22 }: { height?: number }) {
  return (
    <svg height={height} viewBox="0 0 24 11" shapeRendering="crispEdges" role="img" aria-label="Glint">
      <path d={word.text} fill="var(--text)" />
      <path d={word.spark} fill="var(--accent)" />
    </svg>
  );
}
