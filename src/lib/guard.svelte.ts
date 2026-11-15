// Puerta unica de las acciones que borran: aviso de uso, confirmacion, ocupado, accion, informe y recarga.
// App.svelte monta el unico ConfirmDialog, que lee `confirm.req`.
import * as api from './api';
import { app, dataRev, ensureTerms, locked, work } from './state.svelte';
import { t } from './i18n';
import { formatBytes } from './format';
import { currentPlan, runPlan } from './selection.svelte';
import { wtStale, type PlanEntry } from './recovery';
import { firstSentence, impactOf } from './impact';
import type { ConfirmGroup, ConfirmRequest } from './ConfirmDialog.svelte';
import type { Outcome } from './ReportPanel.svelte';

export const confirm = $state({ req: null as ConfirmRequest | null });
let pending: (() => Promise<void>) | null = null;

const fmt = (n: number) => formatBytes(n, app.lang);
const days = (hours: number) => t(hours === 24 ? 'clean.days.one' : 'clean.days.other', { n: hours / 24 });

// Worktrees clasificados con otra antiguedad que la elegida: no se ofrece limpiar hasta que la lista se recargue.
export const staleWorktrees = () => wtStale(currentPlan(), work.wt, work.minAge);
export const canClean = () => currentPlan().length > 0 && !locked() && !(work.catsLoading || work.wt.loading) && !staleWorktrees();

export function itemLabel(e: PlanEntry, path: string): string {
  if (e.kind === 'worktrees') {
    const w = work.wt.list?.worktrees.find((x) => x.path === path);
    return w ? t('wt.itemLabel', { repo: w.repo, name: w.name, branch: w.branch ?? t('wt.noBranch') }) : path;
  }
  return path;
}

function group(e: PlanEntry, age: number): ConfirmGroup {
  const c = work.cats?.find((x) => x.id === e.id);
  const notes: string[] = [];
  if (e.kind === 'contents' && c?.paths.length) notes.push(t('sel.contentsOf', { paths: c.paths.join('; ') }));
  if (e.kind === 'subfolders') notes.push(t('sel.subfoldersNote'));
  if (e.usesMinAge && e.kind !== 'worktrees') notes.push(t('clean.ageNote', { days: days(age) }));
  if (e.kind === 'worktrees') notes.push(t('wt.ageNote', { days: days(age) }), t('wt.confirmKeep'), t('wt.confirmSkip'));
  const level = impactOf(e.id);
  return {
    title: t('cat.' + e.id),
    size: fmt(e.size),
    tag: t('sel.permanent'),
    // careful: el texto entero, para que el aviso (y el comando que lo devuelve) se lea antes de confirmar.
    impact: { label: t('impact.' + level), text: level === 'careful' ? t('cat.' + e.id + '.effect') : firstSentence(t('cat.' + e.id + '.effect')), careful: level === 'careful' },
    notes,
    items: e.items.map((i) => ({ label: itemLabel(e, i.path), size: fmt(i.size) })),
  };
}

// Lo que se envia sale del mismo plan que muestra la confirmacion, tomado tras el aviso de uso.
export async function cleanSelection() {
  if (!canClean()) return; // la misma condicion que desactiva el boton
  if (!(await ensureTerms()) || !canClean()) return; // cancelado (o algo cambio mientras se leia): no hay confirmacion
  const entries = currentPlan();
  const age = work.minAge;
  const size = fmt(entries.reduce((a, e) => a + e.size, 0));
  confirm.req = {
    title: t(entries.length === 1 ? 'sel.confirmTitle.one' : 'sel.confirmTitle.other', { n: entries.length }),
    before: [t('sel.confirmBody', { size })],
    groups: entries.map((e) => group(e, age)),
    after: [t('confirm.permanent')],
    confirmLabel: t('confirm.cleanUpTo', { size }),
  };
  pending = () => runPlan(entries, age);
}

// Docker, Podman y la Papelera: acciones propias, con su confirmacion. `compact`: un fallo avisa que el motor y WSL
// pueden haber quedado detenidos. El aviso de uso va antes de su confirmacion.
export async function guarded(req: ConfirmRequest, action: () => Promise<Outcome>, compact = false) {
  if (locked() || !(await ensureTerms()) || locked()) return; // ocupado: ni siquiera se abre el aviso de uso
  confirm.req = req;
  pending = async () => {
    work.busy = true;
    work.outcome = null;
    try {
      work.outcome = await action();
    } catch (e) {
      work.outcome = { kind: 'error', code: api.errCode(e), compact };
    } finally {
      work.busy = false;
    }
    dataRev.n++; // las vistas recargan lo que la accion cambio
  };
}

export async function resolveConfirm(ok: boolean) {
  const run = pending;
  confirm.req = null;
  pending = null;
  if (ok && run) await run();
}
