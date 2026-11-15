// Reglas de la actualizacion, sin estado ni llamadas: las usan update.svelte.ts y las pruebas de Node.
import type { UpdateInfo } from './api';

export type Phase = 'idle' | 'checking' | 'current' | 'available' | 'downloading' | 'error';
export interface UpdateState { phase: Phase; info: UpdateInfo | null; error: string | null; dismissed: boolean; portable: boolean; done: number; total: number | null }

export const showNotice = (s: UpdateState) => s.phase === 'available' && !s.dismissed;
// La portable no se reemplaza sola: lleva a la pagina de descarga.
export const mainAction = (s: UpdateState): 'install' | 'open-page' => (s.portable ? 'open-page' : 'install');
export const canInstall = (s: UpdateState, locked: boolean) => s.phase === 'available' && !s.portable && !locked;

// La comprobacion automatica no dice nada si no hay novedad o falla; la manual siempre da resultado.
export const afterCheck = (s: UpdateState, result: UpdateInfo | null, manual: boolean): UpdateState =>
  result ? { ...s, phase: 'available', info: result, error: null } : { ...s, phase: manual ? 'current' : 'idle', info: null, error: null };
export const afterCheckError = (s: UpdateState, code: string, manual: boolean): UpdateState =>
  manual ? { ...s, phase: 'error', error: code } : { ...s, phase: 'idle', error: null };

export const autoCheckEnabled = (stored: string | null) => stored !== '0';

// El complemento rechaza con texto en ingles, no con un codigo: api.ts lo cambia por uno de estos (err.* en los JSON).
export const updateErrCode = (kind: 'check' | 'install') => (kind === 'check' ? 'update_check_failed' : 'update_install_failed');
