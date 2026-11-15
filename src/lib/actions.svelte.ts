// Acciones sobre una carpeta o archivo del mapa y del explorador: mostrar en Windows y enviar a la Papelera (por la
// puerta de guard.svelte.ts: aviso de uso, confirmacion e informe en work.outcome).
// El bloqueo real de rutas esta en Rust (trash_paths); ocultar la accion en la interfaz es solo comodidad.
import * as api from './api';
import { app, work } from './state.svelte';
import { t } from './i18n';
import { formatBytes } from './format';
import { guarded } from './guard.svelte';

const leaf = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

export async function reveal(path: string) {
  try {
    await api.reveal(path);
  } catch {
    work.outcome = { kind: 'error', code: 'command_failed' };
  }
}

export function trash(path: string, size: number) {
  const name = leaf(path);
  return guarded(
    {
      title: t('act.trash'),
      before: [t('trash.body', { name, size: formatBytes(size, app.lang) })],
      items: [path],
      after: [t('trash.after')],
      confirmLabel: t('act.trash'),
    },
    async () => ({ kind: 'clean', name, report: await api.trashPaths([path]), op: 'trash' }),
  );
}
