import { app } from '../state.svelte';
import { DICTS } from './dicts';
import { lookup } from './langs';

export { LANGS } from './dicts';

// Clave ausente en todos los idiomas: devuelve la clave. {nombre} se sustituye por params.nombre.
export function t(key: string, params?: Record<string, string | number>): string {
  const text = lookup(DICTS, app.lang, key);
  if (text === undefined) return key;
  return params ? text.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m)) : text;
}
