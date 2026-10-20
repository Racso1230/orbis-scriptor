/**
 * The stamp library: small hand-drawn icons (cities, castles, mountains,
 * forests...) that can be placed on a map.
 *
 * Every stamp is a list of SVG paths in a 100 x 100 box with the anchor at the
 * CENTRE (50, 50): the point you click is the middle of the stamp. They are
 * drawn in a simple "fantasy atlas" ink style: a dark outline with light fills.
 *
 * The same data renders in two places: Konva `Path` nodes on the map
 * (`stamp-renderer.tsx`) and a DOM `<svg>` in the palette (`stamp-palette.tsx`),
 * so a preview always matches the canvas.
 *
 * To add a stamp, append an entry to `STAMPS`. Keys are `a-z0-9-` (the backend
 * validates that) and must never change once released, because saved maps
 * refer to them.
 */

/** Size of the drawing box. */
export const STAMP_BOX = 100;
/** Pixels (map pixels) a stamp of scale 1 occupies. */
export const STAMP_BASE_SIZE = 64;

/**
 * What a shape is filled with. Roles (not colours) so a stamp can be tinted:
 * `paper` is the light body, `accent` a mid tone (roofs, shading), `white` a
 * highlight (snow, sails), `ink` the dark outline colour and `hot` fire/lava.
 */
export type StampFill = "paper" | "accent" | "white" | "ink" | "hot" | "none";

export interface StampShape {
  /** SVG path data in the 100 x 100 box. */
  d: string;
  fill: StampFill;
  /** Outline width in drawing units (default 2.2); 0 = no outline. */
  w?: number;
}

export type StampCategory = "settlements" | "nature" | "biomes" | "other";

export interface StampDef {
  key: string;
  name: string;
  category: StampCategory;
  shapes: StampShape[];
}

export const STAMP_CATEGORIES: readonly { id: StampCategory; label: string }[] = [
  { id: "settlements", label: "Settlements" },
  { id: "nature", label: "Nature" },
  { id: "biomes", label: "Biomes" },
  { id: "other", label: "Other" },
];

export interface StampPalette {
  ink: string;
  paper: string;
  accent: string;
  white: string;
  hot: string;
}

export const DEFAULT_STAMP_INK = "#2b2118";
const PAPER = "#f1e6c8";
const ACCENT = "#b79c6e";
const WHITE = "#fffdf5";
const HOT = "#d9541e";

function hexToRgb(hex: string): [number, number, number] {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

/** `amount` (0..1) of `b` mixed into `a`; both are `#rrggbb`. */
export function mixHex(a: string, b: string, amount: number): string {
  const [ar, ag, ab] = hexToRgb(a);
  const [br, bg, bb] = hexToRgb(b);
  const mix = (x: number, y: number) =>
    Math.round(x + (y - x) * amount)
      .toString(16)
      .padStart(2, "0");
  return `#${mix(ar, br)}${mix(ag, bg)}${mix(ab, bb)}`;
}

/**
 * The colours of a stamp. With no tint it is black-brown ink on cream; with
 * a tint (`style.color`) the outline takes that colour and the mid tone is a
 * paler version of it, so the stamp still reads as one drawing.
 */
export function stampPalette(tint?: string | null): StampPalette {
  if (!tint || !/^#[0-9a-fA-F]{6}$/.test(tint)) {
    return { ink: DEFAULT_STAMP_INK, paper: PAPER, accent: ACCENT, white: WHITE, hot: HOT };
  }
  return {
    ink: tint,
    paper: mixHex(tint, WHITE, 0.82),
    accent: mixHex(tint, WHITE, 0.45),
    white: WHITE,
    hot: HOT,
  };
}

/** Resolves a fill role to a colour (`none` stays `none`). */
export function fillColor(fill: StampFill, palette: StampPalette): string {
  return fill === "none" ? "none" : palette[fill];
}

export const DEFAULT_STAMP_WIDTH = 2.2;

// ---- drawing helpers (they only build path strings) -------------------

const n = (v: number) => String(Math.round(v * 10) / 10);

/** A row of battlement blocks along y, from x0 to x1. */
function merlons(x0: number, x1: number, y: number, h: number, w: number): string {
  let d = "";
  const step = w * 2;
  for (let x = x0; x + w <= x1 + 0.01; x += step) {
    d += `M${n(x)} ${n(y)}V${n(y - h)}H${n(x + w)}V${n(y)}Z`;
  }
  return d;
}

/** A pointed conifer, apex at (cx, by - h), trunk base at (cx, by). */
function pine(cx: number, by: number, h: number): string {
  const w = h * 0.62;
  const p = (dx: number, y: number) => `${n(cx + dx * w)} ${n(by - y * h)}`;
  const m = (dx: number, y: number) => `${n(cx - dx * w)} ${n(by - y * h)}`;
  return (
    `M${p(0, 1)}L${p(0.2, 0.62)}L${p(0.12, 0.62)}L${p(0.31, 0.34)}L${p(0.18, 0.34)}` +
    `L${p(0.44, 0.13)}L${p(0.08, 0.13)}L${p(0.08, 0)}L${m(0.08, 0)}L${m(0.08, 0.13)}` +
    `L${m(0.44, 0.13)}L${m(0.18, 0.34)}L${m(0.31, 0.34)}L${m(0.12, 0.62)}L${m(0.2, 0.62)}Z`
  );
}

/** A round-crowned tree: trunk and crown as two separate shapes. */
function broadleaf(cx: number, by: number, r: number): { trunk: string; crown: string } {
  const cy = by - r * 1.55;
  return {
    trunk: `M${n(cx - r * 0.18)} ${n(by)}V${n(cy + r * 0.6)}H${n(cx + r * 0.18)}V${n(by)}Z`,
    crown: `M${n(cx - r)} ${n(cy)}A${n(r)} ${n(r)} 0 1 1 ${n(cx + r)} ${n(cy)}A${n(r)} ${n(r)} 0 1 1 ${n(cx - r)} ${n(cy)}Z`,
  };
}

function trees(spots: [number, number, number][]): StampShape[] {
  const out: StampShape[] = [];
  for (const [cx, by, r] of spots) {
    const t = broadleaf(cx, by, r);
    out.push({ d: t.trunk, fill: "ink", w: 1 }, { d: t.crown, fill: "accent" });
  }
  return out;
}

function pines(spots: [number, number, number][]): StampShape[] {
  return spots.map(([cx, by, h]) => ({ d: pine(cx, by, h), fill: "accent" as const, w: 2 }));
}

/** A flat-roofed house block with a triangular roof. */
function house(x: number, w: number, top: number, base: number, roofH: number): StampShape[] {
  return [
    { d: `M${n(x)} ${n(base)}V${n(top)}H${n(x + w)}V${n(base)}Z`, fill: "paper" },
    {
      d: `M${n(x - 3)} ${n(top)}L${n(x + w / 2)} ${n(top - roofH)}L${n(x + w + 3)} ${n(top)}Z`,
      fill: "accent",
    },
  ];
}

/**
 * A palm tree: a curved trunk from (cx, by) up to a crown `h` higher and
 * `lean` to the side, with fronds sized by `s`. Returns trunk, fronds, coconuts.
 */
function palm(cx: number, by: number, h: number, lean: number, s: number): StampShape[] {
  const tx = cx + lean;
  const ty = by - h;
  const mx = cx + lean * 0.3;
  const my = by - h * 0.5;
  const trunk =
    `M${n(cx - 2.5)} ${n(by)}Q${n(mx - 2)} ${n(my)} ${n(tx - 1.5)} ${n(ty)}` +
    `H${n(tx + 1.5)}Q${n(mx + 2)} ${n(my)} ${n(cx + 2.5)} ${n(by)}Z`;
  // Each frond is a crescent: out to the tip along one curve, back along a flatter one.
  const frond = (dx: number, dy: number, bend: number) => {
    const ex = tx + dx * s;
    const ey = ty + dy * s;
    const qx = tx + dx * s * 0.5;
    const qy = ty + dy * s * 0.5 - bend * s;
    return `M${n(tx)} ${n(ty)}Q${n(qx)} ${n(qy)} ${n(ex)} ${n(ey)}Q${n(qx)} ${n(qy + 6 * s)} ${n(tx)} ${n(ty)}Z`;
  };
  const fronds =
    frond(-22, 8, 10) + frond(22, 8, 10) + frond(-15, -5, 7) + frond(15, -5, 7) + frond(-4, 16, 2);
  return [
    { d: trunk, fill: "paper", w: 1.6 },
    { d: fronds, fill: "accent", w: 1.6 },
    { d: `M${n(tx - 2)} ${n(ty + 1)}a1.8 1.8 0 1 0 0.1 0Z`, fill: "ink", w: 0.8 },
  ];
}

/** Stamp definitions, grouped by category (the order here is the palette order). */
export const STAMPS: readonly StampDef[] = [
  // ---- settlements ----
  {
    key: "city",
    name: "City",
    category: "settlements",
    shapes: [
      ...house(10, 24, 56, 86, 16),
      ...house(38, 24, 42, 86, 24),
      ...house(66, 24, 58, 86, 14),
      { d: "M47 86V74Q50 69 53 74V86Z", fill: "ink", w: 1 },
      { d: "M18 66h5v7h-5zM74 68h5v7h-5zM45 52h4v7h-4zM52 52h4v7h-4z", fill: "ink", w: 0 },
      { d: "M50 18V8", fill: "none", w: 1.6 },
      { d: "M50 8L60 11L50 14Z", fill: "hot", w: 1.2 },
      { d: "M6 86H94", fill: "none", w: 2 },
    ],
  },
  {
    key: "town",
    name: "Town",
    category: "settlements",
    shapes: [
      ...house(12, 30, 54, 82, 20),
      ...house(50, 34, 48, 82, 24),
      { d: "M62 82V70Q67 65 72 70V82Z", fill: "ink", w: 1 },
      { d: "M20 62h6v7h-6zM37 62h4v7h-4zM76 58h5v7h-5z", fill: "ink", w: 0 },
      { d: "M8 82H92", fill: "none", w: 2 },
    ],
  },
  {
    key: "village",
    name: "Village",
    category: "settlements",
    shapes: [
      ...house(16, 30, 56, 84, 20),
      { d: "M34 38V30H38V42Z", fill: "paper", w: 1.6 },
      { d: "M26 84V72Q31 67 36 72V84Z", fill: "ink", w: 1 },
      ...trees([[72, 84, 11]]),
      { d: "M10 84H90", fill: "none", w: 2 },
    ],
  },
  {
    key: "castle",
    name: "Castle",
    category: "settlements",
    shapes: [
      { d: "M12 86V50H88V86Z", fill: "paper" },
      { d: merlons(12, 88, 50, 6, 6), fill: "paper", w: 1.6 },
      { d: "M6 86V34H32V86Z", fill: "accent" },
      { d: merlons(6, 32, 34, 7, 6.5), fill: "accent", w: 1.6 },
      { d: "M68 86V34H94V86Z", fill: "accent" },
      { d: merlons(68, 94, 34, 7, 6.5), fill: "accent", w: 1.6 },
      { d: "M40 86V66Q50 54 60 66V86Z", fill: "ink", w: 1.2 },
      { d: "M17 46v9M81 46v9", fill: "none", w: 3 },
      { d: "M81 27V10", fill: "none", w: 1.6 },
      { d: "M81 10L93 14L81 18Z", fill: "hot", w: 1.2 },
    ],
  },
  {
    key: "tower",
    name: "Tower",
    category: "settlements",
    shapes: [
      { d: "M33 90L37 34H63L67 90Z", fill: "paper" },
      { d: "M31 34V22H39V28H46V22H54V28H61V22H69V34Z", fill: "accent" },
      { d: "M44 90V74Q50 67 56 74V90Z", fill: "ink", w: 1.2 },
      { d: "M48 46h4v10h-4zM48 60h4v6h-4z", fill: "ink", w: 0 },
      { d: "M50 22V8", fill: "none", w: 1.6 },
      { d: "M50 8L62 12L50 16Z", fill: "hot", w: 1.2 },
      { d: "M26 90H74", fill: "none", w: 2 },
    ],
  },
  {
    key: "ruins",
    name: "Ruins",
    category: "settlements",
    shapes: [
      { d: "M10 86V58L18 50L24 60L32 46V86Z", fill: "paper" },
      { d: "M42 86V64L50 58V86Z", fill: "accent" },
      { d: "M58 86V40H74L78 48L72 54V86Z", fill: "paper" },
      { d: "M80 86V70L90 66V86Z", fill: "accent" },
      { d: "M34 86l4-5 5 5zM76 86l3-4 5 4z", fill: "accent", w: 1.4 },
      { d: "M6 86H94", fill: "none", w: 2 },
    ],
  },
  {
    key: "port",
    name: "Port",
    category: "settlements",
    shapes: [
      { d: "M50 14a8 8 0 1 1-0.1 0Z", fill: "none", w: 4 },
      { d: "M50 30V78", fill: "none", w: 4.5 },
      { d: "M36 42H64", fill: "none", w: 4.5 },
      { d: "M22 60Q26 82 50 82Q74 82 78 60", fill: "none", w: 4.5 },
      { d: "M14 68L22 58L30 68Z", fill: "ink", w: 1 },
      { d: "M70 68L78 58L86 68Z", fill: "ink", w: 1 },
      { d: "M8 92Q18 86 28 92T48 92T68 92T88 92", fill: "none", w: 2 },
    ],
  },
  // ---- nature ----
  {
    key: "mountain",
    name: "Mountain",
    category: "nature",
    shapes: [
      { d: "M4 86L36 22L52 52L64 34L96 86Z", fill: "paper" },
      { d: "M36 22L52 52L44 64L50 86H30Z", fill: "accent", w: 1.4 },
      { d: "M64 34L96 86H74L68 62Z", fill: "accent", w: 1.4 },
      { d: "M36 22L27 40L33 37L38 43L44 38L52 52Z", fill: "white", w: 1.6 },
      { d: "M64 34L58 46L63 44L68 50Z", fill: "white", w: 1.4 },
    ],
  },
  {
    key: "hills",
    name: "Hills",
    category: "nature",
    shapes: [
      { d: "M6 82C10 46 40 46 48 82Z", fill: "paper" },
      { d: "M32 82C40 30 80 30 94 82Z", fill: "paper" },
      { d: "M18 58l4 8M24 54l4 10M58 48l4 10M66 44l4 12M74 48l3 8", fill: "none", w: 1.6 },
    ],
  },
  {
    key: "forest",
    name: "Forest",
    category: "nature",
    shapes: trees([
      [28, 86, 17],
      [72, 86, 17],
      [50, 78, 19],
    ]),
  },
  {
    key: "pine-forest",
    name: "Pine forest",
    category: "nature",
    shapes: pines([
      [24, 84, 46],
      [76, 84, 46],
      [50, 88, 62],
    ]),
  },
  {
    key: "swamp",
    name: "Swamp",
    category: "nature",
    shapes: [
      { d: "M8 78Q18 72 28 78T48 78T68 78T92 78", fill: "none", w: 2 },
      { d: "M20 88Q28 84 36 88T52 88T70 88", fill: "none", w: 1.6 },
      { d: "M30 74L27 38M50 74L52 30M70 74L74 42", fill: "none", w: 1.8 },
      {
        d: "M27 50a3.6 8.5 0 1 0 0.1 0ZM52 42a3.6 8.5 0 1 0 0.1 0ZM74 54a3.6 8.5 0 1 0 0.1 0Z",
        fill: "accent",
        w: 1.6,
      },
      { d: "M38 78l-8-14M62 78l8-12", fill: "none", w: 1.6 },
    ],
  },
  {
    key: "volcano",
    name: "Volcano",
    category: "nature",
    shapes: [
      { d: "M6 88L38 36Q50 44 62 36L94 88Z", fill: "paper" },
      { d: "M38 36Q50 44 62 36L58 30Q50 36 42 30Z", fill: "hot", w: 1.6 },
      { d: "M46 42L43 56L48 62L45 78", fill: "none", w: 2.4 },
      { d: "M56 42L59 58L55 70", fill: "none", w: 2.4 },
      {
        d: "M40 24a7 7 0 1 1 0.1 0ZM53 14a8 8 0 1 1 0.1 0ZM64 24a6 6 0 1 1 0.1 0Z",
        fill: "white",
        w: 1.6,
      },
    ],
  },
  // ---- biomes (cold, arid, tropical, volcanic) ----
  {
    key: "snowy-mountain",
    name: "Snowy mountain",
    category: "biomes",
    shapes: [
      { d: "M4 86L44 16L64 48L74 36L96 86Z", fill: "paper" },
      { d: "M64 48L56 62L62 86H42L50 62L53 46Z", fill: "accent", w: 1.4 },
      { d: "M74 36L96 86H80L76 62Z", fill: "accent", w: 1.4 },
      { d: "M44 16L29 42L36 38L41 46L47 39L53 46L58 40L64 48Z", fill: "white", w: 1.6 },
      { d: "M74 36L67 46L71 44L75 49L80 49Z", fill: "white", w: 1.4 },
    ],
  },
  {
    key: "glacier",
    name: "Glacier",
    category: "biomes",
    shapes: [
      { d: "M4 86L14 30L30 18L40 40L34 86Z", fill: "paper" },
      { d: "M96 86L88 28L72 16L60 38L66 86Z", fill: "paper" },
      { d: "M14 30L22 46L18 86H4ZM88 28L80 44L84 86H96Z", fill: "accent", w: 1.4 },
      {
        d: "M38 14H62L64 40L72 62L80 86L70 80L62 88L52 80L42 88L32 80L22 86L30 62L36 40Z",
        fill: "white",
      },
      {
        d: "M44 30l4 6M54 26l-3 7M40 52l6 6M58 50l-5 7M46 68l5 5M60 66l-4 6",
        fill: "none",
        w: 1.4,
      },
    ],
  },
  {
    key: "ice-floe",
    name: "Ice floe",
    category: "biomes",
    shapes: [
      { d: "M4 76Q12 72 20 76T36 76M62 72Q70 68 78 72T94 72", fill: "none", w: 1.6 },
      { d: "M66 90Q74 86 82 90T96 90", fill: "none", w: 1.6 },
      { d: "M8 62L30 70L48 64V68L30 75L8 67Z", fill: "accent", w: 1.4 },
      { d: "M8 62L22 52L44 54L48 64L30 70Z", fill: "white" },
      { d: "M54 46L62 58L86 56L90 46V50L86 60L62 62L54 50Z", fill: "accent", w: 1.4 },
      { d: "M54 46L72 40L90 46L86 56L62 58Z", fill: "white" },
      { d: "M34 82L38 87L54 86L60 80V83L54 89L38 90L34 85Z", fill: "accent", w: 1.4 },
      { d: "M34 82L46 76L60 80L54 86L38 87Z", fill: "white" },
      { d: "M30 56l4 6M72 44l-3 6", fill: "none", w: 1.2 },
    ],
  },
  {
    key: "tundra",
    name: "Tundra",
    category: "biomes",
    shapes: [
      { d: "M8 60Q20 56 32 60M60 58Q74 54 90 58", fill: "none", w: 1.4 },
      { d: "M24 84Q24 74 32 76Q38 68 44 76Q50 76 50 84Z", fill: "accent" },
      { d: "M52 84Q56 74 70 74Q86 74 90 84Z", fill: "white", w: 1.6 },
      { d: "M10 84l-3-12M14 84V69M18 84l3-12M66 76l-2-8M70 75V65M74 76l2-8", fill: "none", w: 1.6 },
      { d: "M6 84H94", fill: "none", w: 2 },
    ],
  },
  {
    key: "dunes",
    name: "Dunes",
    category: "biomes",
    shapes: [
      { d: "M30 70Q50 34 70 50Q82 58 96 70Z", fill: "paper" },
      { d: "M50 47Q58 56 60 70H96Q82 58 70 50Q60 44 50 47Z", fill: "accent", w: 1.4 },
      { d: "M4 86Q22 50 46 66Q58 74 68 86Z", fill: "paper" },
      { d: "M24 63Q30 72 30 86H68Q58 74 46 66Q34 59 24 63Z", fill: "accent", w: 1.4 },
      { d: "M48 86Q66 60 84 70Q92 76 96 86Z", fill: "paper" },
      { d: "M12 80q6-4 12-2M58 80q6-4 12-2M36 58q5-3 9-1", fill: "none", w: 1.4 },
    ],
  },
  {
    key: "oasis",
    name: "Oasis",
    category: "biomes",
    shapes: [
      ...palm(32, 78, 40, -6, 0.8),
      ...palm(66, 76, 48, 6, 0.9),
      { d: "M20 81Q20 71 50 71Q80 71 80 81Q80 91 50 91Q20 91 20 81Z", fill: "accent", w: 2 },
      { d: "M34 80q6-3 12 0M54 85q5-2 10 0", fill: "none", w: 1.2 },
    ],
  },
  {
    key: "cactus",
    name: "Cactus",
    category: "biomes",
    shapes: [
      { d: "M42 62H32Q26 62 26 56V40Q26 34 31 34Q36 34 36 40V52H42Z", fill: "accent" },
      { d: "M58 54H68Q74 54 74 48V32Q74 26 69 26Q64 26 64 32V44H58Z", fill: "accent" },
      { d: "M42 88V30Q42 20 50 20Q58 20 58 30V88Z", fill: "accent" },
      { d: "M50 28V84M31 40V56M69 32V48", fill: "none", w: 1.1 },
      { d: "M20 88l3-3 4 3zM74 88l3-3 4 3z", fill: "paper", w: 1.4 },
      { d: "M14 88H86", fill: "none", w: 2 },
    ],
  },
  {
    key: "mesa",
    name: "Mesa",
    category: "biomes",
    shapes: [
      { d: "M6 86L18 72L24 42H76L82 72L94 86Z", fill: "paper" },
      { d: "M62 42H76L82 72L94 86H70L66 62Z", fill: "accent", w: 1.4 },
      { d: "M23 52H77M20.5 64H79.5", fill: "none", w: 1.4 },
      { d: "M12 80l4-2M84 80l-4-2", fill: "none", w: 1.4 },
      { d: "M4 86H96", fill: "none", w: 2 },
    ],
  },
  {
    key: "jungle",
    name: "Jungle",
    category: "biomes",
    shapes: [
      { d: "M8 62Q6 44 22 42Q26 26 42 30Q50 18 62 28Q78 24 80 40Q96 42 92 62Z", fill: "accent" },
      { d: "M20 64Q16 52 30 50Q36 40 48 46Q58 38 68 48Q82 46 82 62Z", fill: "paper" },
      { d: "M30 86V62H35V86ZM62 86V60H67V86Z", fill: "ink", w: 1 },
      { d: "M10 88Q6 66 24 58Q28 78 10 88Z", fill: "paper" },
      { d: "M90 88Q94 66 76 58Q72 78 90 88Z", fill: "paper" },
      { d: "M50 88Q34 70 48 50Q64 68 50 88Z", fill: "accent" },
      { d: "M10 88L22 62M90 88L78 62M50 88L48 54", fill: "none", w: 1.2 },
      { d: "M18 46q4 4 8 2M52 32q4 4 8 2M74 42q4 4 8 2", fill: "none", w: 1.4 },
    ],
  },
  {
    key: "palm",
    name: "Palm",
    category: "biomes",
    shapes: [...palm(50, 86, 58, 8, 1.3), { d: "M26 88Q50 78 74 88Z", fill: "paper" }],
  },
  {
    key: "lava-field",
    name: "Lava field",
    category: "biomes",
    shapes: [
      { d: "M4 70L20 56L48 52L78 54L96 66L92 84L60 90L24 88L6 82Z", fill: "ink" },
      { d: "M12 72L28 66L34 72L48 68L50 71L35 76L28 70L13 75Z", fill: "hot", w: 0 },
      { d: "M48 68L58 60L72 62L74 65L60 64L50 71Z", fill: "hot", w: 0 },
      { d: "M50 71L62 78L80 74L88 78L87 81L80 78L62 82L49 74Z", fill: "hot", w: 0 },
      { d: "M28 70L30 80L42 86L40 88L27 82L25 70Z", fill: "hot", w: 0 },
      { d: "M30 44a5 5 0 1 1 0.1 0ZM70 40a6 6 0 1 1 0.1 0Z", fill: "white", w: 1.4 },
    ],
  },
  // ---- other ----
  {
    key: "cave",
    name: "Cave",
    category: "other",
    shapes: [
      { d: "M6 88L16 52Q28 22 50 20Q72 22 84 52L94 88Z", fill: "paper" },
      { d: "M32 88V68Q32 50 50 50Q68 50 68 68V88Z", fill: "ink", w: 1.4 },
      { d: "M24 42l6 8M70 36l-6 10M80 62l-8 4M20 66l8 2", fill: "none", w: 1.6 },
      { d: "M38 52l3 8 3-8M54 52l3 7 3-7", fill: "white", w: 1.2 },
    ],
  },
  {
    key: "shrine",
    name: "Shrine",
    category: "other",
    shapes: [
      { d: "M12 38L50 14L88 38Z", fill: "accent" },
      { d: "M16 38H84V45H16Z", fill: "paper" },
      { d: "M22 45h7v31h-7zM36 45h7v31h-7zM57 45h7v31h-7zM71 45h7v31h-7z", fill: "paper" },
      { d: "M12 76H88V83H12Z", fill: "paper" },
      { d: "M8 83H92V90H8Z", fill: "paper" },
      { d: "M50 22v8", fill: "none", w: 2 },
    ],
  },
  {
    key: "bridge",
    name: "Bridge",
    category: "other",
    shapes: [
      { d: "M4 46H96V56H84V80H64Q64 62 50 62Q36 62 36 80H16V56H4Z", fill: "paper" },
      { d: "M4 38H96M16 38V46M34 38V46M50 38V46M66 38V46M84 38V46", fill: "none", w: 1.8 },
      { d: "M4 90Q14 84 24 90T44 90T64 90T84 90T96 90", fill: "none", w: 2 },
    ],
  },
  {
    key: "camp",
    name: "Camp",
    category: "other",
    shapes: [
      { d: "M6 86L34 30L62 86Z", fill: "accent" },
      { d: "M34 46L26 86H42Z", fill: "ink", w: 1.4 },
      { d: "M34 30V18", fill: "none", w: 1.6 },
      { d: "M34 18L46 21L34 25Z", fill: "hot", w: 1.2 },
      { d: "M70 88L90 84M72 84L88 88", fill: "none", w: 3 },
      { d: "M72 84Q67 72 76 60Q77 70 85 66Q91 76 86 84Z", fill: "hot", w: 1.6 },
    ],
  },
  {
    key: "ship",
    name: "Ship",
    category: "other",
    shapes: [
      { d: "M50 12V70", fill: "none", w: 2.6 },
      { d: "M53 16Q80 38 53 60Z", fill: "white" },
      { d: "M47 24Q26 40 47 56Z", fill: "white" },
      { d: "M50 12L63 16L50 21Z", fill: "hot", w: 1.2 },
      { d: "M12 68H88L76 84H24Z", fill: "accent" },
      { d: "M6 92Q16 86 26 92T46 92T66 92T86 92T96 92", fill: "none", w: 2 },
    ],
  },
  {
    key: "monster",
    name: "Sea serpent",
    category: "other",
    shapes: [
      { d: "M6 86Q14 52 32 86Z", fill: "accent" },
      { d: "M34 86Q42 58 56 86Z", fill: "accent" },
      { d: "M58 86Q56 50 66 34L76 38Q64 54 72 86Z", fill: "paper" },
      { d: "M64 32Q78 18 94 30L84 36L93 41Q76 46 66 38Z", fill: "paper" },
      { d: "M78 31a1.8 1.8 0 1 0 0.1 0Z", fill: "ink", w: 1 },
      { d: "M4 90Q14 84 24 90T44 90T64 90T84 90T96 90", fill: "none", w: 2 },
    ],
  },
];

const BY_KEY = new Map(STAMPS.map((s) => [s.key, s]));

/** The stamp for an icon key, or `undefined` for an unknown (renamed / removed) key. */
export function stampByKey(key: string): StampDef | undefined {
  return BY_KEY.get(key);
}

/** Stamps of one category, in palette order. */
export function stampsIn(category: StampCategory): StampDef[] {
  return STAMPS.filter((s) => s.category === category);
}

/** Display name for an icon key (unknown keys show the key itself). */
export function stampName(key: string): string {
  return BY_KEY.get(key)?.name ?? key;
}

/** Scale a new stamp starts with: bigger maps get bigger stamps so they stay legible. */
export function defaultStampScale(mapWidth: number): number {
  const scale = Math.max(0.5, mapWidth / 1600);
  return Math.round(Math.min(scale, 8) * 10) / 10;
}

export const MIN_STAMP_SCALE = 0.2;
export const MAX_STAMP_SCALE = 10;
