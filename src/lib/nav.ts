// Logica pura de la navegacion de la mesa de trabajo: que grupos se listan y que foco sigue siendo valido.
// Solo imports de tipos de la API: Node no puede cargar api.ts.
import type { CategoryInfo, Worktree } from './api';
import { groupTotals } from './recovery';

export interface Focus { id: string; path: string | null }

// Con datos primero (los de groupTotals), luego los que solo muestran contenedores o worktrees (aunque esten protegidos),
// con bytes 0; el resto de ecosistemas, vacios, por orden alfabetico.
export function navGroups(cats: CategoryInfo[], worktrees: Worktree[]): { active: { ecosystem: string; bytes: number }[]; empty: string[] } {
  const active = groupTotals(cats, worktrees);
  const seen = new Set(active.map((g) => g.ecosystem));
  for (const c of cats) {
    const shows = ((c.kind === 'containers' || c.kind === 'info_only') && c.size > 0) || (c.kind === 'worktrees' && worktrees.length > 0);
    if (shows && !seen.has(c.ecosystem)) {
      seen.add(c.ecosystem);
      active.push({ ecosystem: c.ecosystem, bytes: 0 });
    }
  }
  const empty = [...new Set(cats.map((c) => c.ecosystem))].filter((e) => !seen.has(e)).sort();
  return { active, empty };
}

// La barra de accion, con el mismo criterio que la navegacion: un escaneo cancelado o fallido deja los datos anteriores.
export const barVisible = (root: string, status: 'idle' | 'running' | 'done', busy: boolean): boolean => (!!root && status !== 'running') || busy;

// Progreso de un escaneo de la unidad entera: bytes leidos frente a lo ocupado. null si no se conoce el total.
export function scanPercent(bytes: number, drive: { root: string; total: number; free: number } | undefined, root: string): number | null {
  const key = (p: string) => p.replace(/\\$/, '').toLowerCase();
  if (!drive || key(drive.root) !== key(root) || drive.total - drive.free <= 0) return null;
  return Math.min(99, Math.floor((bytes / (drive.total - drive.free)) * 100));
}

export const resolveGroup = (group: string | null, nav: { active: { ecosystem: string }[]; empty: string[] }): string | null =>
  group != null && (nav.active.some((g) => g.ecosystem === group) || nav.empty.includes(group)) ? group : null;

// El foco solo vale si su categoria sigue en el grupo y su carpeta o worktree sigue en los datos.
export function validFocus(focus: Focus | null, group: string | null, cats: CategoryInfo[], worktrees: Worktree[]): Focus | null {
  const c = focus && cats.find((x) => x.id === focus.id);
  if (!focus || !c || c.ecosystem !== group) return null;
  if (focus.path === null) return focus;
  const there = c.items.some((i) => i.path === focus.path) || (c.kind === 'worktrees' && worktrees.some((w) => w.path === focus.path));
  return there ? focus : null;
}
