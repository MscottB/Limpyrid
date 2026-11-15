// Seleccion de Limpieza y la limpieza en un solo paso. Vive fuera de los componentes: sobrevive al cambio de pestana y
// Resumen la lee para "Recuperas hasta".
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import * as api from './api';
import type { CategoryInfo, Worktree } from './api';
import { dataRev, work } from './state.svelte';
import { plan, type PlanEntry } from './recovery';

export const selection: { categories: Set<string>; items: Map<string, Set<string>> } = {
  categories: new SvelteSet<string>(),
  items: new SvelteMap<string, Set<string>>(),
};

export const selectedBytes = (cats: CategoryInfo[], worktrees: Worktree[] | null): number =>
  plan(cats, worktrees, selection).reduce((a, e) => a + e.size, 0);

// El plan con los datos cargados ahora mismo: lo unico que se muestra en la confirmacion y lo unico que se envia.
export const currentPlan = (): PlanEntry[] => plan(work.cats ?? [], work.wt.list?.worktrees ?? null, selection);

export function setCategory(id: string, on: boolean) {
  if (on) selection.categories.add(id);
  else selection.categories.delete(id);
}

export function setItems(id: string, paths: string[], on: boolean) {
  let set = selection.items.get(id);
  if (!set) selection.items.set(id, (set = new SvelteSet<string>()));
  for (const p of paths) {
    if (on) set.add(p);
    else set.delete(p);
  }
}

export const isPicked = (id: string, path: string) => !!selection.items.get(id)?.has(path);

export function clearSelection() {
  selection.categories.clear();
  selection.items.clear();
}

// Tras recargar datos, lo marcado que ya no existe o ya no se puede seleccionar se desmarca (y no reaparece marcado si
// vuelve). Los worktrees solo se depuran con su lista cargada.
export function pruneSelection() {
  const keep = currentPlan();
  const kept = new Map(keep.map((e) => [e.id, new Set(e.items.map((i) => i.path))]));
  if (work.cats) for (const id of [...selection.categories]) if (!kept.has(id)) selection.categories.delete(id);
  for (const [id, set] of selection.items) {
    const kind = work.cats?.find((c) => c.id === id)?.kind;
    if (!work.cats || (kind === 'worktrees' && !work.wt.list)) continue;
    for (const p of [...set]) if (!kept.get(id)?.has(p)) set.delete(p);
  }
}

// Ejecuta el plan categoria por categoria, con la antiguedad elegida en cada llamada. Se detiene en el primer comando
// que falla y el informe junta lo que llego a correr. El backend vuelve a validar cada ruta. Las dos funciones de la API
// se pueden pasar (scripts/check-frontend.mjs lo prueba en Node con sustitutos).
// Mientras corre, work.progress sigue los eventos clean-progress de la categoria en curso; cada categoria que termina
// queda en work.cleaned con su propio informe.
export async function runPlan(entries: PlanEntry[], minAge: number, clean = api.cleanCategory, removeWorktrees = api.removeWorktrees) {
  if (work.busy) return; // su propia guarda: no depende de que cada llamador la compruebe
  const results: { id: string; report: api.Report }[] = [];
  let error: { id: string; code: string } | undefined;
  let off: (() => void) | undefined;
  work.busy = true;
  work.outcome = null;
  try {
    // Sin Tauri listen() rechaza: la limpieza sigue, sin progreso en bytes.
    off = await api.onCleanProgress((p) => {
      if (work.progress?.id === p.id) Object.assign(work.progress, { freed: p.freed, deleted: p.deleted });
    }).catch(() => undefined);
    const startedAt = Date.now();
    const ids = entries.map((e) => e.id);
    for (const [n, e] of entries.entries()) {
      const paths = e.items.map((i) => i.path);
      // Tamano de la fila al pulsar (toda la categoria, no lo elegido), como lo muestra Limpieza.
      const c = work.cats?.find((x) => x.id === e.id);
      const row = { ecosystem: c?.ecosystem ?? '', size: e.kind === 'worktrees' ? (work.wt.list?.worktrees ?? []).reduce((a, w) => a + w.size, 0) : (c?.size ?? e.size) };
      work.progress = { index: n + 1, total: entries.length, id: e.id, freed: 0, deleted: 0, expected: e.size, startedAt, ids };
      try {
        const report = e.kind === 'worktrees' ? await removeWorktrees(paths, minAge) : await clean(e.id, paths, minAge);
        results.push({ id: e.id, report });
        const prev = work.cleaned[e.id];
        work.cleaned[e.id] = {
          freed: (prev?.freed ?? 0) + report.freed,
          deleted: (prev?.deleted ?? 0) + report.deleted,
          skipped: (prev?.skipped ?? 0) + report.skipped.length,
          before: prev?.before ?? row.size, // el primero: la fila no cambia de sitio
          kind: e.kind,
          ecosystem: row.ecosystem,
          paths: [...(prev?.paths ?? []), ...paths],
        };
      } catch (err) {
        error = { id: e.id, code: api.errCode(err) };
        break;
      }
    }
  } finally {
    off?.();
    work.progress = null;
    const notRun = entries.slice(results.length + (error ? 1 : 0)).map((e) => e.id);
    work.outcome = { kind: 'batch', results, error, notRun };
    work.busy = false;
    if (results.length) clearSelection(); // si nada corrio (p. ej. busy o scan_running) se conserva para reintentar
    dataRev.n++; // categorias, worktrees, unidades, Explorador y Mapa se recargan
  }
}
