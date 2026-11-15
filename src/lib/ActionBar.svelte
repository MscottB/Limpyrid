<script lang="ts">
  // Barra de accion fija: lo marcado (y su lista, hacia arriba), como quedaria el disco y el unico boton principal.
  // Limpiando, la barra entera es el progreso.
  import { tick } from 'svelte';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import X from '@lucide/svelte/icons/x';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { app, work } from './state.svelte';
  import { clearSelection, currentPlan, setCategory, setItems } from './selection.svelte';
  import { bytesOn, driveOf, recovery } from './recovery';
  import { canClean, cleanSelection, itemLabel, staleWorktrees } from './guard.svelte';
  import { impactOf } from './impact';
  import { t } from './i18n';
  import { formatBytes } from './format';

  const fmt = (n: number) => formatBytes(n, app.lang);
  const plan = $derived(currentPlan());
  const planCount = $derived(plan.reduce((a, e) => a + Math.max(1, e.items.length), 0));
  const planSize = $derived(plan.reduce((a, e) => a + e.size, 0));
  const ready = $derived(canClean());
  const stale = $derived(staleWorktrees());

  // Solo lo marcado en la unidad escaneada cambia sus cifras; lo de otras unidades se dice aparte.
  const drive = $derived(app.scan.root ? driveOf(app.scan.root, work.drives ?? []) : undefined);
  const onDrive = $derived(drive ? bytesOn(plan, work.cats ?? [], drive.root) : 0);
  const r = $derived(drive ? recovery(drive, onDrive) : null);
  const elsewhere = $derived(planSize - onDrive);

  // Lista de marcados: se cierra sola sin seleccion o al empezar a limpiar, con Esc (este donde este el foco, salvo con
  // un dialogo abierto) y con un clic fuera de la barra. Si el foco estaba en la barra, vuelve a su boton.
  let open = $state(false);
  let listBtn = $state<HTMLButtonElement | null>(null);
  let listEl = $state<HTMLElement | null>(null);
  let barEl = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!plan.length || work.busy) open = false;
  });
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open || e.defaultPrevented || document.querySelector('[role="dialog"]')) return;
    e.preventDefault();
    open = false;
    if (barEl?.contains(document.activeElement)) listBtn?.focus();
  }
  function onpointerdown(e: PointerEvent) {
    if (open && !barEl?.contains(e.target as Node)) open = false;
  }
  // Al quitar una entrada el foco pasa a la siguiente; si era la ultima, al boton de la lista.
  async function remove(e: MouseEvent, undo: () => void) {
    const btns = () => [...(listEl?.querySelectorAll<HTMLButtonElement>('button[data-remove]') ?? [])];
    const i = btns().indexOf(e.currentTarget as HTMLButtonElement);
    undo();
    await tick();
    (btns()[i] ?? listBtn ?? barEl)?.focus(); // sin seleccion el boton ya no se dibuja: el foco queda en la barra
  }

  // Progreso: los worktrees no emiten bytes (barra indeterminada con el nombre del paso).
  const run = $derived(work.progress);
  let now = $state(Date.now());
  $effect(() => {
    if (!run) return;
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });
  const bytes = $derived(!!run && work.cats?.find((c) => c.id === run.id)?.kind !== 'worktrees');
  const pct = $derived(run && run.expected > 0 ? Math.min(100, (run.freed / run.expected) * 100) : 0);
  const elapsed = $derived.by(() => {
    const s = run ? Math.max(0, Math.floor((now - run.startedAt) / 1000)) : 0;
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  });
  // Solo se anuncian los cambios de paso, no cada avance.
  const announce = $derived(run ? t('progress.announce', { name: t('cat.' + run.id), n: run.index, total: run.total }) : '');
</script>

<svelte:window {onkeydown} {onpointerdown} />

<!-- Un tono por debajo del centro (spec): sale del fondo mezclado con negro, sin color nuevo. -->
<div bind:this={barEl} tabindex="-1" class="relative col-span-full border-t border-border bg-[color-mix(in_srgb,var(--bg)_70%,black)] outline-none">
  <p class="sr-only" aria-live="polite">{announce}</p>
  {#if open}
    <section id="marked-list" bind:this={listEl} aria-labelledby="marked-title" class="absolute bottom-full left-3 z-20 mb-2 flex max-h-[min(60vh,480px)] w-[min(640px,calc(100%-24px))] flex-col overflow-hidden rounded-lg border border-input bg-raised">
      <div class="flex items-center justify-between gap-3 border-b border-border py-1 pr-2 pl-5">
        <h2 id="marked-title" class="text-[13px] font-semibold">{t('tray.title')}</h2>
        <Button variant="ghost" size="sm" onclick={() => { clearSelection(); open = false; barEl?.focus(); }}>{t('tray.clearAll')}</Button>
      </div>
      <ul class="divide-y divide-border overflow-auto">
        {#each plan as e (e.id)}
          {@const name = t('cat.' + e.id)}
          {@const level = impactOf(e.id)}
          <li class="px-5 py-1">
            <div class="flex min-h-9 items-center gap-3">
              <span class="min-w-0 flex-1 truncate font-medium">{name}</span>
              <Badge variant={level === 'careful' ? 'warning' : 'secondary'}>{t('impact.' + level)}</Badge>
              <span class="num w-24 font-semibold">{fmt(e.size)}</span>
              {#if e.kind === 'contents'}
                <Button variant="ghost" size="icon" data-remove aria-label={t('tray.remove', { name })} onclick={(ev: MouseEvent) => remove(ev, () => setCategory(e.id, false))}><X aria-hidden="true" /></Button>
              {:else}
                <span class="size-9 shrink-0" aria-hidden="true"></span>
              {/if}
            </div>
            {#if e.items.length}
              <ul class="ml-1 border-l-2 border-border pl-3 text-[13px]">
                {#each e.items as i (i.path)}
                  {@const what = itemLabel(e, i.path)}
                  <li class="flex min-h-9 items-center gap-3">
                    <span class="min-w-0 flex-1 [overflow-wrap:anywhere]">{what}</span>
                    <span class="num w-24">{fmt(i.size)}</span>
                    <Button variant="ghost" size="icon" data-remove aria-label={t('tray.remove', { name: what })} onclick={(ev: MouseEvent) => remove(ev, () => setItems(e.id, [i.path], false))}><X aria-hidden="true" /></Button>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if run}
    <!-- Una linea de datos a cualquier ancho: en estrecho ceden el nombre (con title), los borrados y el tiempo; nunca el paso ni lo liberado. -->
    <section aria-label={t('progress.title')} class="flex min-h-14 flex-col justify-center gap-1 px-5 py-2 tabular-nums">
      <div class="flex min-w-0 items-center gap-x-4 whitespace-nowrap">
        <LoaderCircle size={16} class="shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
        <span class="min-w-0 truncate font-semibold" title={t('cat.' + run.id)}>{t('cat.' + run.id)}</span>
        <span class="shrink-0 text-[13px] text-muted-foreground">{t('progress.step', { n: run.index, total: run.total })}</span>
        {#if bytes}
          <span class="shrink-0 text-[13px]">{t('progress.freed', { freed: fmt(run.freed), expected: fmt(run.expected) })}</span>
          <span class="shrink-0 text-[13px] text-muted-foreground max-[1199px]:hidden">{t(run.deleted === 1 ? 'progress.deleted.one' : 'progress.deleted.other', { n: run.deleted.toLocaleString(app.lang) })}</span>
          <div role="progressbar" aria-label={t('cat.' + run.id)} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} class="h-1.5 min-w-16 flex-1 overflow-hidden rounded-full bg-chart-free">
            <span class="block h-full rounded-full bg-success transition-[width] duration-150" style:width="{pct}%"></span>
          </div>
        {:else}
          <span class="min-w-0 truncate text-[13px] text-muted-foreground" title={t('progress.worktrees')}>{t('progress.worktrees')}</span>
          <div role="progressbar" aria-label={t('cat.' + run.id)} class="indeterminate relative h-1.5 min-w-16 flex-1 overflow-hidden rounded-full bg-chart-free">
            <span class="sweep absolute inset-y-0 rounded-full bg-chart-used"></span>
          </div>
        {/if}
        <span class="shrink-0 text-[13px] text-muted-foreground max-[999px]:hidden">{t('progress.elapsed', { time: elapsed })}</span>
      </div>
      <p class="truncate text-[12px] text-muted-foreground" title={t('progress.keepOpen')}>{t('progress.keepOpen')}</p>
    </section>
  {:else if work.busy}
    <p class="flex min-h-14 items-center gap-3 px-5" role="status"><LoaderCircle size={16} class="animate-spin text-muted-foreground" aria-hidden="true" />{t('clean.busy')}</p>
  {:else}
    <div class="flex min-h-14 flex-wrap items-center gap-x-6 gap-y-1 px-5 py-2 tabular-nums">
      {#if plan.length}
        <Button variant="ghost" bind:ref={listBtn} aria-expanded={open} aria-controls="marked-list" onclick={() => (open = !open)} class="-ml-3">
          <ChevronUp class="transition-transform {open ? 'rotate-180' : ''}" aria-hidden="true" />{t(planCount === 1 ? 'bar.count.one' : 'bar.count.other', { n: planCount, size: fmt(planSize) })}
        </Button>
      {/if}
      <span class="flex-1"></span>
      {#if drive && r}
        <!-- Sin seleccion, solo el estado del disco; con ella, la proyeccion, que es un maximo (lo explica el title). -->
        <p class="flex items-center gap-2 text-[13px]" title={plan.length ? t('bar.note') : undefined}>
          <span class="text-muted-foreground">{drive.root.replace(/\\$/, '')}</span>{t('drive.free', { free: fmt(r.freeNow) })}
          {#if plan.length}
            <ArrowRight size={14} class="text-muted-foreground" aria-hidden="true" /><span class="sr-only">{t('bar.then')}</span>
            <span class="font-semibold text-success">{t('bar.after', { free: fmt(r.freeAfter) })}</span>
            {#if elsewhere > 0}<span class="text-muted-foreground">· {t('bar.otherDrives', { size: fmt(elsewhere) })}</span>{/if}
          {/if}
        </p>
      {/if}
      <span class="flex-1"></span>
      {#if plan.length}
        <Button size="lg" disabled={!ready} onclick={cleanSelection}>{t('bar.clean', { size: fmt(planSize) })}</Button>
      {:else}
        <span class="text-muted-foreground">{t('bar.none')}</span>
      {/if}
      {#if stale}<span class="basis-full text-right text-[13px] text-muted-foreground">{t('sel.waitWorktrees')}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .sweep {
    left: 0;
    width: 35%;
    animation: sweep 1.4s ease-in-out infinite;
  }
  @keyframes sweep {
    from { transform: translateX(-100%); }
    to { transform: translateX(290%); }
  }
  /* Sin movimiento una barra quieta aparentaria un porcentaje: se quita y queda el texto del paso. */
  @media (prefers-reduced-motion: reduce) {
    .indeterminate { display: none; }
  }
</style>
