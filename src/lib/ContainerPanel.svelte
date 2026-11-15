<script lang="ts">
  // Docker o Podman dentro del detalle: uso segun el motor y sus dos acciones, que pasan por guarded.
  import { untrack } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as api from './api';
  import type { DiskFile, Engine, Usage } from './api';
  import { app, dataRev, locked, work } from './state.svelte';
  import { t } from './i18n';
  import { formatBytes } from './format';
  import { prunableRows } from './containers';
  import { guarded } from './guard.svelte';

  let { engine }: { engine: Engine } = $props();

  let usage = $state<Record<string, Usage | 'loading'>>({});
  let allImages = $state<Record<string, boolean>>({});
  const fmt = (n: number) => formatBytes(n, app.lang);

  // El uso cambia por fuera de la app: se consulta al abrir y tras cada cambio de datos.
  $effect(() => {
    void dataRev.n;
    const e = engine;
    untrack(() => void loadUsage(e));
  });

  async function loadUsage(engine: Engine) {
    usage[engine] = 'loading';
    try {
      usage[engine] = await api.containerUsage(engine);
    } catch (e) {
      delete usage[engine];
      work.outcome = { kind: 'error', code: api.errCode(e) };
    }
  }

  const usageOf = (e: string) => (usage[e] && usage[e] !== 'loading' ? (usage[e] as Usage) : null);

  function prune(engine: Engine) {
    const all = !!allImages[engine];
    const label = t('cat.' + engine);
    guarded(
      {
        title: t('containers.pruneTitle', { engine: label }),
        before: [t('containers.pruneBody'), ...(all ? [t('containers.pruneAllBody')] : []), t('containers.pruneAmount')],
        items: prunableRows(usageOf(engine)?.rows ?? []).map((r) => `${r.kind}: ${r.reclaimable}`),
        after: [t('confirm.permanent')],
        confirmLabel: t('containers.pruneConfirm', { engine: label }),
      },
      async () => ({ kind: 'prune', engine: label, output: await api.containerPrune(engine, all) }),
    );
  }

  function compact(engine: Engine) {
    const label = t('cat.' + engine);
    // El backend compacta solo el disco mas grande: el dialogo nombra ese y su tamano.
    const disk = (usageOf(engine)?.disks ?? []).reduce<DiskFile | null>((a, d) => (!a || d.size > a.size ? d : a), null);
    if (!disk) return;
    const size = fmt(disk.size);
    guarded(
      {
        title: t('containers.compactTitle', { engine: label }),
        before: [t('containers.compactBody', { size, path: disk.path })],
        items: [t('containers.compactStops'), t('containers.compactAdmin'), t('containers.compactTime'), t('containers.compactBackup', { engine: label })],
        after: [],
        confirmLabel: t('containers.compactConfirm', { size }),
      },
      async () => {
        const [before, after] = await api.containerCompact(engine);
        return { kind: 'compact', engine, before, after };
      },
      true,
    );
  }

  const u = $derived(usageOf(engine));
</script>

<div class="flex flex-col gap-3 text-[13px]">
  <p class="text-muted-foreground">{t('containers.apart')}</p>
  {#if !u}
    <p class="text-muted-foreground" role="status">{t('containers.loading')}</p>
  {:else if !u.installed}
    <p>{t('containers.notInstalled')}</p>
  {:else}
    {#if u.rows.length}
      <table class="w-full border-collapse text-left">
        <caption class="pb-1 text-left text-muted-foreground">{t('containers.usage')}</caption>
        <thead><tr class="text-muted-foreground"><th scope="col" class="py-1 font-medium">{t('containers.kind')}</th><th scope="col" class="num py-1 font-medium">{t('containers.total')}</th><th scope="col" class="num py-1 font-medium">{t('containers.reclaimable')}</th></tr></thead>
        <tbody>
          {#each u.rows as r}<tr class="border-t border-border"><th scope="row" class="py-1.5 font-normal">{r.kind}</th><td class="num py-1.5 pl-2">{r.total}</td><td class="num py-1.5 pl-2">{r.reclaimable}</td></tr>{/each}
        </tbody>
      </table>
    {/if}
    {#if u.disks.length}
      <div>
        <p class="text-muted-foreground">{t('containers.disks')}</p>
        <ul>{#each u.disks as d}<li class="flex justify-between gap-3"><span class="min-w-0 font-mono text-[12px] text-muted-foreground [overflow-wrap:anywhere]">{d.path}</span><span class="num">{fmt(d.size)}</span></li>{/each}</ul>
      </div>
    {/if}
    {#if !u.running}<p class="text-muted-foreground">{t('containers.stopped')}</p>{/if}
    <div class="flex flex-col items-start gap-2">
      <label class="flex min-h-8 items-center gap-2">
        <Checkbox bind:checked={() => !!allImages[engine], (on) => (allImages[engine] = on)} disabled={locked() || !u.running} />
        {t('containers.allImages')}
      </label>
      <Button variant="outline" disabled={locked() || !u.running} onclick={() => prune(engine)}>{t('containers.prune')}</Button>
      <Button variant="outline" disabled={locked() || !u.disks.length} onclick={() => compact(engine)}>{t('containers.compact')}</Button>
    </div>
  {/if}
</div>
