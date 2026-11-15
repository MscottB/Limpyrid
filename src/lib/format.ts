// Formato de tamanos y fechas. No importa estado de Svelte: el script de comprobacion lo carga desde Node.
export type Lang = string;

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
const DAY_MS = 86_400_000;

export function formatBytes(bytes: number, lang: Lang): string {
  let i = 0;
  let v = bytes;
  // Se compara el valor ya redondeado: 1048575 B es "1,0 MB", no "1024,0 KB".
  while (Math.round(v * 10) / 10 >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  if (i === 0) return `${Math.round(v)} B`;
  const n = new Intl.NumberFormat(lang, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(v);
  return `${n} ${UNITS[i]}`;
}

// `today`: la etiqueta ya traducida (t('age.today')); este modulo no importa estado de Svelte.
export function formatAge(unixSeconds: number, nowMs: number, lang: Lang, today: string): string {
  const days = Math.floor((nowMs - unixSeconds * 1000) / DAY_MS);
  if (days < 1) return today;
  return new Intl.RelativeTimeFormat(lang, { numeric: 'always' }).format(-days, 'day');
}

// Parte de un total como porcentaje entero; lo que no llega al 1 % se dice "<1 %" en vez de "0 %".
export function formatShare(part: number, total: number, lang: Lang): string {
  const f = new Intl.NumberFormat(lang, { style: 'percent', maximumFractionDigits: 0 });
  if (part <= 0 || total <= 0) return f.format(0);
  return part / total < 0.005 ? '<' + f.format(0.01) : f.format(part / total);
}
