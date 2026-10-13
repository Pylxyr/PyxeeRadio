import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import skinNames from '../../shared/skin-names.json';
import { SKIN_NAMES, SKINS, backgroundOf, contrast, isSkin, luminanceOf, skinVariables, type Mode } from '../lib/skins';
import { resolveMode } from '../lib/theme.svelte';

// Git checks files out with CRLF line endings on Windows (see .gitattributes); the patterns below use \n.
const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'app.css'), 'utf8').replace(/\r\n/g, '\n');

/** The `--name: value` pairs inside the block that starts at `opener`. */
function variablesIn(opener: string): Map<string, string> {
  const start = css.indexOf(opener);
  expect(start, `${opener} not found`).toBeGreaterThanOrEqual(0);
  let depth = 0;
  let end = start;
  for (let i = css.indexOf('{', start); i < css.length; i++) {
    if (css[i] === '{') depth++;
    if (css[i] === '}' && --depth === 0) {
      end = i;
      break;
    }
  }
  const vars = new Map<string, string>();
  for (const m of css.slice(start, end).matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) vars.set(m[1]!, m[2]!.trim());
  return vars;
}
const dark = variablesIn(':root {');
const light = variablesIn(':root[data-theme="light"] {');
const modes: Mode[] = ['dark', 'light'];

describe('the two palettes', () => {
  it('every colour variable of the dark palette has a light counterpart, and nothing extra', () => {
    const sizes = new Set(['--radius', '--font', '--mono']); // not colours
    const darkColours = [...dark.keys()].filter((k) => !sizes.has(k));
    expect(darkColours.length, 'the dark palette looks too small').toBeGreaterThan(40);
    expect([...light.keys()].sort()).toEqual(darkColours.sort());
  });

  it('the light palette is really light, the dark one really dark', () => {
    expect(luminanceOf(dark.get('--bg')!)).toBeLessThan(0.1);
    expect(luminanceOf(dark.get('--text')!)).toBeGreaterThan(0.7);
    expect(luminanceOf(light.get('--bg')!)).toBeGreaterThan(0.85);
    expect(luminanceOf(light.get('--text')!)).toBeLessThan(0.2);
    expect(luminanceOf(light.get('--panel')!)).toBeGreaterThanOrEqual(luminanceOf(light.get('--bg')!) - 0.001);
  });

  it('no colour literal is left outside the two palettes (so neither theme can be broken by a stray one)', () => {
    const stripped = css.replace(/:root \{[^}]*\}/, '').replace(/:root\[data-theme="light"\] \{[^}]*\}/, '');
    const literals = [...stripped.matchAll(/#[0-9a-fA-F]{3,8}\b|rgba?\([^)]*\)/g)].map((m) => m[0].replace(/\s+/g, ''));
    // Allowed: white text on the accent colour, the switch knob, the dark text on the "done" check, the
    // green pulse glow (reads on both backgrounds), plain black shadows and backdrops, and the red of
    // the window's close button, which is the same in light and dark.
    const allowed = new Set(['#fff', '#06251a', '#c42b1c', 'rgba(61,220,151,.6)', 'rgba(61,220,151,0)']);
    const stray = literals.filter((l) => !allowed.has(l) && !l.startsWith('rgba(0,0,0,'));
    expect(stray, `colours outside the palettes: ${stray.join(', ')}`).toEqual([]);
    for (const m of stripped.matchAll(/var\((--[a-z0-9-]+)\)/g)) expect(dark.has(m[1]!), `${m[1]} is used but not defined`).toBe(true);
  });

  it('white labels on buttons and badges read, and hover never lowers the contrast', () => {
    for (const [mode, vars] of [['dark', dark], ['light', light]] as const) {
      for (const key of ['--btn-accent', '--btn-accent-hover', '--err-fill']) {
        expect(contrast('#ffffff', vars.get(key)!), `${mode}: white on ${key}`).toBeGreaterThanOrEqual(4.5);
      }
      expect(contrast('#ffffff', vars.get('--btn-accent-hover')!), `${mode}: hover`).toBeGreaterThanOrEqual(contrast('#ffffff', vars.get('--btn-accent')!));
    }
  });

  it('muted text reads on the page, on cards and on raised surfaces in both palettes', () => {
    for (const [mode, vars] of [['dark', dark], ['light', light]] as const) {
      for (const surface of ['--bg', '--panel', '--panel2']) {
        expect(contrast(vars.get('--muted')!, vars.get(surface)!), `${mode}: muted on ${surface}`).toBeGreaterThanOrEqual(4.5);
      }
      expect(contrast(vars.get('--text')!, vars.get('--panel')!), `${mode}: text`).toBeGreaterThanOrEqual(7);
    }
  });
});

describe('colour styles', () => {
  it('the names the shell accepts are exactly the styles the page has, in the same order', () => {
    expect(Object.keys(SKINS)).toEqual(skinNames);
    expect(SKIN_NAMES).toEqual(skinNames);
  });

  it('the default changes nothing, the others only override known colour variables', () => {
    expect(skinVariables('default', 'dark')).toEqual({});
    expect(skinVariables('default', 'light')).toEqual({});
    expect(skinVariables('no-such-style', 'dark')).toEqual({}); // an unknown name falls back to the default
    expect(SKIN_NAMES.length).toBeGreaterThanOrEqual(6);
    for (const name of SKIN_NAMES) {
      for (const mode of modes) {
        for (const key of Object.keys(skinVariables(name, mode))) expect(dark.has(key), `${name}/${mode}: ${key} is not a palette variable`).toBe(true);
      }
    }
  });

  it('every style stays really dark in dark mode and really light in light mode, and its text reads', () => {
    for (const name of SKIN_NAMES) {
      for (const mode of modes) {
        const vars = skinVariables(name, mode);
        if (!Object.keys(vars).length) continue;
        if (mode === 'dark') {
          expect(luminanceOf(vars['--bg']!), `${name} dark`).toBeLessThan(0.1);
          expect(luminanceOf(vars['--panel']!), `${name} dark`).toBeLessThan(0.15);
        } else {
          expect(luminanceOf(vars['--bg']!), `${name} light`).toBeGreaterThan(0.85);
          expect(luminanceOf(vars['--panel']!), `${name} light`).toBeGreaterThan(0.9);
        }
        expect(contrast('#ffffff', vars['--btn-accent']!), `${name}/${mode}: primary button`).toBeGreaterThanOrEqual(4.5);
        expect(contrast('#ffffff', vars['--btn-accent-hover']!), `${name}/${mode}: hovered primary button`).toBeGreaterThanOrEqual(4.5);
        expect(contrast(vars['--accent-text']!, vars['--panel']!), `${name}/${mode}: accent text on a card`).toBeGreaterThanOrEqual(4.5);
        const base = mode === 'dark' ? dark : light;
        expect(contrast(base.get('--muted')!, vars['--panel2']!), `${name}/${mode}: muted text on raised surfaces`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it('the colour the window is painted with is the page background, in every style and mode', () => {
    for (const name of SKIN_NAMES) {
      for (const [mode, base] of [['dark', dark], ['light', light]] as const) {
        const expected = (skinVariables(name, mode)['--bg'] ?? base.get('--bg')!).toLowerCase();
        expect(backgroundOf(name, mode).toLowerCase(), `${name}/${mode}`).toBe(expected);
      }
    }
  });

  it('isSkin refuses anything that is not a style name', () => {
    for (const bad of ['', 'Midnight', 'constructor', '__proto__', 'toString', 3, null, undefined]) expect(isSkin(bad)).toBe(false);
    for (const name of SKIN_NAMES) expect(isSkin(name)).toBe(true);
  });
});

describe('light, dark, system', () => {
  it('an explicit choice wins, "system" follows the OS', () => {
    expect(resolveMode('light', true)).toBe('light');
    expect(resolveMode('dark', false)).toBe('dark');
    expect(resolveMode('system', true)).toBe('dark');
    expect(resolveMode('system', false)).toBe('light');
  });
});
