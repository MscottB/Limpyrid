<script lang="ts">
  // Que se escanea: una fila por unidad (grupo de radios nativo: flechas para moverse) y "Otra carpeta" al final.
  import type { Component } from 'svelte';
  import HardDrive from '@lucide/svelte/icons/hard-drive';
  import Usb from '@lucide/svelte/icons/usb';
  import Server from '@lucide/svelte/icons/server';
  import Disc from '@lucide/svelte/icons/disc';
  import MemoryStick from '@lucide/svelte/icons/memory-stick';
  import Monitor from '@lucide/svelte/icons/monitor';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { Button } from '$lib/components/ui/button';
  import type { Drive } from './api';
  import { app, loadDrives, work } from './state.svelte';
  import { noReason, selectable } from './recovery';
  import { t } from './i18n';
  import { formatBytes } from './format';

  let { disabled }: { disabled: boolean } = $props();

  const fmt = (n: number) => formatBytes(n, app.lang);
  const drives = $derived(work.drives ?? []);

  function kindText(d: Drive): string {
    const kind = t(d.system ? 'drive.kind.system' : 'drive.kind.' + d.kind);
    return [kind, d.usb ? t('drive.kind.usb') : ''].filter(Boolean).join(' · ');
  }
  function icon(d: Drive): Component {
    if (d.system) return Monitor;
    if (d.usb || d.kind === 'removable') return Usb;
    return ({ network: Server, optical: Disc, ram_disk: MemoryStick } as Record<string, Component>)[d.kind] ?? HardDrive;
  }
  // Motivo en texto de una unidad que no se puede elegir.
  const why = (d: Drive) => t(noReason(d));
  const usedPct = (d: Drive) => (d.total ? Math.round(((d.total - d.free) / d.total) * 100) : 0);
</script>

<section aria-labelledby="drives-title" class="flex flex-col gap-3">
  <div class="flex items-center justify-between gap-3">
    <h2 id="drives-title" class="text-base font-semibold">{t('drive.title')}</h2>
    <Button variant="ghost" disabled={work.drivesLoading || disabled} onclick={loadDrives}>
      <RefreshCw aria-hidden="true" class={work.drivesLoading ? 'animate-spin' : ''} />{t('drive.refresh')}
    </Button>
  </div>

  <fieldset class="m-0 flex min-w-0 flex-col gap-3 border-0 p-0" {disabled}>
    <legend class="sr-only">{t('drive.legend')}</legend>
    {#if !work.drives}
      <p class="text-muted-foreground" role="status">{t('drive.loading')}</p>
    {/if}
    <!-- Filas separadas por lineas, sin tarjetas: tambien va dentro del dialogo de la navegacion. La elegida, en cian. -->
    <ul class="divide-y divide-border border-y border-border">
      {#each drives as d (d.root)}
        {@const s = selectable(d)}
        {@const Icon = icon(d)}
        {@const pct = usedPct(d)}
        <li>
          <label
            class="group flex min-h-12 flex-wrap items-center gap-x-3 gap-y-1 border-l-2 border-transparent px-3 py-1.5 transition-colors -outline-offset-2
              has-[:checked]:border-primary has-[:checked]:bg-primary/10 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-ring
              {s === 'no' ? 'cursor-not-allowed' : 'cursor-pointer hover:bg-raised/50'}"
          >
            <input type="radio" name="drive" value={d.root} bind:group={app.pick} disabled={s === 'no'} class="sr-only" />
            {@render dot()}
            <Icon size={16} class="shrink-0 text-muted-foreground" aria-hidden="true" />
            <span class="flex min-w-40 flex-1 flex-col" class:opacity-60={s === 'no'}>
              <span class="flex min-w-0 items-baseline gap-2">
                <span class="font-semibold">{d.root.replace(/\\$/, '')}</span>
                <span class="truncate" title={d.label}>{d.label || t('drive.noLabel')}</span>
              </span>
              <span class="text-[12px] text-muted-foreground">
                {s === 'no' ? `${kindText(d)} · ${why(d)}` : kindText(d)}
                {#if s === 'confirm'}<span class="text-warning">· {t('drive.confirmNote')}</span>{/if}
              </span>
            </span>
            {#if d.total > 0}
              <span class="flex w-44 shrink-0 flex-col gap-1" class:opacity-60={s === 'no'}>
                <span class="h-1 overflow-hidden rounded-full bg-chart-free" aria-hidden="true">
                  <span class="block h-full rounded-full bg-chart-used" style:width="{pct}%"></span>
                </span>
                <span class="text-[12px] tabular-nums">{t('drive.free', { free: fmt(d.free) })} <span class="text-muted-foreground">{t('drive.of', { total: fmt(d.total) })}</span></span>
              </span>
            {/if}
          </label>
        </li>
      {/each}
      <li>
        <label class="group flex min-h-11 cursor-pointer items-center gap-3 border-l-2 border-transparent px-3 -outline-offset-2 hover:bg-raised/50 has-[:checked]:border-primary has-[:checked]:bg-primary/10 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-ring">
          <input type="radio" name="drive" value={null} bind:group={app.pick} class="sr-only" />
          {@render dot()}
          <FolderOpen size={16} class="text-muted-foreground" aria-hidden="true" />
          <span class="flex-1">{t('drive.other')}</span>
        </label>
        {#if app.pick === null}
          <div class="flex flex-col gap-1.5 px-3 pb-3 pl-[42px]">
            <label for="scan-root" class="text-[13px] text-muted-foreground">{t('scan.root')}</label>
            <input
              id="scan-root"
              bind:value={app.scanRoot}
              spellcheck="false"
              placeholder={t('drive.otherHint')}
              class="h-9 max-w-xl rounded-md border border-input bg-transparent px-3 transition-colors placeholder:text-muted-foreground focus-visible:border-primary"
            />
          </div>
        {/if}
      </li>
    </ul>
  </fieldset>
</section>

<!-- Indicador de radio: el punto marca la elegida, ademas del borde. -->
{#snippet dot()}
  <span aria-hidden="true" class="grid size-[18px] shrink-0 place-items-center rounded-full border-2 border-input group-has-[:checked]:border-primary">
    <span class="hidden size-2 rounded-full bg-primary group-has-[:checked]:block"></span>
  </span>
{/snippet}
