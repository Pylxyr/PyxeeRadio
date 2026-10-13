// Colour styles ("skins") for the dashboard, like the skins of a desktop music player. A style sets
// the accent and tints the surfaces; light and dark mode still apply on top of it. The base palettes
// live in app.css; a style only overrides the variables listed in skinVariables() and is applied as
// inline custom properties on <html> (see theme.svelte.ts), so no colour is duplicated in the stylesheet.
import skinNames from '../../shared/skin-names.json';

export type Mode = 'dark' | 'light';
export type Rgb = [number, number, number];

interface Accent {
  accent: string;
  hi: string;
  text: string;
}
interface Skin {
  label: string;
  /** Tint of the surfaces: hue 0-360 and saturation 0-1. Absent for the default palette. */
  h?: number;
  s?: number;
  black?: boolean;
  dark?: Accent;
  /** `null`: use the default light look. */
  light?: Accent | null;
}

const DEFAULT_SURFACES: Record<Mode, { bg: string; panel: string; accent: string }> = {
  dark: { bg: '#0b0b10', panel: '#13131b', accent: '#9146ff' },
  light: { bg: '#f3f3f8', panel: '#ffffff', accent: '#8a3ffc' },
};

export const SKINS: Record<string, Skin> = {
  default: { label: 'Twitch purple' },
  midnight: {
    label: 'Midnight', h: 222, s: 0.3,
    dark: { accent: '#4f8cff', hi: '#7aa8ff', text: '#c6d8ff' },
    light: { accent: '#2f6fed', hi: '#1f56c9', text: '#1e40af' },
  },
  ocean: {
    label: 'Ocean', h: 190, s: 0.28,
    dark: { accent: '#0f9fae', hi: '#3fd2de', text: '#b4f1f6' },
    light: { accent: '#0e8f9b', hi: '#0b7280', text: '#0b5560' },
  },
  forest: {
    label: 'Forest', h: 150, s: 0.22,
    dark: { accent: '#2a9d5f', hi: '#52c987', text: '#bdf0d3' },
    light: { accent: '#1f8a52', hi: '#176f41', text: '#14532d' },
  },
  sunset: {
    label: 'Sunset', h: 20, s: 0.25,
    dark: { accent: '#d95a14', hi: '#ff8a4c', text: '#ffd2b8' },
    light: { accent: '#c74f0c', hi: '#a63f08', text: '#8a3408' },
  },
  rose: {
    label: 'Rose', h: 335, s: 0.25,
    dark: { accent: '#d63d73', hi: '#f06b99', text: '#fbc2d6' },
    light: { accent: '#c92b64', hi: '#ad2255', text: '#8a1a44' },
  },
  graphite: {
    label: 'Graphite', h: 220, s: 0.05,
    dark: { accent: '#6b7a90', hi: '#8795ab', text: '#d5dbe6' },
    light: { accent: '#55657c', hi: '#435166', text: '#2f3a4b' },
  },
  amoled: {
    label: 'AMOLED black', h: 260, s: 0.06, black: true,
    dark: { accent: '#9146ff', hi: '#ae7bff', text: '#d9c4ff' },
    light: null,
  },
};

/** The names, in display order: the same file the shell checks what the page sends against. */
export const SKIN_NAMES: string[] = skinNames;

export const isSkin = (name: unknown): name is string => typeof name === 'string' && Object.hasOwn(SKINS, name);

export function hslToRgb(h: number, s: number, l: number): Rgb {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [f(0), f(8), f(4)].map((v) => Math.round(v * 255)) as Rgb;
}
export const toHex = (rgb: Rgb) => `#${rgb.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
const surface = (h: number, s: number, lightness: number) => toHex(hslToRgb(h, s, lightness / 100));
const parts = (hex: string): Rgb => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16)) as Rgb;
const rgba = (hex: string, alpha: number) => `rgba(${parts(hex).join(',')},${alpha})`;

export function luminanceOf(hex: string): number {
  const [r, g, b] = parts(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
/** WCAG contrast ratio of two #rrggbb colours (1 to 21). */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminanceOf(a), luminanceOf(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

function hexToHsl(hex: string): [number, number, number] {
  const [r, g, b] = parts(hex).map((v) => v / 255) as Rgb;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l];
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return [(h * 60 + 360) % 360, s, l];
}

/** The accent, darkened just enough that white text on it reads (WCAG AA for normal text, 4.5:1). */
export function fillForWhiteText(accent: string, minimum = 4.5): string {
  const [h, s, l0] = hexToHsl(accent);
  let l = l0;
  let fill = accent;
  while (contrast('#ffffff', fill) < minimum && l > 0.05) {
    l -= 0.005;
    fill = toHex(hslToRgb(h, s, l));
  }
  return fill;
}
/** A step darker than `fill`: a hover state that keeps (never lowers) the contrast of the white label. */
export function pressedFill(fill: string): string {
  const [h, s, l] = hexToHsl(fill);
  return toHex(hslToRgb(h, s, Math.max(0, l - 0.06)));
}

/** The custom properties a style overrides for 'dark' or 'light' ({} when the base palette applies). */
export function skinVariables(name: unknown, mode: Mode): Record<string, string> {
  const skin = SKINS[isSkin(name) ? name : 'default'];
  const accent = skin?.[mode];
  if (!skin || !accent || skin.h === undefined || skin.s === undefined) return {};
  const dark = mode === 'dark';
  const h = skin.h;
  const buttonFill = fillForWhiteText(accent.accent);
  const s = dark ? Math.min(0.6, skin.s * 1.4) : skin.s * 0.8;
  // lightness of: bg, panel, panel2, line, hover, hover line, log background, toast
  const L = dark
    ? skin.black ? [0, 4, 7, 13, 10, 18, 2, 8] : [6, 10, 13.5, 20, 17, 25, 7, 14]
    : [96, 100, 94, 88, 91, 82, 99, 100];
  const [bg, panel, panel2, line, hover, hoverLine, logBg, toast] = L.map((l) => surface(h, s, l)) as [
    string, string, string, string, string, string, string, string,
  ];
  return {
    '--bg': bg, '--panel': panel, '--panel2': panel2, '--line2': line,
    '--btn-hover': hover, '--btn-hover-line': hoverLine,
    '--accent': accent.accent, '--accent-hi': accent.hi,
    '--btn-accent': buttonFill, '--btn-accent-hover': pressedFill(buttonFill),
    '--accent-soft': rgba(accent.accent, dark ? 0.16 : 0.12),
    '--accent-text': accent.text, '--code': accent.text,
    '--accent-line': rgba(accent.accent, dark ? 0.45 : 0.4),
    '--accent-faint': rgba(accent.accent, dark ? 0.09 : 0.07),
    '--mark-bg': rgba(accent.accent, dark ? 0.45 : 0.28),
    '--savebar': rgba(panel, 0.96), '--toast-bg': toast, '--log-bg': logBg,
  };
}

/** The page's background colour in a style and mode: what the window itself is painted with before the page loads. */
export function backgroundOf(name: unknown, mode: Mode): string {
  return skinVariables(name, mode)['--bg'] ?? DEFAULT_SURFACES[mode].bg;
}

/** Three colours to preview a style in a picker. */
export function preview(name: unknown, mode: Mode): { bg: string; panel: string; accent: string } {
  const vars = skinVariables(name, mode);
  const base = DEFAULT_SURFACES[mode];
  return { bg: vars['--bg'] ?? base.bg, panel: vars['--panel'] ?? base.panel, accent: vars['--accent'] ?? base.accent };
}
