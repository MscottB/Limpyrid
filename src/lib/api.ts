// Una funcion tipada por comando de Tauri (src-tauri/src/lib.rs). Los tipos salen de los structs serializados de Rust.
// Los comandos rechazan con un codigo de error (cadena); la interfaz lo traduce con t('err.' + codigo).
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { updateErrCode } from './update';
import { version } from '../../package.json';

// Modo de demostracion: solo con el servidor de desarrollo (vite dev) y fuera de Tauri; dentro de la app existe
// __TAURI_INTERNALS__. En el build de produccion import.meta.env.DEV es false: el import dinamico desaparece y
// demo.ts no llega al paquete.
export const isDemo = import.meta.env.DEV && !('__TAURI_INTERNALS__' in window);
const demo = isDemo ? await import('./demo') : null;
const call = <T>(cmd: string, args?: Record<string, unknown>) => (demo ? demo.demoInvoke<T>(cmd, args) : invoke<T>(cmd, args));
const on = <T>(event: string, cb: (p: T) => void) => (demo ? demo.demoListen<T>(event, cb) : listen<T>(event, (e) => cb(e.payload)));

export type Engine = 'docker' | 'podman';
export type CategoryKind = 'contents' | 'subfolders' | 'project' | 'containers' | 'worktrees' | 'info_only';
export type SkipReason = 'in_use' | 'denied' | 'blocked' | 'too_recent' | 'too_large' | 'other';

export interface Progress { dirs: number; bytes: number; current: string }
export interface ScanDone { cancelled: boolean; denied: number }
// Acumulado de una categoria durante clean_category (lib.rs); los worktrees no emiten.
export interface CleanProgress { id: string; freed: number; deleted: number }
export interface Entry { name: string; path: string; size: number; files: number; is_dir: boolean }
export interface TreeNode { name: string; size: number; files: number; is_dir: boolean; children: TreeNode[] }
export interface Found { category: string; path: string; size: number; modified: number | null }
export interface CategoryInfo {
  id: string;
  ecosystem: string;
  kind: CategoryKind;
  uses_min_age: boolean;
  size: number;
  paths: string[];
  items: Found[];
}
export interface Skipped { path: string; reason: SkipReason }
export interface Report { freed: number; deleted: number; skipped: Skipped[] }
export type WorktreeKind = 'claude_agent' | 'claude' | 'git';
export type Protection = 'uncommitted' | 'unreachable' | 'recent' | 'locked' | 'ignored' | 'unknown';
export interface Worktree {
  path: string;
  repo: string;
  name: string;
  kind: WorktreeKind;
  branch: string | null;
  size: number;
  modified: number | null; // segundos unix; aproximado si hubo actividad reciente
  protection: Protection[]; // vacio = se puede quitar
}
export interface Orphan { path: string; repo: string; size: number }
export interface WorktreeList { git: boolean; worktrees: Worktree[]; orphans: Orphan[] }
export interface UsageRow { kind: string; total: string; reclaimable: string }
export interface DiskFile { path: string; size: number }
export type DriveKind = 'fixed' | 'removable' | 'network' | 'optical' | 'ram_disk' | 'unknown';
// drives.rs: total y free en bytes (free = libre para el usuario; 0 y 0 si la unidad no responde)
export interface Drive { root: string; label: string; kind: DriveKind; usb: boolean; system: boolean; filesystem: string; total: number; free: number }
export interface Usage { installed: boolean; running: boolean; rows: UsageRow[]; disks: DiskFile[] }

export const startScan = (root: string) => call<void>('start_scan', { root });
export const cancelScan = () => call<void>('cancel_scan');
export const deniedPaths = () => call<string[]>('denied_paths');
export const children = (path: string) => call<Entry[]>('children', { path });
export const treemap = (path: string) => call<TreeNode>('treemap', { path });
export const categories = () => call<CategoryInfo[]>('categories');
// toTrash: lo limpiado va a la Papelera de reciclaje; `freed` cuenta entonces lo enviado, que sigue ocupando disco.
export const cleanCategory = (id: string, items: string[], minAgeHours: number | null, toTrash: boolean) =>
  call<Report>('clean_category', { id, items, minAgeHours, toTrash });
export const trashPaths = (paths: string[]) => call<Report>('trash_paths', { paths });
export const worktrees = (minAgeHours: number | null) => call<WorktreeList>('worktrees', { minAgeHours });
export const removeWorktrees = (paths: string[], minAgeHours: number | null) =>
  call<Report>('remove_worktrees', { paths, minAgeHours });
export const containerUsage = (engine: Engine) => call<Usage>('container_usage', { engine });
export const containerPrune = (engine: Engine, allImages: boolean) => call<string>('container_prune', { engine, allImages });
// [tamano antes, tamano despues] en bytes
export const containerCompact = (engine: Engine) => call<[number, number]>('container_compact', { engine });

export const drives = () => call<Drive[]>('drives');
export const isPortable = () => call<boolean>('is_portable');
export const appVersion = () => (isDemo ? Promise.resolve(version) : getVersion());
export const reveal = (path: string) => (isDemo ? Promise.resolve() : revealItemInDir(path));

// Actualizacion (complemento updater): la unica conexion de red de la app.
export interface UpdateInfo { version: string; current: string; date: string | null; notes: string }
let pending: Update | null = null; // lo que encontro la ultima comprobacion: es lo que instala installUpdate
export async function checkUpdate(): Promise<UpdateInfo | null> {
  if (demo) return call<UpdateInfo | null>('check_update');
  void pending?.close().catch(() => {}); // libera el recurso de Rust de la comprobacion anterior
  pending = null;
  try {
    pending = await check({ timeout: 15_000 });
  } catch {
    throw updateErrCode('check');
  }
  return pending && { version: pending.version, current: pending.currentVersion, date: pending.date ?? null, notes: pending.body ?? '' };
}
// Descarga, verifica la firma e instala: en Windows el complemento cierra la app y el instalador (/R) la vuelve a abrir.
export async function installUpdate(onProgress: (done: number, total: number | null) => void): Promise<void> {
  if (demo) return demo.demoInstallUpdate(onProgress);
  if (!pending) throw updateErrCode('install');
  let done = 0;
  let total: number | null = null;
  try {
    // Limite de la peticion entera; sin el, la descarga hereda los 15 s de la comprobacion.
    await pending.downloadAndInstall((e) => {
      if (e.event === 'Started') total = e.data.contentLength ?? null;
      else if (e.event === 'Progress') done += e.data.chunkLength;
      onProgress(done, total);
    }, { timeout: 300_000 });
  } catch {
    throw updateErrCode('install');
  }
}
export const openDownloads = () => (isDemo ? Promise.resolve() : openUrl('https://mscottb.github.io/Limpyrid/descargar.html'));

export const onScanProgress = (cb: (p: Progress) => void) => on<Progress>('scan-progress', cb);
export const onScanDone = (cb: (d: ScanDone) => void) => on<ScanDone>('scan-done', cb);
export const onCleanProgress = (cb: (p: CleanProgress) => void) => on<CleanProgress>('clean-progress', cb);

// Los rechazos llegan como cadena; cualquier otra cosa cuenta como fallo generico.
export const errCode = (e: unknown): string => (typeof e === 'string' ? e : 'command_failed');
