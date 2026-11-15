// Aparte de index.ts para que state.svelte.ts pueda leer la lista de idiomas sin ciclo de imports.
const files = import.meta.glob<Record<string, string>>('./*.json', { eager: true, import: 'default' });

export const DICTS: Record<string, Record<string, string>> = Object.fromEntries(
  Object.entries(files).map(([path, d]) => [path.slice(2, -5), d]),
);

export const LANGS: { code: string; name: string }[] = Object.entries(DICTS)
  .map(([code, d]) => ({ code, name: d['lang.name'] ?? code })) // sin lang.name, el codigo: npm test lo exige, pero la app no debe caer
  .sort((a, b) => a.name.localeCompare(b.name));
