// Estado de la actualizacion y sus llamadas; las reglas estan en update.ts.
import * as api from './api';
import { locked, work } from './state.svelte';
import { afterCheck, afterCheckError, autoCheckEnabled, canInstall, type UpdateState } from './update';

const KEY = 'limpyrid.updateCheck';
function stored(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null; // sin almacenamiento: se comprueba al abrir, el valor por defecto
  }
}

// `open`: el dialogo de actualizacion esta abierto.
export const update = $state<UpdateState & { open: boolean }>({ phase: 'idle', info: null, error: null, dismissed: false, portable: false, done: 0, total: null, open: false });
// "Buscar actualizaciones al abrir".
export const autoCheck = $state({ on: autoCheckEnabled(stored()) });

export function setAutoCheck(on: boolean) {
  autoCheck.on = on;
  try {
    localStorage.setItem(KEY, on ? '1' : '0');
  } catch {
    /* no persistir no es grave */
  }
}

// Nunca rechaza: todo fallo pasa por afterCheckError (la automatica se queda callada).
export async function checkNow(manual: boolean): Promise<void> {
  if (update.phase === 'checking' || update.phase === 'downloading') return;
  update.phase = 'checking';
  try {
    const [info, portable] = await Promise.all([api.checkUpdate(), api.isPortable()]);
    update.portable = portable;
    Object.assign(update, afterCheck(update, info, manual));
  } catch (e) {
    Object.assign(update, afterCheckError(update, api.errCode(e), manual));
  }
}

// En la app real, si todo va bien, la app se cierra y vuelve a abrirse en la version nueva; en demostracion termina sin mas.
export async function install(): Promise<void> {
  if (!canInstall(update, locked())) return;
  work.updating = true;
  Object.assign(update, { phase: 'downloading', error: null, done: 0, total: null });
  try {
    await api.installUpdate((done, total) => Object.assign(update, { done, total }));
    update.phase = 'available';
  } catch (e) {
    Object.assign(update, { phase: 'error', error: api.errCode(e) });
  } finally {
    work.updating = false;
  }
}

export const openDialog = () => void (update.open = true);
export const closeDialog = () => void (update.open = false);
// "Mas tarde": oculta el aviso hasta la proxima apertura de la app.
export function dismiss() {
  update.dismissed = true;
  update.open = false;
}
