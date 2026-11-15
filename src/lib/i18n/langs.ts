// Puro (sin import.meta ni estado): el script de comprobacion lo carga desde Node.
export const FALLBACK = 'en';

// El guardado si hay archivo; si no, el del sistema (etiqueta completa, luego solo el idioma); si no, ingles.
export function pickLang(saved: string | null, system: string, available: string[]): string {
  const find = (tag: string) => available.find((a) => a.toLowerCase() === tag.toLowerCase());
  return (saved && find(saved)) || find(system) || find(system.split('-')[0]) || FALLBACK;
}

// Clave ausente en el idioma: la de ingles; ausente tambien alli: undefined.
export function lookup(dicts: Record<string, Record<string, string>>, lang: string, key: string): string | undefined {
  return dicts[lang]?.[key] ?? dicts[FALLBACK]?.[key];
}
