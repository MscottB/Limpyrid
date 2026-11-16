<script lang="ts" module>
  import type { Engine, Report } from './api';

  export type Outcome =
    | { kind: 'clean'; name: string; report: Report; op: 'delete' | 'trash' }
    | { kind: 'prune'; engine: string; output: string }
    | { kind: 'compact'; engine: Engine; before: number; after: number }
    | { kind: 'error'; code: string; compact?: boolean } // compact: el fallo pudo dejar el motor y WSL detenidos
    // Limpieza de la seleccion: un informe por categoria (ids, para que se traduzcan al cambiar de idioma). `error`: el
    // comando que fallo y detuvo el resto; `notRun`: las categorias que no llegaron a correr.
    // `op: 'trash'`: esa categoria fue a la Papelera y su `freed` es lo enviado, no espacio liberado.
    | { kind: 'batch'; results: { id: string; report: Report; op: 'delete' | 'trash' }[]; error?: { id: string; code: string }; notRun: string[] };
</script>

<script lang="ts">
  import { app } from './state.svelte';
  import { t } from './i18n';
  import { formatBytes } from './format';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { Button } from '$lib/components/ui/button';

  let { outcome, ondismiss }: { outcome: Outcome | null; ondismiss: () => void } = $props();

  const MAX_PATHS = 100;

  const byReason = (skipped: Report['skipped']) => {
    const by = new Map<string, string[]>();
    for (const s of skipped) (by.get(s.reason) ?? by.set(s.reason, []).get(s.reason)!).push(s.path);
    return [...by];
  };
  const skippedByReason = $derived(outcome?.kind === 'clean' ? byReason(outcome.report.skipped) : []);
  const fmt = (n: number) => formatBytes(n, app.lang);
  const catName = (id: string) => t('cat.' + id);
  // Con `op`, solo las categorias que se borraron (`delete`) o que fueron a la Papelera (`trash`).
  const sum = (o: Extract<Outcome, { kind: 'batch' }>, f: (r: Report) => number, op?: 'delete' | 'trash') =>
    o.results.filter((r) => !op || r.op === op).reduce((a, r) => a + f(r.report), 0);
</script>

<div class="report" aria-live="polite">
  {#if outcome?.kind === 'error'}
    <div class="flex items-center gap-3 text-destructive" role="alert">
      <TriangleAlert size={18} class="shrink-0" aria-hidden="true" />
      <p class="flex-1">{t('err.' + outcome.code)}{#if outcome.compact}<br />{t('err.compactStopped')}{/if}</p>
      <Button variant="outline" size="sm" class="text-foreground" onclick={ondismiss}>{t('report.dismiss')}</Button>
    </div>
  {:else if outcome}
    <div>
      <div class="flex items-center justify-between gap-3">
        <h3 class="flex items-center gap-2 font-semibold">
          {#if outcome.kind === 'batch' && outcome.error}
            <TriangleAlert size={18} class="shrink-0 text-warning" aria-hidden="true" />{t('batch.titleStopped')}
          {:else}
            <CircleCheck size={18} class="shrink-0 text-success" aria-hidden="true" />{t('report.title')}{outcome.kind === 'clean' ? `: ${outcome.name}` : ''}
          {/if}
        </h3>
        <Button variant="outline" size="sm" onclick={ondismiss}>{t('report.dismiss')}</Button>
      </div>
      {#if outcome.kind === 'batch'}
        {@render batch(outcome)}
      {:else if outcome.kind === 'clean'}
        {#if outcome.report.deleted === 0 && outcome.report.skipped.length === 0}
          <p class="mt-2 text-muted-foreground">{t('report.nothing')}</p>
        {:else}
          <dl class="mt-3 grid grid-cols-[max-content_max-content] gap-x-6 gap-y-1">
            <!-- La papelera no libera espacio: lo enviado sigue ocupando disco hasta vaciarla. -->
            <dt class="text-muted-foreground">{t(outcome.op === 'trash' ? 'report.trashed' : 'report.freed')}</dt><dd class="num font-semibold">{formatBytes(outcome.report.freed, app.lang)}</dd>
            <dt class="text-muted-foreground">{t('report.deleted')}</dt><dd class="num">{outcome.report.deleted}</dd>
          </dl>
        {/if}
        {#each skippedByReason as [reason, paths]}
          <details class="group mt-2">
            <summary class="flex min-h-9 cursor-pointer items-center gap-1.5 text-muted-foreground hover:text-foreground"><ChevronRight size={14} class="transition-transform group-open:rotate-90" aria-hidden="true" />{t('skip.' + reason)} <span class="num">({paths.length})</span></summary>
            {@render pathList(paths)}
          </details>
        {/each}
      {:else if outcome.kind === 'prune'}
        <p class="mt-2">{t('report.prune', { engine: outcome.engine })}</p>
        {#if outcome.output.trim()}<pre class="mt-2 max-h-[200px] overflow-auto border-y border-border py-2 text-[13px] whitespace-pre-wrap">{outcome.output.trim()}</pre>{/if}
      {:else}
        <p class="mt-2">{t('report.compact')}</p>
        <dl class="mt-3 grid grid-cols-[max-content_max-content] gap-x-6 gap-y-1">
          <dt class="text-muted-foreground">{t('report.before')}</dt><dd class="num">{formatBytes(outcome.before, app.lang)}</dd>
          <dt class="text-muted-foreground">{t('report.after')}</dt><dd class="num">{formatBytes(outcome.after, app.lang)}</dd>
          <dt class="text-muted-foreground">{t('report.freed')}</dt><dd class="num font-semibold">{formatBytes(Math.max(0, outcome.before - outcome.after), app.lang)}</dd>
        </dl>
      {/if}
    </div>
  {/if}
</div>

{#snippet pathList(paths: string[])}
  <ul class="max-h-[200px] overflow-auto pl-6 text-[13px] [overflow-wrap:anywhere]">
    {#each paths.slice(0, MAX_PATHS) as p}<li>{p}</li>{/each}
    {#if paths.length > MAX_PATHS}<li class="text-muted-foreground">… +{paths.length - MAX_PATHS}</li>{/if}
  </ul>
{/snippet}

{#snippet batch(o: Extract<Outcome, { kind: 'batch' }>)}
  <!-- Lo enviado a la Papelera va en su propia columna: no es espacio liberado. -->
  {@const binned = o.results.some((r) => r.op === 'trash')}
  {#if o.error}
    <p class="mt-2 text-destructive" role="alert">{t('batch.stopped', { name: catName(o.error.id) })} {t('err.' + o.error.code)}</p>
  {/if}
  {#if o.results.length}
    <table class="mt-3 w-full border-collapse text-left">
      <thead>
        <tr class="text-[13px] text-muted-foreground">
          <th scope="col" class="py-1.5 pr-4 font-medium">{t('batch.category')}</th>
          <th scope="col" class="num py-1.5 pr-4 font-medium">{t('report.freed')}</th>
          {#if binned}<th scope="col" class="num py-1.5 pr-4 font-medium">{t('report.trashed')}</th>{/if}
          <th scope="col" class="num py-1.5 pr-4 font-medium">{t(binned ? 'batch.items' : 'report.deleted')}</th>
          <th scope="col" class="num py-1.5 font-medium">{t('batch.skipped')}</th>
        </tr>
      </thead>
      <tbody>
        {#each o.results as r (r.id)}
          <tr class="border-t border-border">
            <th scope="row" class="py-2 pr-4 font-normal">{catName(r.id)}</th>
            <td class="num py-2 pr-4 font-semibold">{r.op === 'trash' ? '—' : fmt(r.report.freed)}</td>
            {#if binned}<td class="num py-2 pr-4 font-semibold">{r.op === 'trash' ? fmt(r.report.freed) : '—'}</td>{/if}
            <td class="num py-2 pr-4">{r.report.deleted.toLocaleString(app.lang)}</td>
            <td class="num py-2">{r.report.skipped.length.toLocaleString(app.lang)}</td>
          </tr>
        {/each}
      </tbody>
      {#if o.results.length > 1}
        <tfoot>
          <tr class="border-t border-input">
            <th scope="row" class="py-2 pr-4 font-semibold">{t('batch.total')}</th>
            <td class="num py-2 pr-4 font-semibold">{fmt(sum(o, (r) => r.freed, 'delete'))}</td>
            {#if binned}<td class="num py-2 pr-4 font-semibold">{fmt(sum(o, (r) => r.freed, 'trash'))}</td>{/if}
            <td class="num py-2 pr-4">{sum(o, (r) => r.deleted).toLocaleString(app.lang)}</td>
            <td class="num py-2">{sum(o, (r) => r.skipped.length).toLocaleString(app.lang)}</td>
          </tr>
        </tfoot>
      {/if}
    </table>
    {#if binned}
      <p class="mt-2 text-muted-foreground">
        {t('batch.trashNote')}
        {#if o.results.some((r) => r.report.skipped.some((s) => s.reason === 'too_large'))}{t('batch.tooLarge')}{/if}
      </p>
    {/if}
    {#each o.results as r (r.id)}
      {#each byReason(r.report.skipped) as [reason, paths] (reason)}
        <details class="group mt-1">
          <summary class="flex min-h-9 cursor-pointer items-center gap-1.5 text-muted-foreground hover:text-foreground">
            <ChevronRight size={14} class="transition-transform group-open:rotate-90" aria-hidden="true" />{t('batch.skippedIn', { name: catName(r.id), reason: t('skip.' + reason) })} <span class="num">({paths.length})</span>
          </summary>
          {@render pathList(paths)}
        </details>
      {/each}
    {/each}
  {/if}
  {#if o.notRun.length}
    <p class="mt-2 text-muted-foreground">{t('batch.notRun', { names: o.notRun.map(catName).join(', ') })}</p>
  {/if}
{/snippet}
