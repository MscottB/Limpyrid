// Datos de ejemplo para revisar la interfaz en un navegador, sin el backend de Tauri (modo de demostracion).
// Solo se usan si api.isDemo es verdadero: en desarrollo y fuera de Tauri. Nada aqui toca el disco.
// Sin dependencias de Svelte ni de Tauri: scripts/check-frontend.mjs lo carga desde Node.
import type { CategoryInfo, Drive, Entry, Found, Report, TreeNode, Usage, Worktree, WorktreeList } from './api';

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const gb = (n: number) => Math.round(n * GB);
const DAY = 86_400;
const now = () => Math.floor(Date.now() / 1000);
const ago = (days: number) => now() - Math.round(days * DAY);
const HOME = 'C:\\Users\\dev';
const LOCAL = `${HOME}\\AppData\\Local`;

// Tamanos del equipo real: unidad del sistema de 510,9 GB con unos 79 GB libres.
export const DRIVES: Drive[] = [
  { root: 'C:\\', label: 'Windows', kind: 'fixed', usb: false, system: true, filesystem: 'NTFS', total: gb(510.9), free: gb(79.2) },
  { root: 'D:\\', label: 'Datos', kind: 'fixed', usb: false, system: false, filesystem: 'NTFS', total: gb(931.5), free: gb(412.3) },
  { root: 'E:\\', label: 'KINGSTON', kind: 'removable', usb: true, filesystem: 'exFAT', system: false, total: gb(57.7), free: gb(31.4) },
  { root: 'Z:\\', label: 'Compartido', kind: 'network', usb: false, system: false, filesystem: 'NTFS', total: gb(2048), free: gb(1130.6) },
  { root: 'F:\\', label: '', kind: 'optical', usb: false, system: false, filesystem: '', total: 0, free: 0 }, // lectora sin disco
];

// --- Arbol de C:\ para Explorador y Mapa ---
const file = (name: string, gbs: number, files = 1): TreeNode => ({ name, size: gb(gbs), files, is_dir: files > 1, children: [] });
const dir = (name: string, children: TreeNode[]): TreeNode => ({
  name,
  size: children.reduce((a, c) => a + c.size, 0),
  files: children.reduce((a, c) => a + c.files, 0),
  is_dir: true,
  children: [...children].sort((a, b) => b.size - a.size),
});
const small = (gbs: number, files: number) => file('<small>', gbs, files); // SMALL de treemap.ts

const project = (name: string, parts: TreeNode[]) => dir(name, [...parts, file('src', 0.08, 1840), file('.git', 0.31, 2210)]);

export const TREE: TreeNode = dir('C:\\', [
  dir('Users', [
    dir('dev', [
      dir('AppData', [
        dir('Local', [
          dir('Docker', [dir('wsl', [dir('disk', [file('docker_data.vhdx', 82.3)]), file('main', 1.2, 3)])]),
          dir('Temp', [dir('claude', [file('C--Users-dev-Proyectos-tienda', 1.9, 4120), file('C--Users-dev-Proyectos-motor-rs', 0.8, 1210), file('C--Users-dev-Proyectos-analitica', 0.4, 640)]), small(15.3, 48210)]),
          dir('uv', [file('cache', 24.8, 210400)]),
          file('npm-cache', 9.1, 182300),
          dir('pnpm', [file('store', 3.4, 96100)]),
          dir('Packages', [file('CanonicalGroupLimited.Ubuntu', 11.8, 4)]),
          dir('go-build', [small(2.3, 30120)]),
          file('Microsoft', 7.6, 41800),
          small(4.2, 61200),
        ]),
        dir('Roaming', [file('Code', 2.1, 18300), file('uv', 3.2, 21400), small(1.0, 9100)]),
      ]),
      dir('Proyectos', [
        project('tienda', [file('node_modules', 1.8, 118400), file('.next', 0.6, 2310)]),
        project('panel-admin', [file('node_modules', 1.1, 74200)]),
        project('motor-rs', [dir('target', [file('debug', 5.1, 9800), file('release', 1.7, 2100)])]),
        project('analitica', [file('.venv', 2.4, 31800), file('__pycache__', 0.05, 420), file('datos', 3.2, 120)]),
        project('limpyrid', [file('node_modules', 0.4, 22100), dir('src-tauri', [file('target', 4.3, 7600)])]),
      ]),
      dir('.ollama', [file('models', 14.2, 38)]),
      dir('.claude', [file('projects', 4.8, 9200), small(1.3, 2400)]),
      dir('.cargo', [file('registry', 1.3, 41200), file('git', 0.2, 3100)]),
      dir('.gradle', [file('caches', 4.6, 51800)]),
      file('Videos', 42.0, 310),
      file('Downloads', 23.7, 1240),
      file('Documents', 12.4, 8900),
      small(3.1, 12400),
    ]),
    dir('Public', [small(0.2, 120)]),
  ]),
  dir('Windows', [file('WinSxS', 9.8, 81200), file('System32', 12.3, 22100), file('Temp', 1.2, 3400), small(15.3, 140200)]),
  dir('Program Files', [file('Docker', 4.1, 2100), file('Microsoft Visual Studio', 11.2, 61000), small(11.1, 88400)]),
  dir('Program Files (x86)', [small(8.1, 40100)]),
  dir('ProgramData', [file('Docker', 1.4, 300), small(8.3, 30200)]),
  file('pagefile.sys', 16.0),
  file('hiberfil.sys', 6.4),
  small(24.7, 51200), // completa los 431,7 GB ocupados de la unidad C: de DRIVES
]);

const norm = (p: string) => p.replace(/[\\/]+$/, '').toLowerCase();

// La carpeta pedida dentro de TREE; una ruta que no esta en el ejemplo se muestra vacia.
function find(path: string): TreeNode {
  const root = norm(TREE.name);
  const want = norm(path);
  if (want === root) return TREE;
  if (!want.startsWith(root + '\\')) return { name: path, size: 0, files: 0, is_dir: true, children: [] };
  let node: TreeNode | undefined = TREE;
  for (const part of want.slice(root.length + 1).split('\\')) node = node?.children.find((c) => c.name.toLowerCase() === part);
  return node ?? { name: path, size: 0, files: 0, is_dir: true, children: [] };
}

const join = (dirPath: string, name: string) => `${dirPath.replace(/[\\/]+$/, '')}\\${name}`;
const toEntries = (path: string): Entry[] =>
  find(path).children.map((c) => ({ name: c.name, path: join(path, c.name), size: c.size, files: c.files, is_dir: c.is_dir }));

// El backend envia solo unos niveles; aqui se recorta igual.
const trim = (n: TreeNode, depth: number): TreeNode => ({ ...n, children: depth > 1 ? n.children.map((c) => trim(c, depth - 1)) : [] });

// --- Categorias ---
const found = (category: string, path: string, gbs: number, modifiedDaysAgo: number | null = null): Found => ({
  category,
  path,
  size: gb(gbs),
  modified: modifiedDaysAgo == null ? null : ago(modifiedDaysAgo),
});
const cat = (id: string, ecosystem: string, kind: CategoryInfo['kind'], size: number, paths: string[], items: Found[] = [], usesMinAge = false): CategoryInfo => ({
  id,
  ecosystem,
  kind,
  uses_min_age: usesMinAge,
  size: items.length ? items.reduce((a, i) => a + i.size, 0) : gb(size),
  paths,
  items,
});
const P = `${HOME}\\Proyectos`;
// Una carpeta con muchos ejemplos de un curso: una lista de mas de 100 carpetas, con fechas de dias a mas de un ano.
const EXPERIMENTS: Found[] = Array.from({ length: 128 }, (_, i) =>
  found('node-projects', `${P}\\curso-react\\ejercicio-${String(i + 1).padStart(3, '0')}\\node_modules`, 0.04 + ((i * 37) % 23) / 100, 3 + ((i * 53) % 420)),
);

export const CATEGORIES: CategoryInfo[] = [
  cat('temp-user', 'system', 'contents', 15.3, [`${LOCAL}\\Temp`], [], true),
  cat('temp-windows', 'system', 'contents', 1.2, ['C:\\Windows\\Temp'], [], true),
  cat('npm', 'node', 'contents', 9.1, [`${LOCAL}\\npm-cache`]),
  cat('pnpm', 'node', 'contents', 3.4, [`${LOCAL}\\pnpm\\store`, `${LOCAL}\\pnpm-cache`]),
  cat('yarn', 'node', 'contents', 0.9, [`${LOCAL}\\Yarn\\Cache`]), // nivel careful: la casilla del grupo no la marca
  cat('node-projects', 'node', 'project', 0, [], [
    found('node-projects', `${P}\\tienda\\node_modules`, 1.8, 2),
    found('node-projects', `${P}\\panel-admin\\node_modules`, 1.1, 74),
    found('node-projects', `${P}\\tienda\\.next`, 0.6, 2),
    found('node-projects', `${P}\\limpyrid\\node_modules`, 0.4, 0.2),
    // Nombre y ruta muy largos: la fila y el detalle no deben desbordar.
    found('node-projects', `${P}\\clientes\\2026\\migracion-del-sistema-de-facturacion-electronica-con-integracion-de-pasarelas-de-pago-internacionales-frontend-administracion-v2\\node_modules`, 0.7, 45),
    ...EXPERIMENTS,
  ]),
  cat('uv', 'python', 'contents', 24.8, [`${LOCAL}\\uv\\cache`]),
  cat('pip', 'python', 'contents', 1.9, [`${LOCAL}\\pip\\Cache`]),
  cat('uv-installed', 'python', 'info_only', 3.2, [`${HOME}\\AppData\\Roaming\\uv\\python`, `${HOME}\\AppData\\Roaming\\uv\\tools`]),
  cat('python-projects', 'python', 'project', 0, [], [
    found('python-projects', `${P}\\analitica\\.venv`, 2.4, 131),
    found('python-projects', `${P}\\analitica\\__pycache__`, 0.05, 131),
  ]),
  cat('cargo', 'rust', 'contents', 1.5, [`${HOME}\\.cargo\\registry`, `${HOME}\\.cargo\\git`]),
  cat('rust-target', 'rust', 'project', 0, [], [
    found('rust-target', `${P}\\motor-rs\\target`, 6.8, 38),
    found('rust-target', `${P}\\limpyrid\\src-tauri\\target`, 4.3, 0.1),
  ]),
  cat('go-build', 'go', 'contents', 2.3, [`${LOCAL}\\go-build`]),
  cat('gradle', 'java', 'contents', 4.6, [`${HOME}\\.gradle\\caches`]),
  cat('vscode', 'editors', 'contents', 0.8, [`${HOME}\\AppData\\Roaming\\Code\\Cache`, `${HOME}\\AppData\\Roaming\\Code\\CachedData`]),
  cat('huggingface', 'local-ai', 'contents', 6.2, [`${HOME}\\.cache\\huggingface`]),
  cat('ollama', 'local-ai', 'info_only', 14.2, [`${HOME}\\.ollama\\models`]),
  cat('docker', 'containers', 'containers', 83.5, [`${LOCAL}\\Docker\\wsl`]),
  cat('podman', 'containers', 'containers', 0, [`${HOME}\\.local\\share\\containers\\podman\\machine`]),
  cat('wsl', 'wsl', 'info_only', 11.8, [`${LOCAL}\\Packages\\CanonicalGroupLimited.Ubuntu\\LocalState\\ext4.vhdx`]),
  cat('claude-code', 'ai-agents', 'info_only', 6.1, [`${HOME}\\.claude`]),
  cat('claude-sessions', 'ai-agents', 'subfolders', 0, [`${LOCAL}\\Temp\\claude`], [
    found('claude-sessions', `${LOCAL}\\Temp\\claude\\C--Users-dev-Proyectos-tienda`, 1.9, 0.3),
    found('claude-sessions', `${LOCAL}\\Temp\\claude\\C--Users-dev-Proyectos-motor-rs`, 0.8, 9),
    found('claude-sessions', `${LOCAL}\\Temp\\claude\\C--Users-dev-Proyectos-analitica`, 0.4, 26),
    found('claude-sessions', `${LOCAL}\\Temp\\claude\\C--Users-dev-Proyectos-panel-admin`, 0.12, null),
  ], true),
  cat('git-worktrees', 'git', 'worktrees', 0, [], [], true),
];

// --- Worktrees: libres y protegidos por motivos distintos, y una carpeta huerfana ---
const wt = (repo: string, name: string, kind: Worktree['kind'], branch: string | null, gbs: number, daysAgo: number | null, protection: Worktree['protection'] = []): Worktree => ({
  path: kind === 'git' ? `${P}\\${repo}-${name}` : `${P}\\${repo}\\.claude\\worktrees\\${name}`,
  repo: `${P}\\${repo}`,
  name,
  kind,
  branch,
  size: gb(gbs),
  modified: daysAgo == null ? null : ago(daysAgo),
  protection,
});

export const WORKTREES: WorktreeList = {
  git: true,
  worktrees: [
    wt('tienda', 'agent-a41f9c', 'claude_agent', 'worktree-agent-a41f9c', 1.4, 12),
    wt('tienda', 'fix-pagos', 'claude', 'claude/fix-pagos', 2.1, 21),
    wt('motor-rs', 'agent-7d20b1', 'claude_agent', null, 0.9, 34),
    wt('tienda', 'nueva-api', 'claude', 'claude/nueva-api', 1.7, 0.5, ['recent']),
    wt('panel-admin', 'release-1.2', 'git', 'release/1.2', 1.1, 4, ['uncommitted', 'recent']),
    wt('analitica', 'agent-c3e8aa', 'claude_agent', null, 0.6, 18, ['unreachable']),
    wt('motor-rs', 'bench', 'git', 'bench', 3.2, 40, ['ignored']),
    wt('limpyrid', 'spike-ui', 'git', 'spike/ui', 0.5, 60, ['locked']),
  ],
  orphans: [{ path: `${P}\\tienda\\.claude\\worktrees\\agent-0b11e2`, repo: `${P}\\tienda`, size: 640 * MB }],
};

// --- Contenedores ---
export const DOCKER: Usage = {
  installed: true,
  running: true,
  rows: [
    { kind: 'Images', total: '38.2GB', reclaimable: '21.7GB (56%)' },
    { kind: 'Containers', total: '1.3GB', reclaimable: '912MB (69%)' },
    { kind: 'Local Volumes', total: '12.4GB', reclaimable: '3.1GB (25%)' },
    { kind: 'Build Cache', total: '9.8GB', reclaimable: '9.8GB' },
  ],
  disks: [{ path: `${LOCAL}\\Docker\\wsl\\disk\\docker_data.vhdx`, size: gb(82.3) }],
};
const PODMAN: Usage = { installed: false, running: false, rows: [], disks: [] };

const DENIED = [`${HOME}\\AppData\\Local\\Microsoft\\WindowsApps\\Backup`, 'C:\\Windows\\System32\\LogFiles\\WMI\\RtBackup', 'C:\\ProgramData\\Microsoft\\Windows\\WER'];

// Informes de ejemplo (nada se borra): las categorias con antiguedad minima saltan algo, como haria el backend.
const TEMP_SKIPS: Report['skipped'] = [
  { path: `${LOCAL}\\Temp\\vscode-ipc-4c1d.sock`, reason: 'in_use' },
  { path: `${LOCAL}\\Temp\\claude-update.log`, reason: 'too_recent' },
];
const report = (freed: number, deleted: number, skipped: Report['skipped'] = []): Report => ({ freed: Math.round(freed), deleted, skipped });

function cleanReport(id: string, items: string[]): Report {
  const c = CATEGORIES.find((x) => x.id === id);
  if (!c || c.kind === 'info_only' || c.kind === 'containers' || c.kind === 'worktrees') return report(0, 0, []);
  if (c.kind === 'contents') return c.uses_min_age ? report(c.size * 0.94, 1840, TEMP_SKIPS) : report(c.size, 12_400);
  const picked = c.items.filter((i) => items.includes(i.path));
  if (c.kind === 'project') return report(picked.reduce((s, i) => s + i.size, 0), picked.length);
  // Subcarpetas: la carpeta con actividad de hoy se salta.
  const recent = picked.filter((i) => i.modified != null && now() - i.modified < DAY);
  const rest = picked.filter((i) => !recent.includes(i));
  return report(rest.reduce((s, i) => s + i.size, 0), rest.length, recent.map((i) => ({ path: i.path, reason: 'too_recent' as const })));
}

// Worktrees: el ultimo de la lista "recibio cambios" mientras se confirmaba y se salta.
function removeReport(paths: string[]): Report {
  const picked = WORKTREES.worktrees.filter((w) => paths.includes(w.path));
  const skipped = picked.length > 1 ? picked.slice(-1) : [];
  const done = picked.filter((w) => !skipped.includes(w));
  return report(done.reduce((s, w) => s + w.size, 0), done.length, skipped.map((w) => ({ path: w.path, reason: 'too_recent' as const })));
}

// --- Eventos y comandos simulados ---
type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();
const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((cb) => cb(payload));

export function demoListen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  const set = listeners.get(event) ?? listeners.set(event, new Set()).get(event)!;
  set.add(cb as Listener);
  return Promise.resolve(() => set.delete(cb as Listener));
}

let scanTimer: ReturnType<typeof setInterval> | null = null;

function startScan() {
  if (scanTimer) return Promise.reject('scan_running');
  let step = 0;
  scanTimer = setInterval(() => {
    step++;
    emit('scan-progress', { dirs: step * 11_873, bytes: Math.round((TREE.size * step) / 24), current: `${HOME}\\Proyectos` });
    if (step >= 24) finishScan(false);
  }, 120);
  return Promise.resolve();
}

function finishScan(cancelled: boolean) {
  if (scanTimer) clearInterval(scanTimer);
  scanTimer = null;
  emit('scan-done', { cancelled, denied: cancelled ? 0 : DENIED.length });
}

const later = <T>(value: T, ms = 150) => new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

// Como clean_category en lib.rs: eventos clean-progress con lo acumulado de la categoria y uno final con el total.
function simulateClean(id: string, items: string[]): Promise<Report> {
  const rep = cleanReport(id, items);
  const steps = 10;
  for (let k = 1; k <= steps; k++) {
    const share = k === steps ? 1 : (k / steps) ** 1.3;
    setTimeout(() => emit('clean-progress', { id, freed: Math.round(rep.freed * share), deleted: Math.round(rep.deleted * share) }), k * 350);
  }
  return later(rep, steps * 350 + 50).then((r) => (applyClean(id, items, r), r));
}

// Lo limpiado deja de estar en los datos de ejemplo, como en el disco: lo omitido se queda. Una categoria de proyecto sin
// carpetas desaparece de la lista (igual que en commands.rs).
function applyClean(id: string, items: string[], rep: Report) {
  const i = CATEGORIES.findIndex((x) => x.id === id);
  const c = CATEGORIES[i];
  if (!c) return;
  const kept = new Set(rep.skipped.map((s) => s.path));
  if (c.kind === 'contents') c.size = Math.max(0, c.size - rep.freed);
  else {
    c.items = c.items.filter((f) => !items.includes(f.path) || kept.has(f.path));
    c.size = c.items.reduce((a, f) => a + f.size, 0);
    if (c.kind === 'project' && !c.items.length) CATEGORIES.splice(i, 1);
  }
}
const sizeOf = (paths: string[]) => paths.reduce((a, p) => a + find(p).size, 0);

// Descarga simulada de 20 MB en 2 s; no instala ni reinicia nada.
export async function demoInstallUpdate(onProgress: (done: number, total: number | null) => void) {
  const total = 20 * 1048576;
  for (let done = 0; done < total; ) {
    await later(null, 100);
    done += total / 20;
    onProgress(done, total);
  }
}

// Mismos nombres de comando y argumentos que invoke() en api.ts.
export function demoInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const a = args as Record<string, never>;
  const out = ((): Promise<unknown> => {
    switch (cmd) {
      case 'drives': return later(DRIVES);
      case 'is_portable': return later(false);
      case 'check_update': return later({ version: '0.2.0', current: '0.1.0', date: '2026-12-01T00:00:00Z', notes: 'Notas de ejemplo de la version 0.2.0.' }, 600);
      case 'start_scan': return startScan();
      case 'cancel_scan': finishScan(true); return Promise.resolve();
      case 'denied_paths': return later(DENIED);
      case 'children': return later(toEntries(a.path));
      case 'treemap': return later(trim(find(a.path), 3));
      case 'categories': return later(structuredClone(CATEGORIES), 600);
      case 'clean_category': return simulateClean(a.id, a.items);
      case 'trash_paths': return later({ freed: sizeOf(a.paths), deleted: (a.paths as string[]).length, skipped: [] }, 400);
      case 'worktrees': return later(structuredClone(WORKTREES), 300);
      case 'remove_worktrees': {
        const rep = removeReport(a.paths);
        const kept = new Set(rep.skipped.map((s) => s.path));
        return later(rep, 900).then((r) => ((WORKTREES.worktrees = WORKTREES.worktrees.filter((w) => !(a.paths as string[]).includes(w.path) || kept.has(w.path))), r));
      }
      case 'container_usage': return later(a.engine === 'docker' ? DOCKER : PODMAN, 400);
      case 'container_prune': return later('Deleted build cache objects:\nm2ve7o3b2x1k\n\nTotal reclaimed space: 9.8GB', 900);
      case 'container_compact': return later([gb(82.3), gb(61.0)], 1500);
      default: return Promise.reject('command_failed');
    }
  })();
  return out as Promise<T>;
}
