// Empaqueta Limpyrid para publicar. Uso: node scripts/empaquetar.mjs [--tag vX.Y.Z] [--notas archivo]
import { readFileSync, writeFileSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkVersions, checkRepo, latestJson, sumsFile } from './empaquetar-lib.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
const at = (...p) => join(root, ...p);
const fail = (m) => { console.error(`empaquetar: ${m}`); process.exit(1); };

const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(name); return i < 0 ? undefined : (args[i + 1] ?? fail(`${name} necesita un valor`)); };
const tag = opt('--tag');
const notesFile = opt('--notas');

// 1. Versiones y repositorio.
const conf = JSON.parse(readFileSync(at('src-tauri', 'tauri.conf.json'), 'utf8'));
const pkg = JSON.parse(readFileSync(at('package.json'), 'utf8'));
const cargo = /^version\s*=\s*"([^"]+)"/m.exec(readFileSync(at('src-tauri', 'Cargo.toml'), 'utf8'))?.[1];
let version, repo;
try {
  version = checkVersions({ tauri: conf.version, cargo, npm: pkg.version, tag });
  repo = checkRepo({ pkgRepository: pkg.repository, endpoint: conf.plugins?.updater?.endpoints?.[0] });
} catch (e) { fail(e.message); }
if (!process.env.TAURI_SIGNING_PRIVATE_KEY) fail('Falta TAURI_SIGNING_PRIVATE_KEY: pon en el entorno la ruta de la clave de firma (o su contenido) antes de empaquetar.');
const notes = notesFile ? readFileSync(notesFile, 'utf8').trim() : `Limpyrid ${version}`;

// 2. Compilar.
const build = spawnSync('npm run tauri build', { cwd: root, stdio: 'inherit', shell: true });
if (build.status !== 0) fail('la compilacion fallo');

// 3. Instalador y firma, por su nombre exacto con la version.
const setup = at('src-tauri', 'target', 'release', 'bundle', 'nsis', `Limpyrid_${version}_x64-setup.exe`);
const exe = at('src-tauri', 'target', 'release', 'limpyrid.exe');
for (const f of [setup, `${setup}.sig`, exe]) if (!existsSync(f)) fail(`la compilacion no dejo ${f}`);

const tmp = mkdtempSync(join(tmpdir(), 'limpyrid-empaquetar-'));
try {
  const out = join(tmp, 'out'), stage = join(tmp, 'portable');
  mkdirSync(out); mkdirSync(stage);
  copyFileSync(setup, join(out, 'Limpyrid-setup.exe'));
  copyFileSync(`${setup}.sig`, join(out, 'Limpyrid-setup.exe.sig'));

  // 4. Portable.
  copyFileSync(exe, join(stage, 'Limpyrid.exe'));
  copyFileSync(at('scripts', 'LEEME-portable.txt'), join(stage, 'LEEME.txt'));
  copyFileSync(at('LICENSE'), join(stage, 'LICENSE'));
  copyFileSync(at('TERMINOS.md'), join(stage, 'TERMINOS.md'));
  writeFileSync(join(stage, 'limpyrid.portable'), '');
  execFileSync('powershell', ['-NoProfile', '-Command', 'Compress-Archive -Path (Join-Path $env:LP_STAGE "*") -DestinationPath $env:LP_ZIP'],
    { stdio: 'inherit', env: { ...process.env, LP_STAGE: stage, LP_ZIP: join(out, 'Limpyrid-portable.zip') } });

  // 5. latest.json con la firma de ESTA compilacion, tal cual.
  const signature = readFileSync(join(out, 'Limpyrid-setup.exe.sig'), 'utf8');
  writeFileSync(join(out, 'latest.json'), JSON.stringify(latestJson({ version, notes, date: new Date().toISOString(), signature, repo }), null, 2) + '\n');

  // 6. Sumas de los tres archivos que descarga la gente.
  const sha = (n) => ({ name: n, sha256: createHash('sha256').update(readFileSync(join(out, n))).digest('hex') });
  writeFileSync(join(out, 'SHA256SUMS.txt'), sumsFile(['Limpyrid-setup.exe', 'Limpyrid-setup.exe.sig', 'Limpyrid-portable.zip'].map(sha)));

  // 7. Solo ahora, con todo listo, se toca dist-publicar (se sobrescriben los cinco nombres).
  const dest = at('dist-publicar');
  mkdirSync(dest, { recursive: true });
  for (const n of ['Limpyrid-setup.exe', 'Limpyrid-setup.exe.sig', 'Limpyrid-portable.zip', 'latest.json', 'SHA256SUMS.txt']) copyFileSync(join(out, n), join(dest, n));
  console.log(`empaquetar: version ${version} lista en ${dest}`);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
