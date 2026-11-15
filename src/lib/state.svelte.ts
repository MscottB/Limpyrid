import * as api from './api';
import { isDemo, type CategoryInfo, type CategoryKind, type Drive, type Entry, type Progress, type WorktreeList } from './api';
import type { Outcome } from './ReportPanel.svelte';
import { folderRule, selectable, wtNeedsLoad } from './recovery';
import type { Focus } from './nav';
import { termsAccepted, termsDate, termsRecord } from './terms';
import { LANGS } from './i18n/dicts';
import { pickLang } from './i18n/langs';

export type View = 'home' | 'group' | 'explore' | 'map' | 'about';

function initialLang(): string {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem('limpyrid.lang');
  } catch {
    /* sin almacenamiento: se usa el idioma del sistema */
  }
  return pickLang(saved, navigator.language, LANGS.map((l) => l.code));
}

export const AGE_CHOICES = [24, 72, 168, 336, 720]; // horas; el backend nunca baja de 24
const AGE_KEY = 'limpyrid.minAgeHours';

function readAge(): number {
  try {
    const n = Number(localStorage.getItem(AGE_KEY));
    if (AGE_CHOICES.includes(n)) return n;
  } catch {
    /* sin almacenamiento: valor por defecto */
  }
  return 168;
}

export const app = $state({
  lang: initialLang(),
  view: 'home' as View,
  group: null as string | null, // ecosistema elegido en la navegacion
  focus: null as Focus | null, // fila enfocada del grupo
  picked: null as Entry | null, // elemento elegido en Carpetas o Mapa
  detail: false, // detalle superpuesto abierto (ventana estrecha)
  cwd: 'C:\\',
  // Que se escanea: la raiz de la unidad elegida; null = "Otra carpeta" (scanRoot); '' = aun sin elegir.
  pick: '' as string | null,
  scanRoot: '', // el campo "Otra carpeta": independiente de la carpeta que muestran Explorador y Mapa
  // root: carpeta del ultimo escaneo; las vistas Explorador y Mapa no suben por encima de ella.
  // En demostracion se parte de un escaneo terminado de C:\ (los datos de demo.ts).
  scan: { status: (isDemo ? 'done' : 'idle') as 'idle' | 'running' | 'done', progress: null as Progress | null, denied: 0, root: isDemo ? 'C:\\' : '' },
});

// Sube cuando hay datos nuevos en el backend (fin de un escaneo, papelera, limpieza); las vistas recargan al verlo cambiar.
export const dataRev = $state({ n: 0 });

// Limpieza fuera de los componentes: al cambiar de pestana una limpieza en curso sigue marcada, su informe aparece al
// volver y las categorias no se vuelven a medir. `rev`: el dataRev.n con el que se cargaron.
export const work = $state({
  busy: false,
  updating: false, // descargando e instalando una actualizacion (update.svelte.ts): bloquea como una limpieza
  outcome: null as Outcome | null,
  cats: null as CategoryInfo[] | null,
  catsLoading: false,
  catsError: null as string | null, // codigo del ultimo fallo al cargar las categorias; null si la ultima carga fue bien
  rev: -1,
  minAge: readAge(),
  // Lista de worktrees: `rev` y `age` son el dataRev.n y la antiguedad con que se cargo.
  wt: { list: null as WorktreeList | null, rev: -1, age: -1, loading: false },
  drives: null as Drive[] | null,
  drivesLoading: false,
  // La limpieza en curso: paso `index` de `total` (desde 1), lo liberado de la categoria `id` y lo que se esperaba.
  progress: null as CleanRun | null,
  // Lo limpiado en esta sesion de la app, por categoria (se vacia con un escaneo nuevo). `before`: tamano mostrado antes;
  // `kind`, `ecosystem` y `paths` dejan dibujar la fila aunque el backend ya no la liste.
  cleaned: {} as Record<string, Cleaned>,
});

// Limpiando o escaneando no se marca ni se borra nada: una sola regla para la puerta (guard) y para los controles.
export const locked = () => work.busy || work.updating || app.scan.status === 'running';

export interface CleanRun { index: number; total: number; id: string; freed: number; deleted: number; expected: number; startedAt: number; ids: string[] }
export interface Cleaned { freed: number; deleted: number; skipped: number; before: number; kind: CategoryKind; ecosystem: string; paths: string[] }

export function setLang(lang: string) {
  app.lang = lang;
  try {
    localStorage.setItem('limpyrid.lang', lang);
  } catch {
    /* no persistir no es grave */
  }
}

export function openGroup(ecosystem: string) {
  app.view = 'group';
  app.group = ecosystem;
  app.focus = null;
}

// Detalle superpuesto: por debajo de 1000 px solo lo abre un gesto explicito (clic o Enter en una fila, elegir un
// elemento); al cerrarlo el foco vuelve a donde estaba. Por encima es columna fija y no hace nada.
export const narrow = () => matchMedia('(max-width: 999px)').matches;
let detailFrom: HTMLElement | null = null;
export function openDetail(from: Element | null = document.activeElement) {
  if (!narrow()) return;
  detailFrom = from instanceof HTMLElement ? from : null;
  app.detail = true;
}
export function closeDetail() {
  app.detail = false;
  if (detailFrom?.isConnected) detailFrom.focus();
  detailFrom = null;
}

export function setMinAge(hours: number) {
  work.minAge = hours;
  try {
    localStorage.setItem(AGE_KEY, String(hours));
  } catch {
    /* no persistir no es grave */
  }
}

// Cargas compartidas por la navegacion y la lista del grupo. Un token por carga: solo la ultima pedida escribe su resultado.
let catsToken = 0;
export async function loadCats(): Promise<CategoryInfo[] | null> {
  const token = ++catsToken;
  work.rev = dataRev.n;
  work.catsLoading = true;
  try {
    const list = await api.categories();
    if (token !== catsToken) return null;
    work.cats = list;
    work.catsError = null;
    return list;
  } catch (e) {
    if (token === catsToken) work.outcome = { kind: 'error', code: (work.catsError = api.errCode(e)) };
    return null;
  } finally {
    if (token === catsToken) work.catsLoading = false;
  }
}

let wtToken = 0;
// `silent`: un fallo no escribe work.outcome (la navegacion no debe tapar el informe de una limpieza que aun no se vio).
export async function loadWorktrees(silent = false) {
  const token = ++wtToken;
  const age = work.minAge;
  work.wt.loading = true;
  try {
    const list = await api.worktrees(age);
    if (token === wtToken) work.wt = { list, rev: dataRev.n, age, loading: false };
  } catch (e) {
    if (token === wtToken && !silent) work.outcome = { kind: 'error', code: api.errCode(e) };
  } finally {
    if (token === wtToken) work.wt.loading = false;
  }
}

// Carga la lista de worktrees solo si hace falta (wtNeedsLoad: la misma regla en la lista del grupo y la navegacion). null si no carga.
export function ensureWorktrees(silent = false): Promise<void> | null {
  const has = (work.cats ?? []).some((c) => c.kind === 'worktrees');
  return wtNeedsLoad(work.wt, dataRev.n, work.minAge, app.scan.status === 'running', has) ? loadWorktrees(silent) : null;
}

let drivesToken = 0;
export async function loadDrives() {
  const token = ++drivesToken;
  work.drivesLoading = true;
  try {
    const list = await api.drives();
    if (token !== drivesToken) return;
    work.drives = list;
    // La unidad del sistema viene elegida; una eleccion que ya no existe vuelve a ella.
    if (app.pick !== null && !list.some((d) => d.root === app.pick)) app.pick = list.find((d) => d.system)?.root ?? null;
  } catch {
    if (token === drivesToken) work.drives = work.drives ?? [];
  } finally {
    if (token === drivesToken) work.drivesLoading = false;
  }
}

// Lo que el boton Escanear va a escanear.
export const scanTarget = () => (app.pick === null ? app.scanRoot : app.pick);

// Si se puede escanear lo elegido: la unidad elegida, o la que contiene "Otra carpeta" (con la misma regla).
// Una sola regla para Inicio, el selector de la navegacion y App.svelte, que pide confirmar las extraibles.
export function scanCheck() {
  const target = scanTarget().trim();
  const typed = app.pick === null ? folderRule(target, work.drives) : null;
  const picked = app.pick ? work.drives?.find((d) => d.root === app.pick) : undefined;
  const drive = typed ? typed.drive : picked;
  const rule = typed ? typed.rule : picked ? selectable(picked) : 'no';
  return { target, reason: typed?.reason ?? null, drive, rule, ok: !!target && rule !== 'no' && !work.busy };
}

// --- Aviso de uso: antes de toda accion que borra (limpiar, Docker/Podman, Papelera) y antes de su confirmacion.
const TERMS_KEY = 'limpyrid.terms';
// En demostracion cada carga empieza sin aceptar, para poder ver el aviso.
if (isDemo) {
  try {
    localStorage.removeItem(TERMS_KEY);
  } catch {
    /* sin almacenamiento: igual se pregunta */
  }
}
function storedTerms(): string | null {
  try {
    return localStorage.getItem(TERMS_KEY);
  } catch {
    return null; // sin almacenamiento: se pregunta cada vez, nunca se da por aceptado
  }
}

// `mode`: null cerrado; 'ask' antes de una accion (pide aceptar); 'read' solo lectura (desde Acerca de).
export const terms = $state({ mode: null as 'ask' | 'read' | null, at: termsDate(storedTerms()) });
let settle: ((ok: boolean) => void) | null = null;

// La unica puerta del aviso: true si ya estaban aceptados o se aceptan ahora; false si se cancela.
export function ensureTerms(): Promise<boolean> {
  if (termsAccepted(storedTerms())) return Promise.resolve(true);
  settle?.(false); // un aviso que quedo abierto cuenta como cancelado
  terms.mode = 'ask';
  return new Promise((resolve) => (settle = resolve));
}

export function closeTerms(accepted: boolean) {
  const ask = terms.mode === 'ask';
  terms.mode = null;
  if (ask && accepted) {
    const now = Date.now();
    try {
      localStorage.setItem(TERMS_KEY, termsRecord(now));
      terms.at = now; // solo si quedo guardado: Acerca de no dice "aceptaste" si se va a volver a preguntar
    } catch {
      /* no se guarda: la proxima accion vuelve a preguntar */
    }
  }
  const s = settle;
  settle = null;
  s?.(ask && accepted);
}

export const showTerms = () => (terms.mode = 'read');
