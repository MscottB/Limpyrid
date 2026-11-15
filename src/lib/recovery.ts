// Logica pura del resumen de recuperacion y del plan de limpieza: el script de comprobacion la carga desde Node.
// Solo imports de tipos: Node carga este archivo sin resolver modulos sin extension.
import type { CategoryInfo, CategoryKind, Drive, Worktree } from './api';

// La unidad que contiene la ruta escaneada (sin distinguir mayusculas; la raiz de una unidad termina en "\").
// Un "\\?\" delante de una letra de unidad se quita ("\\?\C:\x" es C:\x); "\\?\UNC\..." no tiene unidad.
const norm = (p: string) => p.replaceAll('/', '\\').replace(/^\\\\\?\\(?=[a-z]:)/i, '').toLowerCase();
export const driveOf = (root: string, drives: Drive[]): Drive | undefined => {
  const r = norm(root) + (root.endsWith('\\') || root.endsWith('/') ? '' : '\\');
  return drives.find((d) => r.startsWith(norm(d.root).replace(/\\?$/, '\\')));
};
// Igual que removable() de worktrees.ts: sin motivo de proteccion.
const removable = (w: Worktree) => w.protection.length === 0;

// Lo seleccionado nunca libera mas de lo que la unidad tiene ocupado.
export function recovery(drive: Drive, selectedBytes: number) {
  const used = Math.max(0, drive.total - drive.free);
  const recover = Math.min(Math.max(0, selectedBytes), used);
  return { freeNow: drive.free, recover, freeAfter: drive.free + recover, usedAfter: used - recover };
}

// fixed: se escanea; extraible o USB: con confirmacion; red, optica, desconocida o sin tamano: no.
export function selectable(d: Drive): 'yes' | 'confirm' | 'no' {
  if (d.total === 0 || d.kind === 'network' || d.kind === 'optical' || d.kind === 'unknown') return 'no';
  return d.kind === 'removable' || d.usb ? 'confirm' : 'yes';
}
// Clave del texto que explica por que una unidad no se puede elegir.
export const noReason = (d: Drive) => (d.kind === 'network' || d.kind === 'optical' || d.kind === 'unknown' ? 'drive.no.' + d.kind : 'drive.no.empty');

// "Otra carpeta": se escanea solo si la ruta cae en una unidad de la lista cargada que se puede elegir. Una ruta UNC,
// relativa, en una letra desconocida o con la lista sin cargar se rechaza. `reason`: clave del texto que lo explica.
export function folderRule(path: string, drives: Drive[] | null): { rule: 'yes' | 'confirm' | 'no'; reason: string | null; drive?: Drive } {
  if (!path.trim()) return { rule: 'no', reason: null };
  if (!drives) return { rule: 'no', reason: 'drive.no.loading' };
  const drive = driveOf(path.trim(), drives);
  if (!drive) return { rule: 'no', reason: 'drive.no.folder' };
  const rule = selectable(drive);
  return { rule, reason: rule === 'no' ? noReason(drive) : rule === 'confirm' ? 'drive.confirmNote' : null, drive };
}

// Worktrees del plan clasificados con otra antiguedad que la elegida: no se ofrece limpiar hasta recargar la lista.
export const wtStale = (entries: PlanEntry[], wt: { list: unknown; age: number }, minAge: number) =>
  entries.some((e) => e.kind === 'worktrees') && wt.list != null && wt.age !== minAge;

// Lo que se puede limpiar, sumado por ecosistema: cachés, carpetas de proyecto y subcarpetas enteras, y los worktrees
// que se pueden quitar (bajo el ecosistema de su categoria). De mayor a menor, sin los grupos en cero.
export function groupTotals(cats: CategoryInfo[], worktrees: Worktree[]): { ecosystem: string; bytes: number }[] {
  const by = new Map<string, number>();
  for (const c of cats) {
    const bytes =
      c.kind === 'contents' || c.kind === 'project' || c.kind === 'subfolders' ? c.size
      : c.kind === 'worktrees' ? worktrees.filter(removable).reduce((a, w) => a + w.size, 0)
      : 0;
    if (bytes > 0) by.set(c.ecosystem, (by.get(c.ecosystem) ?? 0) + bytes);
  }
  return [...by].map(([ecosystem, bytes]) => ({ ecosystem, bytes })).sort((a, b) => b.bytes - a.bytes);
}

// Los `n` mayores y, si sobran grupos, un resto (ecosystem null) con la suma de los demas. Espera la lista ya ordenada.
export function topGroups(totals: { ecosystem: string; bytes: number }[], n: number): { ecosystem: string | null; bytes: number }[] {
  if (totals.length <= n) return totals;
  return [...totals.slice(0, n), { ecosystem: null, bytes: totals.slice(n).reduce((a, g) => a + g.bytes, 0) }];
}

// Cuantos ecosistemas se nombran en las barras de Inicio; el resto va junto en "Otros".
export const TOP_GROUPS = 6;

// Referencia de las barras de magnitud de las filas: la fila mayor que no es de contenedores (Docker puede pesar 80 GB
// y aplanaria todas las demas). Una fila mayor que la referencia se dibuja llena.
export const magnitudeRef = (rows: { kind: CategoryKind; size: number }[]) =>
  Math.max(1, ...rows.filter((r) => r.kind !== 'containers').map((r) => r.size));

// Si hay que (re)cargar la lista de worktrees: hay categoria de worktrees, no hay escaneo en curso y la lista falta o se
// cargo con otra revision de datos u otra antiguedad minima.
export const wtNeedsLoad = (wt: { list: unknown; rev: number; age: number }, rev: number, minAge: number, scanning: boolean, hasCategory: boolean) =>
  hasCategory && !scanning && (wt.list == null || wt.rev !== rev || wt.age !== minAge);

export interface Selection {
  categories: Set<string>; // categorias de tipo Contenido marcadas enteras
  items: Map<string, Set<string>>; // id de categoria -> rutas marcadas (Proyecto, Subcarpetas, Worktrees)
}
export interface PlanItem { path: string; size: number }
export interface PlanEntry { id: string; kind: CategoryKind; usesMinAge: boolean; items: PlanItem[]; size: number }

// Bytes del plan que estan en la unidad `root`: una cache de C:\ no libera espacio en D:\. Las categorias de Contenido
// cuentan si alguna de sus carpetas esta en esa unidad.
export function bytesOn(entries: PlanEntry[], cats: CategoryInfo[], root: string): number {
  const on = (p: string) => driveOf(p, [{ root } as Drive]) !== undefined;
  let sum = 0;
  for (const e of entries) {
    if (e.kind === 'contents') sum += cats.find((c) => c.id === e.id)?.paths.some(on) ? e.size : 0;
    else sum += e.items.filter((i) => on(i.path)).reduce((a, i) => a + i.size, 0);
  }
  return sum;
}

// Lo que se enviaria con la seleccion actual, filtrado contra los datos cargados: una ruta que ya no esta o que ya
// no se puede seleccionar se descarta. Sin lista de worktrees cargada no entra ningun worktree.
export function plan(cats: CategoryInfo[], worktrees: Worktree[] | null, sel: Selection): PlanEntry[] {
  const out: PlanEntry[] = [];
  for (const c of cats) {
    const entry = (items: PlanItem[], size: number) => out.push({ id: c.id, kind: c.kind, usesMinAge: c.uses_min_age, items, size });
    if (c.kind === 'contents') {
      if (sel.categories.has(c.id)) entry([], c.size);
      continue;
    }
    const marked = sel.items.get(c.id);
    if (!marked?.size) continue;
    const pool = c.kind === 'project' || c.kind === 'subfolders' ? c.items : c.kind === 'worktrees' ? (worktrees ?? []).filter(removable) : [];
    const items = pool.filter((i) => marked.has(i.path)).map((i) => ({ path: i.path, size: i.size }));
    if (items.length) entry(items, items.reduce((a, i) => a + i.size, 0));
  }
  return out;
}
