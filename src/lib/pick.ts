// Atajos de seleccion de Limpieza: solo dicen que marcar; la seleccion, plan() y la confirmacion siguen siendo las
// puertas del borrado. Puro: check-frontend.mjs lo carga desde Node.
import type { CategoryInfo, Found } from './api';
import { impactOf, type Impact } from './impact';

// Nunca entra lo que pide revisar una a una (nivel careful), ni worktrees, contenedores o informativas.
const pickable = (c: CategoryInfo) => impactOf(c.id) !== 'careful';

// Lo que marca la casilla de un grupo: sus cachés enteras con algo dentro y los elementos de sus categorias de proyecto
// y de subcarpetas.
export function groupTargets(cats: CategoryInfo[], ecosystem: string): { categories: string[]; items: Record<string, string[]> } {
  const mine = cats.filter((c) => c.ecosystem === ecosystem && pickable(c));
  const items: Record<string, string[]> = {};
  for (const c of mine) if ((c.kind === 'project' || c.kind === 'subfolders') && c.items.length) items[c.id] = c.items.map((f) => f.path);
  return { categories: mine.filter((c) => c.kind === 'contents' && c.size > 0).map((c) => c.id), items };
}

// Seleccion rapida por consecuencia: solo cachés enteras (tipo Contenido) con algo dentro. 'redownload' incluye 'auto'.
export function quickPick(level: 'auto' | 'redownload', cats: CategoryInfo[]): string[] {
  const ok: Impact[] = level === 'auto' ? ['auto'] : ['auto', 'redownload'];
  return cats.filter((c) => c.kind === 'contents' && c.size > 0 && ok.includes(impactOf(c.id))).map((c) => c.id);
}

// Elementos con fecha conocida y mas viejos que `days` (estrictamente); sin fecha nunca entran.
export const staleItems = (items: Found[], days: number, nowMs: number): string[] =>
  items.filter((f) => f.modified != null && nowMs - f.modified * 1000 > days * 86_400_000).map((f) => f.path);
