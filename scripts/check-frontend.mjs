// Comprobaciones del frontend sin navegador: claves de i18n y formato. Uso: npm test
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { formatBytes, formatAge, formatShare } from '../src/lib/format.ts';
import { pickLang, lookup, FALLBACK } from '../src/lib/i18n/langs.ts';
import { prunableRows } from '../src/lib/containers.ts';
import { removable, sortWorktrees } from '../src/lib/worktrees.ts';
import { layout, hit, labelInk, contrast, isInside, parentPath, crumbs, normalizeRoot, sortEntries, filterEntries, HEADER } from '../src/lib/treemap.ts';

const load = (l) => JSON.parse(readFileSync(new URL(`../src/lib/i18n/${l}.json`, import.meta.url), 'utf8'));
const es = load('es');
const en = load('en');

// Copiado de CATALOG en src-tauri/src/catalog.rs: mantener sincronizado con esa fuente.
const CATEGORY_IDS = [
  'temp-user', 'temp-windows', 'npm', 'pnpm', 'yarn', 'bun', 'deno', 'playwright', 'node-projects',
  'pip', 'uv', 'uv-installed', 'poetry', 'python-projects', 'go-build', 'go-mod', 'cargo', 'rust-target',
  'gradle', 'maven', 'nuget', 'composer', 'dart-pub', 'cpp', 'zig', 'terraform-projects', 'web-testing',
  'native-build', 'vscode', 'jetbrains', 'huggingface', 'ollama', 'android', 'minikube', 'docker', 'podman',
  'wsl', 'claude-desktop-cache', 'claude-desktop-data', 'claude-code', 'codex', 'antigravity-cache', 'gemini',
  'claude-sessions', 'git-worktrees',
];
// Las que se pueden limpiar (Contenido, Proyecto, Subcarpetas y Worktrees en catalog.rs): mantener sincronizado.
const CLEANABLE_IDS = [
  'temp-user', 'temp-windows', 'npm', 'pnpm', 'yarn', 'bun', 'deno', 'playwright', 'node-projects', 'pip', 'uv', 'poetry',
  'python-projects', 'go-build', 'go-mod', 'cargo', 'rust-target', 'gradle', 'maven', 'nuget', 'composer', 'dart-pub', 'cpp',
  'zig', 'terraform-projects', 'git-worktrees', 'web-testing', 'native-build', 'vscode', 'jetbrains', 'huggingface',
  'minikube', 'antigravity-cache', 'claude-sessions',
];
const LEVELS = ['auto', 'redownload', 'rebuild', 'careful'];
const ECOSYSTEMS = [
  'system', 'node', 'python', 'go', 'rust', 'java', 'dotnet', 'php', 'dart', 'cpp', 'zig', 'terraform',
  'web-testing', 'native-build', 'editors', 'local-ai', 'android', 'kubernetes', 'containers', 'wsl', 'ai-agents', 'git',
];
const ERROR_CODES = ['no_scan', 'scan_running', 'busy', 'blocked', 'unknown_category', 'engine_missing', 'engine_stopped', 'engine_remote', 'uac_denied', 'command_failed', 'update_check_failed', 'update_install_failed'];
const SKIP_REASONS = ['in_use', 'denied', 'blocked', 'too_recent', 'too_large', 'other']; // SkipReason en clean.rs (snake_case)
// Protection y WorktreeKind en worktrees.rs (snake_case): mantener sincronizado con esa fuente.
const PROTECTIONS = ['uncommitted', 'unreachable', 'recent', 'locked', 'ignored', 'unknown'];
const WORKTREE_KINDS = ['claude_agent', 'claude', 'git'];
const REQUIRED = [
  ...CATEGORY_IDS.flatMap((i) => [`cat.${i}`, `cat.${i}.hint`]),
  ...ECOSYSTEMS.map((e) => `eco.${e}`),
  ...ERROR_CODES.map((c) => `err.${c}`),
  ...SKIP_REASONS.map((r) => `skip.${r}`),
  ...PROTECTIONS.map((p) => `wt.protection.${p}`),
  ...WORKTREE_KINDS.map((k) => `wt.kind.${k}`),
  'node.small',
  ...[...CLEANABLE_IDS, 'docker', 'podman'].map((i) => `cat.${i}.effect`),
  ...LEVELS.map((l) => `impact.${l}`),
];

const keys = (o) => Object.keys(o).sort();
assert.deepEqual(keys(es), keys(en), 'es.json y en.json deben tener las mismas claves');
for (const [name, d] of [['es', es], ['en', en]]) {
  const missing = REQUIRED.filter((k) => !(k in d));
  assert.deepEqual(missing, [], `faltan claves en ${name}.json`);
  const empty = Object.keys(d).filter((k) => typeof d[k] !== 'string' || !d[k].trim());
  assert.deepEqual(empty, [], `valores vacios en ${name}.json`);
}
// Los marcadores {nombre} deben coincidir entre idiomas.
const ph = (s) => (s.match(/\{\w+\}/g) ?? []).sort().join();
for (const k of Object.keys(es)) assert.equal(ph(es[k]), ph(en[k]), `marcadores distintos en ${k}`);
// Cada <codigo>.json es un idioma: es y en deben coincidir (arriba); los demas pueden ir incompletos pero sin claves ajenas.
const langFiles = readdirSync(new URL('../src/lib/i18n/', import.meta.url)).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5));
assert.ok(langFiles.includes('es') && langFiles.includes('en'));
for (const code of langFiles) {
  assert.doesNotThrow(() => Intl.getCanonicalLocales(code), `${code}.json: el nombre debe ser una etiqueta de idioma`);
  const d = load(code);
  assert.ok(typeof d['lang.name'] === 'string' && d['lang.name'].trim(), `${code}.json sin lang.name`);
  if (code === 'es' || code === 'en') continue;
  assert.deepEqual(Object.keys(d).filter((k) => !(k in en)), [], `${code}.json tiene claves que no existen en en.json`);
  for (const k of Object.keys(d)) assert.equal(ph(d[k]), ph(en[k]), `marcadores distintos en ${k} (${code})`);
  const missing = Object.keys(en).filter((k) => !(k in d));
  if (missing.length) console.warn(`${code}.json: faltan ${missing.length} claves (se usara el ingles): ${missing.slice(0, 10).join(', ')}`);
}
assert.equal(FALLBACK, 'en');
assert.equal(pickLang('es', 'en-US', ['en', 'es']), 'es', 'manda el guardado');
assert.equal(pickLang('fr', 'es-CL', ['en', 'es']), 'es', 'guardado sin archivo: el del sistema');
assert.equal(pickLang(null, 'pt-BR', ['en', 'es', 'pt-BR', 'pt']), 'pt-BR', 'etiqueta completa primero');
assert.equal(pickLang(null, 'PT-br', ['en', 'pt']), 'pt', 'luego solo el idioma, sin importar mayusculas');
assert.equal(pickLang(null, 'ja-JP', ['en', 'es']), 'en', 'sin archivo: ingles');
assert.equal(pickLang(null, '', ['en', 'es']), 'en');
const D = { en: { a: 'A', b: 'B' }, pt: { a: 'Á' } };
assert.equal(lookup(D, 'pt', 'a'), 'Á');
assert.equal(lookup(D, 'pt', 'b'), 'B', 'clave ausente: la de ingles');
assert.equal(lookup(D, 'pt', 'c'), undefined);
assert.equal(lookup(D, 'xx', 'a'), 'A', 'idioma sin archivo: ingles');
// Ningun texto de la interfaz debe quedar fuera de los JSON: toda clave usada con t('...') existe.
// Todos los .svelte y .ts de src/ (menos los componentes copiados de shadcn-svelte, que no usan t()).
const srcDir = new URL('../src/', import.meta.url);
const srcFiles = readdirSync(srcDir, { recursive: true })
  .map((f) => String(f).replaceAll('\\', '/'))
  .filter((f) => /\.(svelte|ts)$/.test(f) && !f.startsWith('lib/components/ui/'));
assert.ok(['lib/NavPanel.svelte', 'lib/Home.svelte', 'lib/DrivePicker.svelte', 'lib/GroupList.svelte', 'lib/ItemRow.svelte', 'lib/Inspector.svelte', 'lib/ContainerPanel.svelte'].every((f) => srcFiles.includes(f)), 'el escaneo de claves ve los archivos nuevos');
const src = srcFiles.map((f) => readFileSync(new URL(f, srcDir), 'utf8')).join('\n');
const used = [...src.matchAll(/\bt\(\s*'([\w.-]+)'\s*[,)]/g)].map((m) => m[1]);
assert.deepEqual(used.filter((k) => !(k in es)), [], 'claves usadas en componentes y ausentes en es.json');

// Worktrees: los libres primero (de mayor a menor), luego los protegidos.
const GB = 1073741824;
const wt = (name, gb, protection = []) => ({ path: `C:\\p\\${name}`, repo: 'C:\\p', name, kind: 'git', branch: null, size: gb * GB, modified: null, protection });
const sample = [wt('libre1', 1), wt('prot9', 9, ['recent']), wt('libre5', 5)];
assert.equal(removable(sample[0]), true);
assert.equal(removable(sample[1]), false);
assert.deepEqual(sortWorktrees(sample).map((w) => w.name), ['libre5', 'libre1', 'prot9']);
assert.deepEqual(sample.map((w) => w.name), ['libre1', 'prot9', 'libre5'], 'no muta la lista original');

assert.equal(formatBytes(1073741824, 'es'), '1,0 GB');
assert.equal(formatBytes(1073741824, 'en'), '1.0 GB');
assert.equal(formatBytes(0, 'es'), '0 B');
assert.equal(formatBytes(1536, 'en'), '1.5 KB');
assert.equal(formatBytes(1048575, 'en'), '1.0 MB');
assert.equal(formatBytes(1023, 'en'), '1023 B');
assert.equal(formatBytes(1024, 'es'), '1,0 KB');

const df = ['Images', 'Containers', 'Local Volumes', 'Build Cache'].map((kind) => ({ kind, total: '1', reclaimable: '1' }));
assert.deepEqual(prunableRows(df).map((r) => r.kind), ['Images', 'Containers', 'Build Cache']);

const now = Date.UTC(2026, 10, 13, 12);
const sec = (daysAgo) => (now - daysAgo * 86400000) / 1000;
assert.equal(formatAge(sec(3), now, 'en', en['age.today']), '3 days ago');
assert.equal(formatAge(sec(3), now, 'es', es['age.today']), 'hace 3 días');
// "hoy" sale del JSON, no del codigo: una etiqueta cualquiera pasa tal cual.
assert.equal(formatAge(sec(0.2), now, 'es', 'HOY-X'), 'HOY-X');
assert.equal(formatAge(sec(0.2), now, 'en', en['age.today']), 'today');

// Treemap: dos hijos de 300 y 100 en un lienzo 400x100.
const leaf = (name, size) => ({ name, size, files: 1, is_dir: false, children: [] });
const rects = layout({ name: 'r', size: 400, files: 2, is_dir: true, children: [leaf('a', 300), leaf('b', 100)] }, 'C:\\r', 400, 100);
assert.equal(rects.length, 2);
const area = (r) => r.w * r.h;
assert.ok(Math.abs(area(rects[0]) + area(rects[1]) - 40000) < 400, 'las areas suman el lienzo');
assert.ok(Math.abs(area(rects[0]) / area(rects[1]) - 3) < 0.03, 'proporcion 3:1');
assert.deepEqual(rects.map((r) => r.path).sort(), ['C:\\r\\a', 'C:\\r\\b']);
for (const r of rects) assert.equal(hit(rects, r.x + r.w / 2, r.y + r.h / 2), r);
assert.equal(hit(rects, -1, 50), undefined);
assert.equal(hit(rects, 400, 50), undefined);
assert.equal(hit(rects, 200, 100), undefined);
// Anidado: el rectangulo mas profundo gana el clic, y dos <small> hermanos tienen rectangulos distintos.
const nested = layout({ name: 'r', size: 400, files: 4, is_dir: true, children: [
  { name: 'd', size: 300, files: 2, is_dir: true, children: [leaf('f', 200), leaf('<small>', 100)] }, leaf('<small>', 100)] }, 'C:\\r', 400, 100);
const deep = nested.find((r) => r.name === 'f');
assert.equal(deep.path, 'C:\\r\\d\\f');
assert.equal(deep.depth, 2);
assert.equal(hit(nested, deep.x + deep.w / 2, deep.y + deep.h / 2), deep);
assert.equal(nested.filter((r) => r.name === '<small>').length, 2);
// Franja superior de una carpeta con hijos: los hijos quedan dentro del padre y por debajo de su franja, y un punto en
// la franja es del padre.
const big = layout({ name: 'r', size: 900, files: 6, is_dir: true, children: [
  { name: 'd', size: 600, files: 3, is_dir: true, children: [leaf('x', 300), leaf('y', 200), leaf('z', 100)] }, leaf('w', 300)] }, 'C:\\r', 600, 400);
const parentD = big.find((r) => r.path === 'C:\\r\\d');
const kids = big.filter((r) => parentPath(r.path) === parentD.path);
assert.equal(kids.length, 3);
const eps = 1e-6;
for (const k of kids) {
  assert.ok(k.y >= parentD.y + HEADER - eps, `${k.name} por debajo de la franja`);
  assert.ok(k.x >= parentD.x - eps && k.x + k.w <= parentD.x + parentD.w + eps && k.y + k.h <= parentD.y + parentD.h + eps, `${k.name} dentro del padre`);
}
assert.equal(hit(big, parentD.x + parentD.w / 2, parentD.y + HEADER / 2), parentD, 'la franja es del padre');
// Rutas
assert.ok(isInside('C:\\r\\a', 'C:\\R') && isInside('c:\\', 'C:\\') && !isInside('C:\\rx', 'C:\\r') && !isInside('D:\\r', 'C:\\r'));
assert.equal(parentPath('C:\\r\\a\\b'), 'C:\\r\\a');
assert.equal(parentPath('C:\\r'), 'C:\\');
assert.deepEqual(crumbs('C:\\', 'C:\\Users\\x').map((c) => c.path), ['C:\\', 'C:\\Users', 'C:\\Users\\x']);
assert.deepEqual(crumbs('C:\\r', 'C:\\r').map((c) => c.path), ['C:\\r']);
assert.equal(normalizeRoot('C:/Users/x/'), 'C:\\Users\\x');
assert.equal(normalizeRoot('C:\\'), 'C:\\');
assert.equal(normalizeRoot('c:/'), 'c:\\');
assert.equal(normalizeRoot('C:'), 'C:\\');
assert.equal(normalizeRoot(' D:\\a//b\\ '), 'D:\\a\\b');
// Explorador: orden por tamano o nombre y filtro por nombre dentro de la carpeta actual.
const ent = (name, size) => ({ name, path: `C:\\r\\${name}`, size, files: 1, is_dir: true });
const entries = [ent('beta', 5), ent('<small>', 9), ent('Álbum', 5), ent('alpha10', 1), ent('alpha9', 2)];
const names = (l) => l.map((e) => e.name);
assert.deepEqual(names(sortEntries(entries, 'size', 'es')), ['<small>', 'Álbum', 'beta', 'alpha9', 'alpha10'], 'tamano: mayor primero, empate por nombre');
assert.deepEqual(names(sortEntries(entries, 'name', 'es')), ['Álbum', 'alpha9', 'alpha10', 'beta', '<small>'], 'nombre: natural, sin tildes; lo agrupado al final');
assert.deepEqual(names(entries), ['beta', '<small>', 'Álbum', 'alpha10', 'alpha9'], 'no muta la lista');
assert.deepEqual(names(filterEntries(entries, ' ALB ', 'es')), ['Álbum'], 'sin mayusculas ni tildes');
assert.deepEqual(names(filterEntries(entries, 'alpha', 'es')), ['alpha10', 'alpha9']);
assert.equal(filterEntries(entries, '', 'es').length, 5, 'sin texto: todo');
assert.deepEqual(filterEntries(entries, 'small', 'es'), [], 'lo agrupado no tiene nombre que filtrar');
assert.equal(formatShare(1, 4, 'es'), '25 %');
assert.equal(formatShare(1, 4, 'en'), '25%');
assert.equal(formatShare(1, 1000, 'en'), '<1%');
assert.equal(formatShare(0, 1000, 'en'), '0%');
assert.equal(formatShare(5, 0, 'en'), '0%');

// Color: la tinta de cada escalon de la rampa (tokens de app.css) alcanza 4,5:1.
const css = readFileSync(new URL('../src/app.css', import.meta.url), 'utf8');
const ramp = [...css.matchAll(/--depth-[123]:\s*(#[0-9a-f]{6})/gi)].map((m) => m[1]);
assert.equal(ramp.length, 3, 'tres escalones en app.css');
for (const c of ramp) assert.ok(contrast(labelInk(c), c) >= 4.5, `tinta sobre ${c}`);

// Fichas de la spec ("Rediseño de la interfaz"): un solo tema, el oscuro del :root marcado.
assert.doesNotMatch(css, /prefers-color-scheme/, 'sin tema claro: la interfaz solo esta medida en oscuro');
const tokens = (re, name) => {
  const m = css.match(re);
  assert.ok(m, `bloque de fichas ${name} en app.css`);
  return Object.fromEntries([...m[1].matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})\b/gi)].map((x) => [x[1], x[2]]));
};
const themes = { oscuro: tokens(/\/\* Tema oscuro \*\/\s*:root\s*\{([^}]*)\}/, 'oscuro') };
const TEXT_PAIRS = [
  ...['text', 'text-2', 'success', 'warning', 'danger'].flatMap((fg) => ['bg', 'surface', 'raised'].map((bg) => [fg, bg])),
  ['primary-text', 'primary'],
  ['danger-text', 'danger'],
];
// El primario tambien se usa como texto (enlaces, insignias, iconos con rotulo): 4,5:1 donde aparece como texto.
TEXT_PAIRS.push(['primary', 'bg'], ['primary', 'surface'], ['primary', 'raised']);
const CONTROL_PAIRS = [['control-border', 'bg'], ['control-border', 'surface'], ['control-border', 'raised'], ['primary', 'bg'], ['primary', 'surface']];
for (const [name, tk] of Object.entries(themes)) {
  for (const [pairs, min] of [[TEXT_PAIRS, 4.5], [CONTROL_PAIRS, 3]]) {
    for (const [fg, bg] of pairs) {
      assert.ok(tk[fg] && tk[bg], `faltan --${fg} o --${bg} en el tema ${name}`);
      const c = contrast(tk[fg], tk[bg]);
      assert.ok(c >= min, `${name}: --${fg} sobre --${bg} da ${c.toFixed(2)}:1, se pide ${min}:1`);
    }
  }
}
// Rampa del mapa contra la superficie de su tema (el piso del validador de paletas: 2:1), y de un paso a otro cambia.
for (const [name, tk] of Object.entries(themes)) {
  const steps = ['depth-1', 'depth-2', 'depth-3'].map((k) => tk[k]);
  for (const c of steps) assert.ok(contrast(c, tk.surface) >= 2, `${name}: ${c} sobre la superficie ${tk.surface} da ${contrast(c, tk.surface).toFixed(2)}:1`);
  assert.equal(new Set(steps.map((c) => c.toLowerCase())).size, 3, `${name}: tres escalones distintos`);
}
// La spec fija estos valores; solo cambian si la medicion lo exige (y entonces se anota en la spec).
assert.deepEqual(
  ['bg', 'surface', 'raised', 'text', 'text-2', 'primary'].map((k) => themes.oscuro[k]?.toUpperCase()),
  ['#0B1220', '#111B2E', '#172338', '#E8EEF7', '#A3B3CC', '#22C9E0'],
  'fichas del tema oscuro segun la spec',
);

// Pantallas nuevas: navegacion, Acerca de y la marca de demostracion existen en los dos idiomas.
const NEW_KEYS = [
  'nav.home', 'nav.clean', 'nav.explore', 'nav.map', 'nav.about', 'nav.main', 'nav.analyze', 'nav.noScan', 'nav.emptyGroups', 'demo.badge',
  'home.lead', 'home.recover', 'home.review', 'home.oneByOne',
  'about.title', 'about.lead', 'about.version', 'about.description', 'about.protectsTitle', 'about.protects.1',
  'about.protects.2', 'about.protects.3', 'about.doesNotTitle', 'about.doesNot.1', 'about.doesNot.2', 'about.license',
];
for (const [name, d] of [['es', es], ['en', en]]) assert.deepEqual(NEW_KEYS.filter((k) => !(k in d)), [], `faltan claves nuevas en ${name}.json`);
// Selector de unidad, resumen de recuperacion y limpieza por seleccion.
const T16_KEYS = [
  ...['fixed', 'removable', 'network', 'optical', 'ram_disk', 'unknown', 'system', 'usb'].map((k) => `drive.kind.${k}`),
  ...['network', 'optical', 'unknown', 'empty'].map((k) => `drive.no.${k}`),
  'bar.count.one', 'bar.count.other', 'bar.clean', 'bar.none',
  'status.removable', 'status.protected', 'status.info',
  'filter.label', 'batch.stopped',
];
for (const [name, d] of [['es', es], ['en', en]]) assert.deepEqual(T16_KEYS.filter((k) => !(k in d)), [], `faltan claves de seleccion en ${name}.json`);
// Las cotas superiores dicen "hasta" (o "up to") en los textos que muestran una cifra de lo que se recupera.
for (const k of ['bar.count.one', 'bar.count.other', 'bar.clean', 'bar.after', 'group.cleanable', 'detail.upTo', 'clean.overview.go', 'clean.overview.othersLabel']) {
  assert.match(es[k] ?? '', /hasta/i, `es: ${k}`);
  assert.match(en[k] ?? '', /up to/i, `en: ${k}`);
}

// Demostracion: datos suficientes para juzgar el diseno, y coherentes con los catalogos de este archivo.
const demo = await import('../src/lib/demo.ts');
const drives = demo.DRIVES;
assert.ok(drives.length >= 3, 'tres unidades o mas');
assert.ok(drives.some((d) => d.system && d.kind === 'fixed'), 'unidad del sistema fija');
assert.ok(drives.some((d) => !d.system && d.kind === 'fixed'), 'segundo disco fijo');
assert.ok(drives.some((d) => d.kind === 'removable' && d.usb), 'una extraible USB');
for (const d of drives) assert.ok(d.free <= d.total && /^[A-Z]:\\$/.test(d.root), d.root);
const kinds = new Set(demo.CATEGORIES.map((c) => c.kind));
for (const k of ['contents', 'subfolders', 'project', 'containers', 'worktrees', 'info_only']) assert.ok(kinds.has(k), `categoria de tipo ${k}`);
assert.deepEqual(demo.CATEGORIES.map((c) => c.id).filter((i) => !CATEGORY_IDS.includes(i)), [], 'ids de categoria conocidos');
assert.ok(new Set(demo.CATEGORIES.map((c) => c.ecosystem)).size >= 6, 'varios ecosistemas');
assert.ok(demo.CATEGORIES.find((c) => c.kind === 'subfolders').items.filter((i) => i.modified != null).length >= 2, 'subcarpetas con ultima actividad');
const wts = demo.WORKTREES.worktrees;
assert.ok(wts.some(removable) && wts.some((w) => !removable(w)), 'worktrees libres y protegidos');
assert.ok(new Set(wts.flatMap((w) => w.protection)).size >= 3, 'motivos de proteccion distintos');
assert.ok(demo.WORKTREES.orphans.length >= 1, 'una carpeta huerfana');
assert.ok(demo.DOCKER.installed && demo.DOCKER.disks.length >= 1, 'Docker con disco virtual');
const depth = (n) => (n.children.length ? 1 + Math.max(...n.children.map(depth)) : 0);
assert.ok(depth(demo.TREE) >= 4, 'arbol con cuatro niveles o mas');
assert.ok(drives.some((d) => d.kind === 'network'), 'una unidad de red');
assert.ok(drives.some((d) => d.kind === 'optical' && d.total === 0), 'una unidad optica vacia');
const sample2 = await demo.demoInvoke('remove_worktrees', { paths: wts.filter(removable).map((w) => w.path), minAgeHours: 168 });
assert.ok(sample2.freed > 0 && sample2.deleted >= 1 && sample2.skipped.length >= 1, 'quitar worktrees devuelve un informe con saltados');
const sample3 = await demo.demoInvoke('clean_category', { id: 'npm', items: [], minAgeHours: 168 });
assert.ok(sample3.freed > 0, 'limpiar una cache devuelve lo liberado');

// Resumen de recuperacion (recovery.ts)
const { driveOf, recovery, selectable, noReason, folderRule, wtStale, plan, bytesOn } = await import('../src/lib/recovery.ts');
const drv = (root, kind, extra = {}) => ({ root, label: '', kind, usb: false, system: false, filesystem: 'NTFS', total: 476 * GB, free: 77.7 * GB, ...extra });
const r = recovery(drv('C:\\', 'fixed'), 45.2 * GB);
const near = (a, b) => Math.abs(a - b) <= 0.1 * GB;
assert.ok(near(r.freeNow, 77.7 * GB) && near(r.recover, 45.2 * GB), 'libre ahora y recuperas');
assert.ok(near(r.freeAfter, 122.9 * GB), `libre despues: ${r.freeAfter / GB}`);
assert.ok(near(r.usedAfter, 353.1 * GB), `ocupado despues: ${r.usedAfter / GB}`);
const capped = recovery(drv('C:\\', 'fixed'), 900 * GB);
assert.ok(near(capped.recover, (476 - 77.7) * GB) && near(capped.freeAfter, 476 * GB) && capped.usedAfter === 0, 'recuperas nunca supera lo ocupado');
assert.equal(selectable(drv('C:\\', 'fixed')), 'yes');
assert.equal(selectable(drv('C:\\', 'fixed', { system: true })), 'yes');
assert.equal(selectable(drv('F:\\', 'fixed', { usb: true })), 'confirm');
assert.equal(selectable(drv('E:\\', 'removable')), 'confirm');
for (const k of ['network', 'optical', 'unknown']) assert.equal(selectable(drv('Z:\\', k)), 'no', k);
assert.equal(selectable(drv('D:\\', 'fixed', { total: 0, free: 0 })), 'no', 'total 0');
// "Otra carpeta" pasa por la unidad que la contiene: una ruta en la extraible pide confirmar, en la de red se rechaza.
const pool = [drv('C:\\', 'fixed', { system: true }), drv('E:\\', 'removable', { usb: true }), drv('Z:\\', 'network')];
assert.equal(selectable(driveOf('e:\\fotos\\2024', pool)), 'confirm');
assert.equal(selectable(driveOf('Z:\\share', pool)), 'no');
assert.equal(noReason(driveOf('Z:\\share', pool)), 'drive.no.network');
assert.equal(noReason(drv('D:\\', 'fixed', { total: 0 })), 'drive.no.empty');
// Otra carpeta: solo se escanea si cae en una unidad de la lista cargada cuya regla es yes o confirm.
const fr = (p, list = pool) => folderRule(p, list).rule;
assert.equal(fr('C:\\Users\\x'), 'yes');
assert.equal(fr('E:\\fotos'), 'confirm');
assert.equal(folderRule('E:\\fotos', pool).reason, 'drive.confirmNote');
assert.equal(fr('Z:\\share'), 'no');
assert.equal(folderRule('Z:\\share', pool).reason, 'drive.no.network');
for (const p of ['\\\\server\\share\\x', '\\\\?\\UNC\\server\\share', 'Q:\\x', 'carpeta\\relativa', '']) assert.equal(fr(p), 'no', `rechaza ${JSON.stringify(p)}`);
assert.equal(folderRule('\\\\server\\share\\x', pool).reason, 'drive.no.folder');
assert.equal(fr('C:\\Users\\x', null), 'no', 'sin lista de unidades todavia');
assert.equal(folderRule('C:\\Users\\x', null).reason, 'drive.no.loading');
assert.equal(fr('C:\\Users\\x', []), 'no', 'lista vacia');
assert.equal(fr('\\\\?\\C:\\x'), 'yes', '\\\\?\\ delante de una letra se ignora');
assert.equal(driveOf('\\\\?\\C:\\x', pool)?.root, 'C:\\');
assert.equal(driveOf('\\\\?\\UNC\\server\\share', pool), undefined);
// Worktrees clasificados con otra antiguedad: solo bloquea si el plan los incluye y hay lista con otra antiguedad.
const wtE = { id: 'git-worktrees', kind: 'worktrees', usesMinAge: true, items: [], size: 1 };
const npmE = { id: 'npm', kind: 'contents', usesMinAge: false, items: [], size: 1 };
assert.equal(wtStale([npmE, wtE], { list: { git: true, worktrees: [], orphans: [] }, age: 168 }, 72), true);
assert.equal(wtStale([npmE, wtE], { list: { git: true, worktrees: [], orphans: [] }, age: 72 }, 72), false, 'misma antiguedad');
assert.equal(wtStale([npmE], { list: { git: true, worktrees: [], orphans: [] }, age: 168 }, 72), false, 'sin worktrees en el plan');
assert.equal(wtStale([npmE, wtE], { list: null, age: -1 }, 72), false, 'sin lista');
const drvs = [drv('D:\\', 'fixed'), drv('C:\\', 'fixed', { system: true })];
assert.equal(driveOf('c:\\Users\\x', drvs)?.root, 'C:\\');
assert.equal(driveOf('C:\\', drvs)?.root, 'C:\\');
assert.equal(driveOf('E:\\x', drvs), undefined);

// Plan de limpieza: solo lo seleccionable que sigue en los datos cargados.
const fnd = (category, path, gb) => ({ category, path, size: gb * GB, modified: null });
const C = (id, kind, size, items = [], usesMinAge = false) => ({ id, ecosystem: 'x', kind, uses_min_age: usesMinAge, size: size * GB, paths: [], items });
const cats = [
  C('npm', 'contents', 9),
  C('temp-user', 'contents', 15, [], true),
  C('node-projects', 'project', 3, [fnd('node-projects', 'C:\\p\\a\\node_modules', 2), fnd('node-projects', 'C:\\p\\b\\node_modules', 1)]),
  C('claude-sessions', 'subfolders', 1, [fnd('claude-sessions', 'C:\\t\\s1', 1)], true),
  C('ollama', 'info_only', 14),
  C('docker', 'containers', 80),
  C('git-worktrees', 'worktrees', 0, [], true),
];
const wtl = [wt('libre1', 1), wt('prot9', 9, ['recent']), wt('libre5', 5)];
const pick = {
  categories: new Set(['npm', 'ollama', 'docker', 'gone']),
  items: new Map([
    ['node-projects', new Set(['C:\\p\\a\\node_modules', 'C:\\p\\zz\\node_modules'])],
    ['git-worktrees', new Set([wtl[0].path, wtl[1].path, 'C:\\p\\viejo'])],
    ['claude-sessions', new Set()],
  ]),
};
const p = plan(cats, wtl, pick);
assert.deepEqual(p.map((e) => e.id), ['npm', 'node-projects', 'git-worktrees'], 'categorias del plan, en orden');
assert.deepEqual(p[0].items, [], 'contenido: se envia sin elementos');
assert.equal(p[0].size, 9 * GB);
assert.deepEqual(p[1].items.map((i) => i.path), ['C:\\p\\a\\node_modules'], 'rutas que ya no estan se descartan');
assert.equal(p[1].size, 2 * GB);
assert.deepEqual(p[2].items.map((i) => i.path), [wtl[0].path], 'worktrees protegidos o desaparecidos se descartan');
assert.equal(p[2].kind, 'worktrees');
assert.equal(plan(cats, null, pick).length, 2, 'sin lista de worktrees no se envia ninguno');
// Solo cuenta para una unidad lo que esta en ella.
cats[0].paths = ['C:\\Users\\x\\AppData\\Local\\npm-cache'];
assert.equal(bytesOn(p, cats, 'C:\\'), 9 * GB + 2 * GB + 1 * GB, 'cache, carpeta de proyecto y worktree en C:');
assert.equal(bytesOn(p, cats, 'D:\\'), 0, 'nada seleccionado esta en D:');
assert.deepEqual(plan(cats, wtl, { categories: new Set(), items: new Map() }), []);

// Lo que se puede limpiar por ecosistema (barras de Limpieza): solo lo limpiable, de mayor a menor, sin grupos en cero.
const { groupTotals, topGroups } = await import('../src/lib/recovery.ts');
const G = (id, ecosystem, kind, gb) => ({ ...C(id, kind, gb), ecosystem });
const eco = [
  G('npm', 'node', 'contents', 9),
  G('node-projects', 'node', 'project', 3),
  G('pip', 'python', 'contents', 2),
  G('claude-sessions', 'ai-agents', 'subfolders', 4),
  G('claude-desktop-data', 'ai-agents', 'info_only', 40),
  G('docker', 'containers', 'containers', 80),
  G('ollama', 'local-ai', 'info_only', 14),
  G('go-build', 'go', 'contents', 0),
  G('git-worktrees', 'git', 'worktrees', 0),
];
assert.deepEqual(groupTotals(eco, wtl), [
  { ecosystem: 'node', bytes: 12 * GB },
  { ecosystem: 'git', bytes: 6 * GB },
  { ecosystem: 'ai-agents', bytes: 4 * GB },
  { ecosystem: 'python', bytes: 2 * GB },
], 'limpiable por ecosistema: worktrees protegidos, informativas, contenedores y ceros fuera');
assert.deepEqual(groupTotals(eco, []).map((g) => g.ecosystem), ['node', 'ai-agents', 'python'], 'sin worktrees que quitar no hay grupo git');
const eight = [8, 7, 6, 5, 4, 3, 2, 1].map((n) => ({ ecosystem: 'e' + n, bytes: n }));
assert.deepEqual(topGroups(eight, 6), [...eight.slice(0, 6), { ecosystem: null, bytes: 3 }], 'seis y un resto con los dos menores');
assert.deepEqual(topGroups(eight.slice(0, 4), 6), eight.slice(0, 4), 'cuatro grupos: sin resto');
assert.deepEqual(topGroups(eight.slice(0, 6), 6), eight.slice(0, 6), 'justo seis: sin resto');

// Lista de worktrees: cuando hay que (re)cargarla. Una sola regla para Limpieza y Resumen.
const { wtNeedsLoad, magnitudeRef } = await import('../src/lib/recovery.ts');
const fresh = { list: { git: true, worktrees: [], orphans: [] }, rev: 3, age: 168 };
assert.equal(wtNeedsLoad(fresh, 3, 168, false, true), false, 'lista al dia para esta revision y antiguedad: no');
assert.equal(wtNeedsLoad(fresh, 4, 168, false, true), true, 'revision vieja: si');
assert.equal(wtNeedsLoad(fresh, 3, 72, false, true), true, 'otra antiguedad: si');
assert.equal(wtNeedsLoad({ list: null, rev: -1, age: -1 }, 3, 168, false, true), true, 'sin lista: si');
assert.equal(wtNeedsLoad({ list: null, rev: -1, age: -1 }, 3, 168, true, true), false, 'escaneo en curso: no');
assert.equal(wtNeedsLoad({ list: null, rev: -1, age: -1 }, 3, 168, false, false), false, 'sin categoria de worktrees: no');
// Barras de magnitud de las filas: la referencia es la fila mayor que no es de contenedores.
assert.equal(magnitudeRef([{ kind: 'containers', size: 82 }, { kind: 'contents', size: 20 }, { kind: 'info_only', size: 30 }]), 30, 'Docker no aplana las demas');
assert.equal(magnitudeRef([{ kind: 'containers', size: 82 }]), 1, 'sin otras filas: referencia minima');
assert.equal(magnitudeRef([]), 1);

// Consecuencias de limpiar (impact.ts): un nivel por categoria limpiable, y Docker y Podman.
const { IMPACT, impactOf, firstSentence, sentences } = await import('../src/lib/impact.ts');
assert.deepEqual(sentences('Ejecuta npm install. Con «Todas» sale. .next y terraform.tfstate quedan.'), ['Ejecuta npm install.', 'Con «Todas» sale. .next y terraform.tfstate quedan.']);
for (const d of [es, en]) assert.equal(sentences(d['cat.docker.effect']).length, 3, 'Docker: limpiar, todas las imagenes y compactar');
assert.deepEqual(CLEANABLE_IDS.filter((i) => !CATEGORY_IDS.includes(i)), [], 'ids limpiables del catalogo');
assert.deepEqual(Object.keys(IMPACT).sort(), [...CLEANABLE_IDS, 'docker', 'podman'].sort(), 'un nivel por categoria limpiable');
assert.ok(Object.values(IMPACT).every((l) => LEVELS.includes(l)), 'niveles conocidos');
assert.equal(impactOf('no-existe'), 'careful');
assert.equal(impactOf('npm'), IMPACT.npm);
assert.equal(firstSentence('Se borra. Vuelve solo.'), 'Se borra.');
assert.equal(firstSentence('Ejecuta npx playwright install. Luego.'), 'Ejecuta npx playwright install.');
assert.equal(firstSentence('Sin punto'), 'Sin punto');
// Docker y Podman distinguen limpiar, todas las imagenes y compactar.
for (const [d, words] of [[es, [/volumen/i, /todas las imágenes/i, /compactar/i, /WSL/]], [en, [/volume/i, /all unused images/i, /compact/i, /WSL/]]]) {
  for (const id of ['docker', 'podman']) for (const w of words) assert.match(d[`cat.${id}.effect`], w, `${id}: ${w}`);
}

// runPlan (selection.svelte.ts), la unica puerta al borrado definitivo. Se carga con su estado y su API sustituidos: el
// modulo real de la API necesita Vite/Tauri y state.svelte usa runas. Las dos funciones de la API se le pasan.
const STUBS = {
  'svelte/reactivity': 'export const SvelteSet = Set; export const SvelteMap = Map;',
  './api': "export const errCode = (e) => (typeof e === 'string' ? e : 'command_failed'); const no = () => Promise.reject('stub'); export const cleanCategory = no; export const removeWorktrees = no; export const onCleanProgress = (cb) => { globalThis.__subs.push(cb); return Promise.resolve(() => { globalThis.__unsubs++; }); };",
  './state.svelte': 'export const work = globalThis.__work; export const dataRev = globalThis.__dataRev;',
};
registerHooks({
  resolve(spec, ctx, next) {
    // pick.ts, nav.ts y levels.ts importan a sus vecinos sin extension, como lo resuelve Vite.
    if (/\/(pick|nav|levels)\.ts$/.test(ctx.parentURL ?? '') && /^\.\/[a-z]+$/.test(spec)) return { url: new URL(`../src/lib/${spec.slice(2)}.ts`, import.meta.url).href, shortCircuit: true };
    if (!ctx.parentURL?.endsWith('/selection.svelte.ts')) return next(spec, ctx);
    if (spec in STUBS) return { url: 'data:text/javascript,' + encodeURIComponent(STUBS[spec]), shortCircuit: true };
    if (spec === './recovery') return { url: new URL('../src/lib/recovery.ts', import.meta.url).href, shortCircuit: true };
    return next(spec, ctx);
  },
});
const work = (globalThis.__work = { busy: false, outcome: null, cats: null, wt: { list: null }, progress: null, cleaned: {} });
globalThis.__subs = [];
globalThis.__unsubs = 0;
globalThis.__dataRev = { n: 0 };
const sel = await import('../src/lib/selection.svelte.ts');
const E = (id, kind = 'contents') => ({ id, kind, usesMinAge: false, items: kind === 'contents' ? [] : [{ path: `C:\\${id}`, size: 1 }], size: 1 });
const ok = { freed: 1, deleted: 1, skipped: [] };
let calls = [];
const secondFails = (id) => (calls.push(id), calls.length === 2 ? Promise.reject('blocked') : Promise.resolve(ok));
sel.setCategory('npm', true);
await sel.runPlan([E('npm'), E('pip'), E('uv')], 168, secondFails, secondFails);
assert.deepEqual(calls, ['npm', 'pip'], 'se detiene en el primer fallo');
assert.deepEqual(work.outcome.results.map((x) => x.id), ['npm'], 'lo que corrio se informa');
assert.deepEqual(work.outcome.error, { id: 'pip', code: 'blocked' }, 'el error nombra la segunda categoria');
assert.deepEqual(work.outcome.notRun, ['uv'], 'la tercera no llego a correr');
assert.equal(work.busy, false, 'busy se suelta');
assert.equal(sel.selection.categories.size, 0, 'algo corrio: la seleccion se vacia');
// Los worktrees van por su propia funcion, con sus rutas y la antiguedad.
calls = [];
const wtCalls = [];
await sel.runPlan([E('git-worktrees', 'worktrees')], 72, secondFails, (paths, age) => (wtCalls.push([paths, age]), Promise.resolve(ok)));
assert.deepEqual([calls, wtCalls], [[], [[['C:\\git-worktrees'], 72]]], 'worktrees por removeWorktrees');
// Con busy no corre nada.
calls = [];
work.busy = true;
work.outcome = 'antes';
await sel.runPlan([E('npm')], 168, secondFails, secondFails);
assert.deepEqual(calls, [], 'con busy no corre nada');
assert.equal(work.outcome, 'antes', 'con busy no toca el informe');
assert.equal(work.busy, true, 'con busy no suelta el bloqueo de otro');
work.busy = false;
// El primer comando rechaza y no corrio nada (p. ej. scan_running): la seleccion se conserva.
sel.setCategory('npm', true);
await sel.runPlan([E('npm'), E('pip')], 168, () => Promise.reject('scan_running'), secondFails);
assert.deepEqual([work.outcome.results, work.outcome.error, work.outcome.notRun], [[], { id: 'npm', code: 'scan_running' }, ['pip']]);
assert.ok(sel.selection.categories.has('npm'), 'nada corrio: la seleccion se conserva');
assert.equal(work.busy, false);
// Progreso: se escucha clean-progress durante la ejecucion y siempre se deja de escuchar; cada categoria anota su informe.
const subsBefore = globalThis.__subs.length;
const seenProgress = [];
const withEvents = (id) => {
  const cb = globalThis.__subs.at(-1);
  cb({ id: 'otra', freed: 99, deleted: 99 }); // de otra categoria: se ignora
  cb({ id, freed: 5, deleted: 2 });
  seenProgress.push({ ...work.progress });
  return id === 'pip' ? Promise.reject('blocked') : Promise.resolve({ freed: 7, deleted: 3, skipped: [{ path: 'C:\\x', reason: 'in_use' }] });
};
work.cleaned = {};
work.cats = [{ ...C('npm', 'contents', 0), size: 10 }, C('node-projects', 'project', 3)];
await sel.runPlan([{ ...E('npm'), size: 10 }, E('pip')], 168, withEvents, secondFails);
assert.equal(globalThis.__subs.length, subsBefore + 1, 'una suscripcion por ejecucion');
assert.equal(globalThis.__unsubs, globalThis.__subs.length, 'siempre se deja de escuchar, tambien tras un fallo');
assert.deepEqual(seenProgress.map((p) => [p.index, p.total, p.id, p.freed, p.deleted, p.expected]), [[1, 2, 'npm', 5, 2, 10], [2, 2, 'pip', 5, 2, 1]]);
assert.equal(work.progress, null, 'sin progreso al terminar');
assert.deepEqual(work.cleaned.npm, { freed: 7, deleted: 3, skipped: 1, before: 10, kind: 'contents', ecosystem: 'x', paths: [] }, 'informe propio de la categoria');
assert.equal(work.cleaned.pip, undefined, 'la que fallo no queda como limpiada');
// Con busy no se escucha nada.
work.busy = true;
await sel.runPlan([E('npm')], 168, withEvents, secondFails);
assert.equal(globalThis.__subs.length, subsBefore + 1, 'con busy no se suscribe');
work.busy = false;
// `before` es el tamano de la fila al pulsar (no el de lo elegido): una limpieza parcial no la cambia de sitio.
await sel.runPlan([E('node-projects', 'project')], 168, withEvents, secondFails);
assert.equal(work.cleaned['node-projects'].before, 3 * GB, 'limpieza parcial: before es el tamano de toda la fila');
assert.deepEqual(seenProgress.at(-1).expected, 1, 'la barra sigue usando lo elegido');
work.wt.list = { git: true, worktrees: [wt('w1', 2), wt('w2', 5, ['recent'])], orphans: [] };
await sel.runPlan([E('git-worktrees', 'worktrees')], 168, withEvents, () => Promise.resolve(ok));
assert.equal(work.cleaned['git-worktrees'].before, 7 * GB, 'worktrees: la suma de la lista, como la fila');
// Limpiar otra vez la misma categoria acumula lo liberado y conserva el primer `before`.
work.cats[0].size = 2;
await sel.runPlan([{ ...E('npm'), size: 2 }], 168, withEvents, secondFails);
assert.deepEqual([work.cleaned.npm.freed, work.cleaned.npm.deleted, work.cleaned.npm.skipped, work.cleaned.npm.before], [14, 6, 2, 10], 'se acumula en la sesion');

// Claves que el codigo arma o elige con un ternario (el escaneo de t('...') no las ve).
const T17_KEYS = [
  'map.depth.1', 'map.depth.2', 'map.depth.3', 'explore.kind.dir', 'explore.kind.file', 'explore.kind.small',
  'explore.summary.one', 'explore.summary.other', 'home.notScanned', 'home.target',
  'drive.no.loading', 'drive.no.folder', 'drive.confirmNote', 'sel.waitWorktrees',
];
for (const [name, d] of [['es', es], ['en', en]]) assert.deepEqual(T17_KEYS.filter((k) => !(k in d)), [], `faltan claves de Explorador y Mapa en ${name}.json`);

// Plurales: una forma por cantidad, como clean.days.*.
for (const k of ['bar.count', 'sel.confirmTitle', 'group.count']) {
  for (const d of [es, en]) assert.ok(d[`${k}.one`] && d[`${k}.other`] && !(k in d), `${k}.one/.other`);
  assert.doesNotMatch(es[`${k}.one`], /seleccionados|marcados|categorías/, `es: ${k}.one en singular`);
}

// --- Task 21: aviso de uso, seleccion por grupo y rapida, proyectos inactivos y teclado ---
const { TERMS_VERSION, termsAccepted, termsRecord } = await import('../src/lib/terms.ts');
assert.equal(termsAccepted(null), false, 'sin registro');
assert.equal(termsAccepted('no es json'), false, 'texto que no es JSON');
assert.equal(termsAccepted(JSON.stringify({ version: TERMS_VERSION + 1, at: 1 })), false, 'otra version');
assert.equal(termsAccepted('null'), false, 'JSON nulo');
assert.equal(termsAccepted(termsRecord(now)), true, 'registro de termsRecord');
assert.deepEqual(JSON.parse(termsRecord(now)), { version: TERMS_VERSION, at: now });

const { groupTargets, quickPick, staleItems } = await import('../src/lib/pick.ts');
const E2 = (id, ecosystem, kind, gb, items = []) => ({ ...C(id, kind, gb, items), ecosystem });
const mix = [
  E2('npm', 'node', 'contents', 9),
  E2('yarn', 'node', 'contents', 2), // careful
  E2('node-projects', 'node', 'project', 3, [fnd('node-projects', 'C:\\p\\a\\node_modules', 2), fnd('node-projects', 'C:\\p\\b\\node_modules', 1)]),
  E2('playwright', 'node', 'contents', 1), // careful
  E2('temp-user', 'system', 'contents', 15),
  E2('temp-windows', 'system', 'contents', 0), // vacia: nada que marcar
  E2('go-build', 'go', 'contents', 2),
  E2('go-mod', 'go', 'contents', 1),
  E2('rust-target', 'rust', 'project', 4, [fnd('rust-target', 'C:\\r\\target', 4)]),
  E2('claude-sessions', 'ai-agents', 'subfolders', 1, [fnd('claude-sessions', 'C:\\t\\s1', 1)]), // careful
  E2('antigravity-cache', 'ai-agents', 'contents', 1),
  E2('vscode', 'editors', 'subfolders', 1, [fnd('vscode', 'C:\\v\\s1', 1)]), // subcarpetas de nivel auto (dato de prueba)
  E2('claude-code', 'ai-agents', 'info_only', 6),
  E2('docker', 'containers', 'containers', 80),
  E2('git-worktrees', 'git', 'worktrees', 6),
];
assert.deepEqual(groupTargets(mix, 'node'), { categories: ['npm'], items: { 'node-projects': ['C:\\p\\a\\node_modules', 'C:\\p\\b\\node_modules'] } }, 'grupo node: contenido y proyecto, sin careful');
assert.deepEqual(groupTargets(mix, 'ai-agents'), { categories: ['antigravity-cache'], items: {} }, 'sin subcarpetas careful ni informativas');
assert.deepEqual(groupTargets(mix, 'editors'), { categories: [], items: { vscode: ['C:\\v\\s1'] } }, 'subcarpetas que no son careful');
assert.deepEqual(groupTargets(mix, 'git'), { categories: [], items: {} }, 'nunca worktrees');
assert.deepEqual(groupTargets(mix, 'containers'), { categories: [], items: {} }, 'nunca contenedores');
assert.deepEqual(groupTargets(mix, 'system'), { categories: ['temp-user'], items: {} }, 'sin categorias vacias');
assert.deepEqual(groupTargets(mix, 'rust').items, { 'rust-target': ['C:\\r\\target'] }, 'solo el ecosistema pedido');
assert.deepEqual(quickPick('auto', mix), ['temp-user', 'go-build', 'antigravity-cache'], 'auto: solo contenido de nivel auto');
assert.deepEqual(quickPick('redownload', mix), ['npm', 'temp-user', 'go-build', 'go-mod', 'antigravity-cache'], 'redownload: auto y redownload');
for (const lvl of ['auto', 'redownload']) {
  const ids = quickPick(lvl, mix);
  assert.ok(ids.every((i) => mix.find((c) => c.id === i).kind === 'contents'), `${lvl}: solo contenido`);
  assert.ok(ids.every((i) => !['rebuild', 'careful'].includes(impactOf(i))), `${lvl}: nunca rebuild ni careful`);
}
const NOW = Date.UTC(2026, 10, 14, 12);
const at = (d) => (d == null ? null : (NOW - d * 86400000) / 1000);
const aged = [['C:\\nuevo', 3], ['C:\\justo', 90], ['C:\\viejo', 91], ['C:\\sin', null], ['C:\\antiguo', 400]].map(([path, d]) => ({ category: 'x', path, size: 1, modified: at(d) }));
assert.deepEqual(staleItems(aged, 90, NOW), ['C:\\viejo', 'C:\\antiguo'], 'mas viejos que el limite; el limite exacto y sin fecha no');
assert.deepEqual(staleItems(aged, 1000, NOW), []);

const { keyAction } = await import('../src/lib/keys.ts');
assert.equal(keyAction({ key: '/', target: 'other' }), 'focus-filter');
assert.equal(keyAction({ key: '/', target: 'row' }), 'focus-filter');
for (const key of ['/', ' ', 'Enter', 'ArrowDown', 'ArrowUp', 'a']) assert.equal(keyAction({ key, target: 'input' }), null, `${key} en un campo`);
assert.equal(keyAction({ key: ' ', target: 'row' }), 'toggle');
assert.equal(keyAction({ key: 'Enter', target: 'row' }), 'expand');
assert.equal(keyAction({ key: 'ArrowDown', target: 'row' }), 'next');
assert.equal(keyAction({ key: 'ArrowUp', target: 'row' }), 'prev');
assert.equal(keyAction({ key: ' ', target: 'other' }), null, 'espacio fuera de una fila: lo nativo');
assert.equal(keyAction({ key: 'Enter', target: 'other' }), null, 'Enter fuera de una fila: lo nativo');
assert.equal(keyAction({ key: 'x', target: 'row' }), null);

const T21_KEYS = [
  'terms.title', 'terms.lead', 'terms.point.1', 'terms.point.2', 'terms.point.3', 'terms.point.4', 'terms.point.5', 'terms.license',
  'terms.copyright', 'terms.check', 'terms.accept', 'terms.close', 'about.copyright', 'about.licenseName', 'about.termsAccepted',
  'about.termsNotAccepted', 'about.readTerms',
  'group.checkLabel', 'group.leavesOut', 'tray.title', 'tray.remove',
  'project.unknownDate', 'project.staleNote', 'list.more', 'list.partial', 'keys.help', 'group.expand', 'group.collapse',
  // Lista del grupo y detalle: claves que se arman con un prefijo
  ...PROTECTIONS.map((p) => `wt.short.${p}`), ...LEVELS.map((l) => `impact.${l}.note`),
];
for (const [name, d] of [['es', es], ['en', en]]) assert.deepEqual(T21_KEYS.filter((k) => !(k in d)), [], `faltan claves de la tarea 21 en ${name}.json`);
// Titular y licencia, iguales en los dos idiomas.
for (const d of [es, en]) {
  assert.match(d['terms.copyright'], /Copyright © 2026 Pergamio/);
  assert.match(d['terms.license'], /GPL-3\.0-or-later/);
  assert.match(d['terms.point.5'], /Pergamio/);
}
// Demostracion: fechas de proyecto de dias a meses y una lista larga.
const projDays = demo.CATEGORIES.filter((c) => c.kind === 'project').flatMap((c) => c.items).map((i) => i.modified);
assert.ok(projDays.every((m) => m != null), 'carpetas de proyecto con fecha');
const nowS = Date.now() / 1000;
assert.ok(projDays.some((m) => nowS - m < 7 * 86400) && projDays.some((m) => nowS - m > 120 * 86400), 'de dias a meses');
assert.ok(demo.CATEGORIES.some((c) => c.items.length > 100), 'una lista de mas de 100 elementos');

// Mesa de trabajo: navegacion, niveles y teclas nuevas (nav.ts, levels.ts, keys.ts)
const { navGroups, resolveGroup, validFocus, barVisible, scanPercent } = await import('../src/lib/nav.ts');
// Progreso del escaneo: lo leido frente a lo ocupado en la unidad, solo si se escanea la unidad entera.
const DRV = { root: 'C:\\', total: 500 * GB, free: 100 * GB };
assert.equal(scanPercent(0, DRV, 'C:\\'), 0, 'empieza en cero');
assert.equal(scanPercent(200 * GB, DRV, 'C:\\'), 50);
assert.equal(scanPercent(200 * GB, DRV, 'c:'), 50, 'la raiz se compara sin barra final ni mayusculas');
assert.equal(scanPercent(450 * GB, DRV, 'C:\\'), 99, 'nunca llega a 100 antes de terminar');
assert.equal(scanPercent(10 * GB, DRV, 'C:\\Users\\dev'), null, 'carpeta suelta: sin total conocido');
assert.equal(scanPercent(10 * GB, undefined, 'C:\\'), null);
assert.equal(scanPercent(10 * GB, { root: 'C:\\', total: 100, free: 100 }, 'C:\\'), null);
// Barra de accion: con un escaneo valido (tambien tras cancelar otro) o limpiando; nunca escaneando ni sin escaneo.
assert.equal(barVisible('C:\\', 'idle', false), true, 'escaneo cancelado con datos anteriores');
assert.equal(barVisible('', 'idle', false), false, 'sin escaneo');
assert.equal(barVisible('C:\\', 'running', false), false, 'escaneando');
assert.equal(barVisible('C:\\', 'done', false), true);
assert.equal(barVisible('', 'idle', true), true, 'limpiando');
const { levelTotals, levelPick, levelHasItems, levelGroup } = await import('../src/lib/levels.ts');
const mk = (id, ecosystem, kind, size, items = []) => ({ id, ecosystem, kind, uses_min_age: false, size, paths: [], items });
const it = (category, path, size) => ({ category, path, size, modified: null });
const CATS = [
  mk('temp-user', 'system', 'contents', 12 * GB),
  mk('npm', 'node', 'contents', 9 * GB),
  mk('yarn', 'node', 'contents', 1 * GB),
  mk('node-projects', 'node', 'project', 3 * GB, [it('node-projects', 'C:\\p\\a\\node_modules', 3 * GB)]),
  mk('docker', 'containers', 'containers', 80 * GB),
  mk('git-worktrees', 'git', 'worktrees', 0),
  mk('zig', 'zig', 'contents', 0),
  mk('cargo', 'rust', 'contents', 0),
];
const WTS = [wt('prot', 2, ['recent'])];
const nav = navGroups(CATS, WTS);
assert.deepEqual(nav.active, [
  { ecosystem: 'node', bytes: 13 * GB }, { ecosystem: 'system', bytes: 12 * GB },
  { ecosystem: 'containers', bytes: 0 }, { ecosystem: 'git', bytes: 0 },
]);
assert.deepEqual(nav.empty, ['rust', 'zig']);
assert.deepEqual(navGroups([], []), { active: [], empty: [] });
assert.equal(resolveGroup('node', nav), 'node');
assert.equal(resolveGroup('zig', nav), 'zig', 'un grupo vacio sigue siendo navegable');
assert.equal(resolveGroup('python', nav), null, 'grupo que ya no existe');
assert.equal(resolveGroup(null, nav), null);
assert.deepEqual(validFocus({ id: 'npm', path: null }, 'node', CATS, WTS), { id: 'npm', path: null });
assert.equal(validFocus({ id: 'npm', path: null }, 'system', CATS, WTS), null, 'fila de otro grupo');
assert.equal(validFocus({ id: 'pip', path: null }, 'node', CATS, WTS), null, 'categoria que ya no existe');
assert.equal(validFocus({ id: 'node-projects', path: 'C:\\p\\z\\node_modules' }, 'node', CATS, WTS), null, 'carpeta ya limpiada');
assert.deepEqual(validFocus({ id: 'git-worktrees', path: 'C:\\p\\prot' }, 'git', CATS, WTS), { id: 'git-worktrees', path: 'C:\\p\\prot' });
assert.equal(validFocus(null, 'node', CATS, WTS), null);

const lv = levelTotals(CATS, [wt('libre', 4)]);
assert.deepEqual(lv, { auto: 12 * GB, redownload: 9 * GB, rebuild: 3 * GB, careful: 5 * GB });
assert.equal(Object.values(lv).reduce((a, b) => a + b, 0), groupTotals(CATS, [wt('libre', 4)]).reduce((a, g) => a + g.bytes, 0), 'por nivel suma lo mismo que por grupo');
assert.deepEqual(levelPick('auto', CATS), ['temp-user']);
assert.deepEqual(levelPick('redownload', CATS), ['npm'], 'sin las de nivel auto, sin las vacias');
assert.equal(levelHasItems('rebuild', CATS), true);
assert.equal(levelHasItems('auto', CATS), false);
assert.equal(levelGroup('rebuild', CATS, WTS), 'node');
assert.equal(levelGroup('careful', CATS, WTS), 'node', 'yarn pide atencion y node va primero');
assert.equal(levelGroup('careful', [mk('git-worktrees', 'git', 'worktrees', 0)], [wt('libre', 4)]), 'git');
assert.equal(levelGroup('rebuild', [], []), null);

assert.equal(keyAction({ key: 'ArrowLeft', target: 'row' }), 'to-nav');
assert.equal(keyAction({ key: 'ArrowRight', target: 'nav' }), 'to-list');
assert.equal(keyAction({ key: 'ArrowDown', target: 'nav' }), 'next');
assert.equal(keyAction({ key: 'ArrowUp', target: 'nav' }), 'prev');
assert.equal(keyAction({ key: 'Escape', target: 'row' }), 'close');
assert.equal(keyAction({ key: 'Escape', target: 'other' }), 'close');
assert.equal(keyAction({ key: 'Escape', target: 'input' }), null);
assert.equal(keyAction({ key: ' ', target: 'nav' }), null, 'espacio y Enter en la navegacion son del propio boton');
assert.equal(keyAction({ key: 'Enter', target: 'nav' }), null);

// Puerta unica de las acciones que borran (texto de las fuentes).
const read = (f) => readFileSync(new URL(f, srcDir), 'utf8');
const guard = read('lib/guard.svelte.ts');
assert.ok(guard.indexOf('ensureTerms(') > -1 && guard.indexOf('ensureTerms(') < guard.indexOf('confirm.req ='), 'el aviso de uso va antes de la confirmacion');
// Lo mismo dentro de cada funcion (hasta el siguiente export), no solo en la primera aparicion del archivo.
for (const fn of ['guarded', 'cleanSelection']) {
  const start = guard.indexOf(`export async function ${fn}`);
  const end = guard.indexOf('export', start + 1);
  const body = guard.slice(start, end < 0 ? undefined : end);
  assert.ok(start > -1 && body.includes('ensureTerms(') && body.indexOf('ensureTerms(') < body.indexOf('confirm.req ='), `${fn}: el aviso de uso va antes de la confirmacion`);
}
// Sin comentarios: un comentario que nombra una funcion no cuenta como llamada. (El "//" de una URL va tras ":".)
const code = (f) => read(f).replace(/\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->/g, '').replace(/(^|[^:])\/\/.*$/gm, '$1');
const callers = (re) => srcFiles.filter((f) => re.test(code(f))).sort();
assert.deepEqual(callers(/@tauri-apps\/api\/core/), ['lib/api.ts'], 'solo api.ts llega a invoke');
assert.deepEqual(callers(/\brunPlan\(/), ['lib/guard.svelte.ts', 'lib/selection.svelte.ts'], 'runPlan solo se llama desde la puerta');
assert.deepEqual(callers(/\bapi\.(cleanCategory|removeWorktrees)\b/), ['lib/selection.svelte.ts'], 'solo runPlan borra categorias y worktrees');
// Tambien por nombre, sin "api.": un import { cleanCategory } from './api' saltaria la regla anterior.
assert.deepEqual(callers(/\b(cleanCategory|removeWorktrees)\b/), ['lib/api.ts', 'lib/selection.svelte.ts'], 'categorias y worktrees: solo api.ts los define y solo runPlan los usa');
assert.deepEqual(callers(/\bensureTerms\(/), ['lib/guard.svelte.ts', 'lib/state.svelte.ts'], 'toda accion que borra pasa por guarded');
const otherDeletes = callers(/\b(trashPaths|containerPrune|containerCompact)\b/).filter((f) => f !== 'lib/api.ts');
assert.ok(otherDeletes.length > 0);
for (const f of otherDeletes) assert.match(code(f), /\bguarded\(/, `${f} borra sin pasar por guarded`);
assert.deepEqual(callers(/\bcleanSelection\b/), ['lib/ActionBar.svelte', 'lib/guard.svelte.ts'], 'limpiar la seleccion: solo el boton de la barra');
assert.deepEqual(callers(/\bresolveConfirm\b/), ['App.svelte', 'lib/guard.svelte.ts'], 'solo el dialogo de App.svelte responde a la confirmacion');

// guarded (guard.svelte.ts): aviso de uso, confirmacion y accion, en ese orden y solo sin bloqueo. Se carga como
// selection.svelte.ts, con su estado, la API, i18n y la seleccion sustituidos; $state es un objeto plano.
const G_STUBS = {
  './api': "export const errCode = (e) => (typeof e === 'string' ? e : 'command_failed');",
  './state.svelte': 'const g = globalThis.__guard; export const app = g.app; export const work = g.work; export const dataRev = g.dataRev; export const ensureTerms = () => g.ensureTerms(); export const locked = () => g.work.busy || g.app.scan.status === "running";',
  './i18n': 'export const t = (k) => k;',
  './selection.svelte': 'export const currentPlan = () => []; export const runPlan = async () => {};',
};
registerHooks({
  resolve(spec, ctx, next) {
    if (!ctx.parentURL?.endsWith('/guard.svelte.ts')) return next(spec, ctx);
    if (spec in G_STUBS) return { url: 'data:text/javascript,' + encodeURIComponent(G_STUBS[spec]), shortCircuit: true };
    if (/^\.\/(format|recovery|impact)$/.test(spec)) return { url: new URL(`../src/lib/${spec.slice(2)}.ts`, import.meta.url).href, shortCircuit: true };
    return next(spec, ctx);
  },
});
globalThis.$state = (v) => v;
const gs = (globalThis.__guard = { app: { lang: 'es', scan: { status: 'done' } }, work: { busy: false, outcome: null, wt: { list: null } }, dataRev: { n: 0 }, ensureTerms: null, termsAsked: 0 });
const guardMod = await import('../src/lib/guard.svelte.ts');
const gReq = { title: 'x', before: [], after: [], confirmLabel: 'x' };
const terms = (ok) => (gs.ensureTerms = () => (gs.termsAsked++, Promise.resolve(ok)));
let ran = 0;
let busyInside = null;
const act = (fail = false) => async () => {
  ran++;
  busyInside = gs.work.busy;
  if (fail) throw 'blocked';
  return { kind: 'prune', engine: 'docker', output: '' };
};
// Aviso de uso cancelado: ni confirmacion ni accion.
terms(false);
await guardMod.guarded(gReq, act());
assert.equal(guardMod.confirm.req, null, 'aviso cancelado: no se abre la confirmacion');
await guardMod.resolveConfirm(true);
assert.equal(ran, 0, 'aviso cancelado: no se ejecuta');
// Confirmacion cancelada: no se ejecuta.
terms(true);
await guardMod.guarded(gReq, act());
assert.equal(guardMod.confirm.req, gReq, 'aviso aceptado: se abre la confirmacion');
await guardMod.resolveConfirm(false);
assert.equal(ran, 0, 'confirmacion cancelada: no se ejecuta');
assert.equal(guardMod.confirm.req, null);
await guardMod.resolveConfirm(true);
assert.equal(ran, 0, 'una respuesta tardia no ejecuta lo cancelado');
// Confirmacion aceptada: una sola vez, ocupado mientras corre y libre despues, tambien si falla.
for (const fail of [false, true]) {
  ran = 0;
  await guardMod.guarded(gReq, act(fail));
  await guardMod.resolveConfirm(true);
  await guardMod.resolveConfirm(true);
  assert.equal(ran, 1, `aceptada${fail ? ' (falla)' : ''}: se ejecuta una vez`);
  assert.equal(busyInside, true, 'ocupado mientras corre');
  assert.equal(gs.work.busy, false, `aceptada${fail ? ' (falla)' : ''}: busy vuelve a falso`);
  assert.equal(gs.work.outcome.kind, fail ? 'error' : 'prune');
}
// Bloqueado (limpiando o escaneando): ni aviso, ni confirmacion, ni accion.
for (const lock of [() => (gs.work.busy = true), () => (gs.app.scan.status = 'running')]) {
  ran = 0;
  gs.termsAsked = 0;
  lock();
  await guardMod.guarded(gReq, act());
  await guardMod.resolveConfirm(true);
  assert.deepEqual([gs.termsAsked, guardMod.confirm.req, ran], [0, null, 0], 'bloqueado: no abre el aviso ni la confirmacion ni ejecuta');
  gs.work.busy = false;
  gs.app.scan.status = 'done';
}

// La limpieza de restos del actualizador no puede borrar de forma recursiva.
assert.ok(!readFileSync(new URL('../src-tauri/src/updates.rs', import.meta.url), 'utf8').includes('remove_dir_all'), 'updates.rs sin remove_dir_all');

// Actualizacion: estados de update.ts.
const U = await import('../src/lib/update.ts');
const base = { phase: 'idle', info: null, error: null, dismissed: false, portable: false, done: 0, total: null };
const info = { version: '0.2.0', current: '0.1.0', date: '2026-12-01', notes: 'Notas' };
assert.equal(U.afterCheck(base, info, false).phase, 'available');
assert.equal(U.afterCheck(base, null, false).phase, 'idle', 'automatica sin novedad: no dice nada');
assert.equal(U.afterCheck(base, null, true).phase, 'current', 'manual sin novedad: estas al dia');
assert.equal(U.afterCheckError(base, 'command_failed', false).phase, 'idle', 'automatica con error: silencio');
const err = U.afterCheckError(base, 'command_failed', true);
assert.deepEqual([err.phase, err.error], ['error', 'command_failed']);
const av = U.afterCheck(base, info, false);
assert.equal(U.showNotice(av), true);
assert.equal(U.showNotice({ ...av, dismissed: true }), false, 'Mas tarde oculta el aviso');
assert.equal(U.mainAction(av), 'install');
assert.equal(U.mainAction({ ...av, portable: true }), 'open-page');
assert.equal(U.canInstall(av, false), true);
assert.equal(U.canInstall(av, true), false, 'con escaneo o limpieza en curso no se actualiza');
assert.equal(U.canInstall({ ...av, portable: true }, false), false);
assert.equal(U.autoCheckEnabled(null), true);
assert.equal(U.autoCheckEnabled('0'), false);
assert.equal(U.autoCheckEnabled('1'), true);
// Los fallos del complemento llegan como texto en ingles: api.ts los cambia por un codigo traducible.
assert.deepEqual([U.updateErrCode('check'), U.updateErrCode('install')], ['update_check_failed', 'update_install_failed']);
for (const k of ['check', 'install']) assert.ok(`err.${U.updateErrCode(k)}` in es && `err.${U.updateErrCode(k)}` in en, `err.${U.updateErrCode(k)} en los dos idiomas`);
assert.match(readFileSync(new URL('../src/lib/state.svelte.ts', import.meta.url), 'utf8'), /locked = \(\) =>[^\n]*work\.updating/, 'locked incluye work.updating');
for (const mod of ['@tauri-apps/plugin-updater', '@tauri-apps/plugin-process'])
  assert.deepEqual(srcFiles.filter((f) => readFileSync(new URL(f, srcDir), 'utf8').includes(mod)), ['lib/api.ts'], `${mod} solo se importa en api.ts`);
// Aviso, dialogo y Acerca de: claves que se eligen con un ternario.
const UPDATE_KEYS = [
  'about.updates', 'update.notice', 'update.have', 'update.published', 'update.notes', 'update.install', 'update.openPage', 'update.later',
  'update.portable', 'update.blockedScan', 'update.blockedClean', 'update.downloading', 'update.downloadedBytes', 'update.restartNote',
  'update.retry', 'update.check', 'update.checking', 'update.upToDate', 'update.auto', 'update.privacy',
];
for (const [name, d] of [['es', es], ['en', en]]) assert.deepEqual(UPDATE_KEYS.filter((k) => !(k in d)), [], `faltan claves de la actualizacion en ${name}.json`);
// La lista "no hace" ya no dice que no hay ninguna conexion: nombra la consulta de version.
assert.match(es['about.doesNot.1'], /GitHub/);
assert.match(en['about.doesNot.1'], /GitHub/);
// Privacidad (Ruling A-6): consulta y descarga de la actualizacion, y GitHub ve la IP; nada de "unica conexion".
assert.match(es['update.privacy'], /dirección IP/);
assert.match(en['update.privacy'], /IP address/);
for (const d of [es, en]) for (const k of ['about.doesNot.1', 'update.privacy']) assert.doesNotMatch(d[k], /única conexión|only connection/i, k);

// Instalador: un solo NSIS, firma del actualizador y los archivos que cita la configuracion existen.
const conf = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
assert.deepEqual(conf.bundle.targets, ['nsis']);
assert.equal(conf.bundle.createUpdaterArtifacts, true);
assert.equal(conf.plugins.updater.endpoints[0], 'https://github.com/MscottB/Limpyrid/releases/latest/download/latest.json');
const nsis = conf.bundle.windows.nsis;
for (const f of [conf.bundle.licenseFile, nsis.installerIcon, nsis.uninstallerIcon, nsis.headerImage, nsis.sidebarImage])
  assert.ok(existsSync(new URL(`../src-tauri/${f}`, import.meta.url)), `${f} existe`);
assert.deepEqual(nsis.languages, ['Spanish', 'English']);
assert.equal(nsis.displayLanguageSelector, true);
assert.match(readFileSync(new URL(`../src-tauri/${nsis.installerHooks}`, import.meta.url), 'utf8'), /^!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED\s*$/m, 'casilla del escritorio desmarcada');

// Empaquetado: partes puras del guion (versiones, repositorio, latest.json, sumas).
const P = await import('./empaquetar-lib.mjs');
assert.equal(P.checkVersions({ tauri: '0.1.0', cargo: '0.1.0', npm: '0.1.0' }), '0.1.0');
assert.equal(P.checkVersions({ tauri: '0.1.0', cargo: '0.1.0', npm: '0.1.0', tag: 'v0.1.0' }), '0.1.0');
assert.throws(() => P.checkVersions({ tauri: '0.1.0', cargo: '0.1.1', npm: '0.1.0' }), /Cargo\.toml/);
assert.throws(() => P.checkVersions({ tauri: '0.1.0', cargo: '0.1.0', npm: '0.1.0', tag: 'v0.2.0' }), /etiqueta/);
assert.throws(() => P.checkVersions({ tauri: '0.1.0', cargo: '0.1.0', npm: '0.1.0', tag: '0.1.0' }), /etiqueta/);
assert.equal(P.checkRepo({ pkgRepository: 'github:MscottB/Limpyrid', endpoint: 'https://github.com/MscottB/Limpyrid/releases/latest/download/latest.json' }), 'MscottB/Limpyrid');
assert.throws(() => P.checkRepo({ pkgRepository: 'github:MscottB/Otro', endpoint: 'https://github.com/MscottB/Limpyrid/releases/latest/download/latest.json' }));
const lj = P.latestJson({ version: '0.2.0', notes: 'Notas', date: '2026-12-01T00:00:00Z', signature: 'FIRMA', repo: 'MscottB/Limpyrid' });
assert.deepEqual(lj, { version: '0.2.0', notes: 'Notas', pub_date: '2026-12-01T00:00:00Z', platforms: { 'windows-x86_64': { signature: 'FIRMA', url: 'https://github.com/MscottB/Limpyrid/releases/download/v0.2.0/Limpyrid-setup.exe' } } });
assert.equal(P.sumsFile([{ name: 'b.zip', sha256: 'bb' }, { name: 'a.exe', sha256: 'aa' }]), 'aa  a.exe\nbb  b.zip\n');

// Descargas del sitio: salen de `gh release view --json tagName,assets` y llevan los nombres sin version.
const DS = await import('./descargas-sitio.mjs');
const BASE = 'https://github.com/MscottB/Limpyrid/releases/latest/download';
const assets = ['Limpyrid-setup.exe', 'Limpyrid-setup.exe.sig', 'Limpyrid-portable.zip', 'latest.json', 'SHA256SUMS.txt'].map((name) => ({ name, digest: `sha256:${name.length}` }));
const dd = DS.descargas({ tagName: 'v0.2.0', assets });
assert.equal(dd.version, '0.2.0');
assert.deepEqual(dd.archivos['Limpyrid-setup.exe'], { url: `${BASE}/Limpyrid-setup.exe`, sha256: '18' });
assert.equal(dd.archivos['Limpyrid-portable.zip'].url, `${BASE}/Limpyrid-portable.zip`);
assert.equal(dd.archivos['Limpyrid-portable.zip'].sha256, '21');
assert.equal(dd.archivos['latest.json'], undefined, 'solo se enlazan el instalador y el portable');
assert.deepEqual(DS.descargas(null), {});
assert.deepEqual(DS.descargas({}), {});
assert.deepEqual(DS.descargas({ tagName: 'v0.2.0', assets: [] }), {});
assert.deepEqual(DS.descargas({ tagName: 'v0.2.0', assets: assets.slice(0, 1) }), {}, 'falta el portable');
assert.deepEqual(DS.descargas({ tagName: 'nightly', assets }), {}, 'etiqueta que no es una version');
assert.equal(DS.descargas({ tagName: 'v0.2.0', assets: assets.map(({ name }) => ({ name })) }).archivos['Limpyrid-setup.exe'].sha256, undefined);
const pagina = '<p>$&</p><script type="application/json" id="descargas">{}</script>';
assert.equal(DS.rellenar(pagina, {}), pagina, 'sin datos la pagina no cambia');
const rellena = DS.rellenar(pagina, { version: '0.2.0', archivos: { x: { url: '</script>$&' } } });
assert.ok(rellena.startsWith('<p>$&</p><script type="application/json" id="descargas">{"version":"0.2.0"'));
assert.equal(rellena.match(/<\/script>/g).length, 1, 'el JSON no cierra el script');
assert.equal(DS.rellenar('<p>sin bloque</p>', dd), '<p>sin bloque</p>');
for (const page of ['index.html', 'descargar.html']) {
  const html = readFileSync(new URL(`../site/${page}`, import.meta.url), 'utf8');
  assert.match(html, /<script type="application\/json" id="descargas">\{\}<\/script>/, `${page}: bloque #descargas vacio`);
  assert.doesNotMatch(html, /windows-x64/, `${page}: nombres con version`);
}
assert.match(readFileSync(new URL('../site/descargar.html', import.meta.url), 'utf8'), /certutil -hashfile Limpyrid-setup\.exe SHA256/);

// Flujos de GitHub: la clave solo como secreto, y las pruebas en ci.yml y publicar.yml.
const wf = (n) => readFileSync(new URL(`../.github/workflows/${n}`, import.meta.url), 'utf8');
const publicar = wf('publicar.yml');
assert.deepEqual([...publicar.matchAll(/secrets\.(\w+)/g)].map((m) => m[1]).sort(), ['TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD']);
assert.doesNotMatch(publicar, /BEGIN|untrusted comment|dW50cnVzdGVk/, 'publicar.yml no lleva ninguna clave');
for (const l of publicar.split('\n').filter((x) => x.includes('${{'))) assert.match(l, /^\s+(TAG|GH_TOKEN|TAURI_\w+): \$\{\{/, `solo en env: ${l.trim()}`);
for (const n of ['ci.yml', 'publicar.yml']) for (const cmd of ['npm test', 'npm run check', 'cargo test']) assert.ok(wf(n).includes(cmd), `${n}: ${cmd}`);
assert.match(wf('paginas.yml'), /descargas-sitio\.mjs/);

console.log(`check-frontend: ${Object.keys(es).length} claves, todo en orden`);
