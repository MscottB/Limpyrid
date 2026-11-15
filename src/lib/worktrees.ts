// Logica pura de las filas de worktrees: el script de comprobacion la carga desde Node.
import type { Worktree } from './api';

// Sin ningun motivo de proteccion = se puede quitar.
export const removable = (w: Worktree): boolean => w.protection.length === 0;

// Primero los que se pueden quitar, luego por tamano descendente. No muta la lista.
export const sortWorktrees = (list: Worktree[]): Worktree[] =>
  [...list].sort((a, b) => Number(removable(b)) - Number(removable(a)) || b.size - a.size);
