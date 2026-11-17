// Partes puras del guion de empaquetado: sin disco ni red, para poder probarlas.

export function checkVersions({ tauri, cargo, npm, tag }) {
  if (cargo !== tauri) throw new Error(`La version de Cargo.toml (${cargo}) no coincide con tauri.conf.json (${tauri})`);
  if (npm !== tauri) throw new Error(`La version de package.json (${npm}) no coincide con tauri.conf.json (${tauri})`);
  if (tag !== undefined && tag !== `v${tauri}`) throw new Error(`La etiqueta ${tag} no coincide con la version ${tauri} (se esperaba v${tauri})`);
  return tauri;
}

export function checkRepo({ pkgRepository, endpoint }) {
  const a = /^github:([^/\s]+\/[^/\s]+)$/.exec(pkgRepository ?? '')?.[1];
  const b = /^https:\/\/github\.com\/([^/]+\/[^/]+)\/releases\//.exec(endpoint ?? '')?.[1];
  if (!a) throw new Error(`El campo repository de package.json debe ser github:dueno/nombre (es ${pkgRepository})`);
  if (!b) throw new Error(`El punto de consulta de tauri.conf.json no es una URL de releases de GitHub (es ${endpoint})`);
  if (a !== b) throw new Error(`El repositorio de package.json (${a}) no coincide con el de tauri.conf.json (${b})`);
  return a;
}

export function latestJson({ version, notes, date, signature, repo }) {
  return {
    version,
    notes,
    pub_date: date,
    platforms: { 'windows-x86_64': { signature, url: `https://github.com/${repo}/releases/download/v${version}/Limpyrid-setup.exe` } },
  };
}

// Id de clave de una firma o clave publica de Tauri (base64 de un texto minisign): bytes 2 a 9 de la segunda linea
// decodificada, al reves, como lo muestra minisign. `what` nombra el dato en el error.
export function keyId(text, what = 'la clave') {
  const line = Buffer.from(text.trim(), 'base64').toString('utf8').split('\n')[1]?.trim() ?? '';
  const raw = Buffer.from(line, 'base64');
  if (raw.length < 10) throw new Error(`No se pudo leer el identificador de ${what}`);
  return [...raw.subarray(2, 10)].reverse().map((b) => b.toString(16).padStart(2, '0')).join('').toUpperCase();
}

export function checkKey({ signature, pubkey }) {
  const a = keyId(signature, 'la firma');
  const b = keyId(pubkey, 'la clave publica de tauri.conf.json');
  if (a !== b) throw new Error(`La firma es de la clave ${a}, pero tauri.conf.json espera ${b}: el instalador se firmo con otra clave`);
  return a;
}

export function sumsFile(entries) {
  return [...entries].sort((x, y) => (x.name < y.name ? -1 : x.name > y.name ? 1 : 0)).map((e) => `${e.sha256}  ${e.name}\n`).join('');
}
