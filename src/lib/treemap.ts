// Geometria del mapa y utilidades de rutas. Sin estado de Svelte ni API de Tauri: el script de comprobacion lo carga desde Node.
import { hierarchy, treemap, treemapSquarify } from 'd3-hierarchy';
import type { TreeNode } from './api';

// `files` se anade a la forma del plan para el tooltip. Nunca se usa el nombre como identidad: pueden existir dos `<small>` hermanos.
export type Rect = { path: string; name: string; size: number; files: number; isDir: boolean; depth: number; x: number; y: number; w: number; h: number };

export const SMALL = '<small>'; // Windows no admite '<' en nombres: no choca con un archivo real
export const isSmall = (name: string) => name === SMALL;

const trimSep = (p: string) => p.replace(/[\\/]+$/, '');
export const joinPath = (dir: string, name: string) => `${trimSep(dir)}\\${name}`;
// Raiz escaneada en forma canonica: separadores '\', sin barra final salvo en una unidad (C:\).
export function normalizeRoot(p: string): string {
  const q = trimSep(p.trim().replaceAll('/', '\\').replace(/(?<=[^\\])\\{2,}/g, '\\'));
  return /^[A-Za-z]:$/.test(q) ? q + '\\' : q;
}
const norm = (p: string) => trimSep(p).toLowerCase().replaceAll('/', '\\');

export function isInside(path: string, root: string): boolean {
  const p = norm(path);
  const r = norm(root);
  return p === r || p.startsWith(r + '\\');
}

export function parentPath(path: string): string {
  const p = trimSep(path);
  const i = Math.max(p.lastIndexOf('\\'), p.lastIndexOf('/'));
  const up = i < 0 ? p : p.slice(0, i);
  return /^[A-Za-z]:$/.test(up) ? up + '\\' : up;
}

// Migas desde la raiz escaneada hasta `path`; la primera lleva la ruta completa de la raiz.
export function crumbs(root: string, path: string): { label: string; path: string }[] {
  const out = [{ label: root, path: root }];
  if (!isInside(path, root) || norm(path) === norm(root)) return out;
  let cur = root;
  for (const seg of path.slice(trimSep(root).length).split(/[\\/]/).filter(Boolean)) {
    cur = joinPath(cur, seg);
    out.push({ label: seg, path: cur });
  }
  return out;
}

// Explorador: orden y filtro de la carpeta actual. Copias, nunca mutan la lista. Lo agrupado (<small>) no tiene nombre:
// va al final al ordenar por nombre y ningun texto lo encuentra.
export type SortBy = 'size' | 'name';
type Named = { name: string; size: number };
export function sortEntries<T extends Named>(list: T[], by: SortBy, lang: string): T[] {
  const coll = new Intl.Collator(lang, { numeric: true, sensitivity: 'base' }); // uno por llamada, no por comparacion
  const byName = (a: T, b: T) => Number(isSmall(a.name)) - Number(isSmall(b.name)) || coll.compare(a.name, b.name);
  return [...list].sort(by === 'size' ? (a, b) => b.size - a.size || byName(a, b) : byName);
}
const fold = (s: string, lang: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase(lang);
export function filterEntries<T extends Named>(list: T[], query: string, lang: string): T[] {
  const q = fold(query.trim(), lang);
  return q ? list.filter((e) => !isSmall(e.name) && fold(e.name, lang).includes(q)) : list;
}

export const HEADER = 20; // franja superior de una carpeta con hijos, donde va su etiqueta

// Squarified treemap anidado. Orden: padres antes que hijos (por profundidad), asi que el ultimo que contiene un punto es el mas profundo.
export function layout(root: TreeNode, rootPath: string, width: number, height: number): Rect[] {
  const h = hierarchy(root, (n) => (n.children.length ? n.children : null))
    .sum((n) => (n.children.length ? 0 : n.size))
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
  const pad = (n: { depth: number }) => (n.depth === 0 ? 0 : 3);
  const laid = treemap<TreeNode>().tile(treemapSquarify).size([width, height])
    .paddingTop((n) => (n.depth === 0 ? 0 : HEADER)).paddingRight(pad).paddingBottom(pad).paddingLeft(pad).round(false)(h);
  const paths = new Map<unknown, string>([[laid, rootPath]]);
  return laid.descendants().slice(1).map((n) => {
    const path = joinPath(paths.get(n.parent)!, n.data.name);
    paths.set(n, path);
    return { path, name: n.data.name, size: n.data.size, files: n.data.files, isDir: n.data.is_dir, depth: n.depth, x: n.x0, y: n.y0, w: n.x1 - n.x0, h: n.y1 - n.y0 };
  });
}

export function hit(rects: Rect[], x: number, y: number): Rect | undefined {
  for (let i = rects.length - 1; i >= 0; i--) {
    const r = rects[i];
    if (x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r;
  }
}

// Contraste WCAG entre dos colores #rrggbb.
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
export function labelInk(fill: string): '#ffffff' | '#0b0b0b' {
  return contrast('#ffffff', fill) >= contrast('#0b0b0b', fill) ? '#ffffff' : '#0b0b0b';
}
