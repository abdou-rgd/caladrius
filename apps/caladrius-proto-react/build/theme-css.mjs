// theme.json (crates/caladrius-ui/src/theme.json, the tokens of the egui interface) turned into CSS
// variables at build time. Nothing is copied by hand: a change of the file changes the next build.
// light is the default, dark follows the system, and `data-theme` on <html> forces one (the screenshots).

const px = (v) => `${v}px`;

/** The declarations of one theme, as `--name: value;` lines. */
export function declarations(theme) {
  const out = [];
  for (const [name, value] of Object.entries(theme.colors)) {
    if (Array.isArray(value)) value.forEach((c, i) => out.push([`--c-${name.replace(/_/g, '-')}-${i}`, c]));
    else out.push([`--c-${name.replace(/_/g, '-')}`, value]);
  }
  for (const [name, value] of Object.entries(theme.radius)) out.push([`--radius-${name}`, px(value)]);
  for (const [name, value] of Object.entries(theme.spacing)) out.push([`--space-${name.replace(/_/g, '-')}`, px(value)]);
  for (const [name, value] of Object.entries(theme.size)) out.push([`--size-${name.replace(/_/g, '-')}`, px(value)]);
  for (const [name, value] of Object.entries(theme.stroke)) out.push([`--stroke-${name}`, px(value)]);
  for (const [name, value] of Object.entries(theme.font)) out.push([`--font-${name}`, px(value)]);
  return out.map(([k, v]) => `  ${k}: ${v};`).join('\n');
}

/** The style sheet: light, then dark by preference, then the explicit choices. */
export function themeCss(tokens) {
  const light = declarations(tokens.light);
  const dark = declarations(tokens.dark);
  return [
    `:root {\n  color-scheme: light dark;\n${light}\n}`,
    `@media (prefers-color-scheme: dark) {\n  :root:not([data-theme="light"]) {\n    color-scheme: dark;\n${dark.replace(/^/gm, '  ')}\n  }\n}`,
    `:root[data-theme="light"] {\n  color-scheme: light;\n${light}\n}`,
    `:root[data-theme="dark"] {\n  color-scheme: dark;\n${dark}\n}`,
  ].join('\n');
}
