// Que conlleva limpiar cada categoria: un nivel (el texto esta en cat.<id>.effect). Puro: check-frontend.mjs lo carga.
export type Impact = 'auto' | 'redownload' | 'rebuild' | 'careful';

// auto: vuelve solo; redownload: la herramienta lo baja otra vez; rebuild: hay que recompilar o reinstalar;
// careful: hace falta una accion tuya o no se puede asegurar como vuelve.
export const IMPACT: Record<string, Impact> = {
  'temp-user': 'auto',
  'temp-windows': 'auto',
  npm: 'redownload',
  pnpm: 'redownload',
  yarn: 'careful', // los proyectos Plug'n'Play leen los paquetes de esta cache
  bun: 'redownload',
  deno: 'redownload',
  playwright: 'careful',
  'node-projects': 'rebuild',
  pip: 'redownload',
  uv: 'redownload',
  poetry: 'redownload',
  'python-projects': 'rebuild',
  'go-build': 'auto',
  'go-mod': 'redownload',
  cargo: 'redownload',
  'rust-target': 'rebuild',
  gradle: 'redownload',
  maven: 'careful', // lo instalado con mvn install no esta en ningun servidor
  nuget: 'redownload',
  composer: 'redownload',
  'dart-pub': 'redownload',
  cpp: 'careful', // los paquetes de conan create no estan en ningun servidor
  zig: 'redownload',
  'terraform-projects': 'careful', // terraform init y el workspace elegido
  'git-worktrees': 'careful',
  'web-testing': 'careful',
  'native-build': 'redownload',
  vscode: 'auto',
  jetbrains: 'auto',
  huggingface: 'redownload',
  minikube: 'redownload',
  'antigravity-cache': 'auto',
  'claude-sessions': 'careful', // lo que quedo en esas carpetas no vuelve
  docker: 'careful',
  podman: 'careful',
};

export const impactOf = (id: string): Impact => IMPACT[id] ?? 'careful';

// Primera frase de un texto de efecto (para la confirmacion). Los textos no usan abreviaturas con punto.
export const firstSentence = (s: string) => s.match(/^.*?\.(?=\s|$)/)?.[0] ?? s;
// Las frases por separado (el detalle muestra cada una en su linea: el texto de Docker son tres acciones).
export const sentences = (s: string) => s.split(/(?<=\.)\s+(?=[\p{Lu}«"])/u);
