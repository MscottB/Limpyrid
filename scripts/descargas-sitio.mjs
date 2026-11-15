// Rellena #descargas de las paginas del sitio con la ultima version publicada.
// Uso: node scripts/descargas-sitio.mjs release.json site/index.html site/descargar.html
// release.json es la salida de `gh release view --json tagName,assets`; sin red: este guion solo lee archivos.
import { readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const BASE = 'https://github.com/MscottB/Limpyrid/releases/latest/download';
const ARCHIVOS = ['Limpyrid-setup.exe', 'Limpyrid-portable.zip'];

// Vacio ({}) si no hay version o faltan los archivos: el sitio se queda como esta, con sus avisos de provisional.
export function descargas(release) {
  const version = /^v(\d+\.\d+\.\d+)$/.exec(release?.tagName ?? '')?.[1];
  const assets = Array.isArray(release?.assets) ? release.assets : [];
  if (!version || !ARCHIVOS.every((n) => assets.some((a) => a.name === n))) return {};
  const archivos = {};
  for (const n of ARCHIVOS) {
    const sha256 = /^sha256:([0-9a-f]+)$/i.exec(assets.find((a) => a.name === n).digest ?? '')?.[1];
    archivos[n] = { url: `${BASE}/${n}`, ...(sha256 && { sha256 }) };
  }
  return { version, archivos };
}

const BLOQUE = /(<script type="application\/json" id="descargas">)[^]*?(<\/script>)/;

export function rellenar(html, datos) {
  if (!datos.version) return html;
  // "<" escapado: ningun valor puede cerrar el <script>.
  const json = JSON.stringify(datos).replaceAll('<', '\\u003c');
  return html.replace(BLOQUE, (_, abre, cierra) => abre + json + cierra);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [json, ...paginas] = process.argv.slice(2);
  let release = null;
  try { release = JSON.parse(readFileSync(json, 'utf8')); } catch { /* sin version publicada */ }
  const datos = descargas(release);
  for (const p of paginas) writeFileSync(p, rellenar(readFileSync(p, 'utf8'), datos));
  console.log(datos.version ? `descargas del sitio: v${datos.version} en ${paginas.length} paginas` : 'descargas del sitio: sin version publicada, el sitio queda como esta');
}
