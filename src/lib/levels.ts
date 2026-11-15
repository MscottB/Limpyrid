// Logica pura de los niveles de consecuencia en Inicio. Solo imports de tipos de la API: Node no puede cargar api.ts.
import type { CategoryInfo, Worktree } from './api';
import { impactOf, type Impact } from './impact';
import { quickPick } from './pick';
import { navGroups } from './nav';
import { removable } from './worktrees';

// Mismos sumandos que groupTotals (recovery.ts).
const bytesOf = (c: CategoryInfo, worktrees: Worktree[]) =>
  c.kind === 'contents' || c.kind === 'project' || c.kind === 'subfolders' ? c.size
  : c.kind === 'worktrees' ? worktrees.filter(removable).reduce((a, w) => a + w.size, 0)
  : 0;

export function levelTotals(cats: CategoryInfo[], worktrees: Worktree[]): Record<Impact, number> {
  const out: Record<Impact, number> = { auto: 0, redownload: 0, rebuild: 0, careful: 0 };
  for (const c of cats) out[impactOf(c.id)] += bytesOf(c, worktrees);
  return out;
}

export function levelPick(level: 'auto' | 'redownload', cats: CategoryInfo[]): string[] {
  const picked = quickPick(level, cats);
  return level === 'auto' ? picked : picked.filter((id) => impactOf(id) !== 'auto');
}

export const levelHasItems = (level: Impact, cats: CategoryInfo[]): boolean =>
  cats.some((c) => (c.kind === 'project' || c.kind === 'subfolders') && impactOf(c.id) === level && c.items.length > 0);

// El primer grupo de la navegacion con algo que limpiar en ese nivel.
export function levelGroup(level: Impact, cats: CategoryInfo[], worktrees: Worktree[]): string | null {
  const has = (eco: string) => cats.some((c) => c.ecosystem === eco && impactOf(c.id) === level && bytesOf(c, worktrees) > 0);
  return navGroups(cats, worktrees).active.find((g) => has(g.ecosystem))?.ecosystem ?? null;
}
