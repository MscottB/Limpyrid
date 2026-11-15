<script lang="ts">
  // Navegacion de la mesa de trabajo: marca, disco, Inicio, grupos de limpieza, analisis, idioma, aviso de version y Acerca de.
  // Tambien carga lo que la navegacion muestra (unidades, categorias, worktrees), porque esta siempre a la vista.
  import { tick, untrack, type Component } from 'svelte';
  import House from '@lucide/svelte/icons/house';
  import FolderTree from '@lucide/svelte/icons/folder-tree';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import Info from '@lucide/svelte/icons/info';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { app, dataRev, ensureWorktrees, loadCats, loadDrives, openGroup, scanCheck, setLang, work, type View } from './state.svelte';
  import { pruneSelection, currentPlan } from './selection.svelte';
  import { bytesOn, driveOf, recovery } from './recovery';
  import { navGroups } from './nav';
  import { keyAction } from './keys';
  import { t, LANGS } from './i18n';
  import { formatBytes } from './format';
  import { isDemo } from './api';
  import { showNotice } from './update';
  import { openDialog, update } from './update.svelte';
  import * as Select from '$lib/components/ui/select';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import Logo from './Logo.svelte';
  import DrivePicker from './DrivePicker.svelte';

  let { onscan }: { onscan: () => void } = $props();

  // Unidades al abrir y cuando cambian los datos (fin de un escaneo o de una limpieza).
  $effect(() => {
    void dataRev.n;
    untrack(() => void loadDrives());
  });
  // Con un escaneo hecho, lo que se puede limpiar se mide aqui (la navegacion siempre esta a la vista).
  $effect(() => {
    void dataRev.n;
    void app.scan.root;
    untrack(() => {
      if (app.scan.root && app.scan.status !== 'running' && (!work.cats || work.rev !== dataRev.n) && !work.catsLoading)
        void loadCats().then((l) => l && pruneSelection());
    });
  });
  // Los worktrees que se pueden quitar cuentan en los grupos. En silencio: un fallo aqui no tapa el informe de una limpieza.
  $effect(() => {
    void dataRev.n;
    void work.minAge;
    void work.cats;
    void app.scan.status;
    untrack(() => void ensureWorktrees(true)?.then(pruneSelection));
  });

  const fmt = (n: number) => formatBytes(n, app.lang);
  const scanned = $derived(!!app.scan.root);
  const running = $derived(app.scan.status === 'running');
  const drive = $derived(app.scan.root ? driveOf(app.scan.root, work.drives ?? []) : undefined);
  const driveName = $derived(drive ? drive.root.replace(/\\$/, '') : '');
  // Lo marcado que esta en esta unidad: el tramo verde de la barra.
  const r = $derived(drive ? recovery(drive, bytesOn(currentPlan(), work.cats ?? [], drive.root)) : null);
  const pct = (n: number) => (drive && drive.total ? (n / drive.total) * 100 : 0);
  const nav = $derived(navGroups(work.cats ?? [], work.wt.list?.worktrees ?? []));
  const check = $derived(scanCheck());

  // La lista de grupos se desplaza sola lo justo para que el grupo activo se vea, tambien si se reordena tras limpiar.
  let groupsEl = $state<HTMLElement>();
  $effect(() => {
    void app.view;
    void app.group;
    void nav.active;
    tick().then(() => groupsEl?.querySelector('[aria-current="page"]')?.scrollIntoView({ block: 'nearest' }));
  });

  let pickerOpen = $state(false);
  function scanPicked() {
    pickerOpen = false;
    onscan();
  }

  // Flechas arriba y abajo entre los elementos de la navegacion; derecha, a la primera fila del centro.
  function onkey(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey || !el.matches('[data-nav]')) return;
    const action = keyAction({ key: e.key, target: 'nav' });
    if (action !== 'next' && action !== 'prev' && action !== 'to-list') return;
    e.preventDefault();
    if (action === 'to-list') {
      const center = document.getElementById('center');
      return (center?.querySelector<HTMLElement>('[data-row]') ?? center?.querySelector<HTMLElement>('button:not(:disabled), input:not(:disabled), [href]'))?.focus();
    }
    const all = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[data-nav]:not(:disabled)')].filter((x) => x.offsetParent);
    all[all.indexOf(el) + (action === 'next' ? 1 : -1)]?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<nav aria-label={t('nav.main')} class="flex min-h-0 flex-col border-r border-border bg-surface" onkeydown={onkey}>
  <!-- Fijo arriba: marca, disco, Inicio. Solo la lista de grupos se desplaza; Analizar y el pie quedan fijos abajo. -->
  <div class="flex h-14 shrink-0 items-center gap-2.5 px-4">
    <Logo size={22} />
    <span class="font-medium">Limpyrid</span>
  </div>
  {#if isDemo}
    <div class="-mt-2 mb-2 px-4">
      <Badge variant="secondary" title={t('demo.note')}>{t('demo.badge')}<span class="sr-only">: {t('demo.note')}</span></Badge>
    </div>
  {/if}

  <!-- Disco: la unidad abre el selector; el icono vuelve a escanear lo mismo. -->
  <div class="mx-3 mb-3 flex flex-col gap-2 rounded-lg border border-border p-2.5">
    <div class="flex items-center gap-1">
      <button type="button" data-nav aria-haspopup="dialog" class="min-w-0 flex-1 truncate rounded-md px-1 py-0.5 text-left font-medium hover:bg-raised" onclick={() => (pickerOpen = true)}>
        {drive ? t('nav.disk', { drive: driveName }) : t('nav.pickDrive')}
      </button>
      {#if scanned}
        <Button variant="ghost" size="icon-sm" data-nav aria-label={running ? t('scan.running') : t('nav.rescan')} disabled={running || !check.ok} onclick={onscan}>
          <RefreshCw aria-hidden="true" class={running ? 'animate-spin' : ''} />
        </Button>
      {/if}
    </div>
    {#if drive && drive.total > 0 && r}
      <span
        role="img"
        aria-label={t('disk.bar', { drive: driveName, used: fmt(drive.total - drive.free), total: fmt(drive.total) })}
        class="flex h-1.5 overflow-hidden rounded-full bg-chart-free"
      >
        <span class="h-full bg-chart-used" style:width="{pct(r.usedAfter)}%"></span>
        <span class="h-full bg-success" style:width="{pct(r.recover)}%"></span>
      </span>
      <p class="text-[12px] text-muted-foreground tabular-nums">{t('drive.free', { free: fmt(drive.free) })} {t('drive.of', { total: fmt(drive.total) })}</p>
    {/if}
  </div>

  <ul class="flex flex-col gap-0.5 px-2">
    <li>{@render item('home', House)}</li>
  </ul>

  <h2 class="mt-4 mb-1 px-4 text-[12px] font-medium text-muted-foreground">{t('nav.clean')}</h2>
  <div bind:this={groupsEl} class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto pb-2">
    {#if !scanned}
      <p class="px-4 text-[12px] text-muted-foreground">{t('nav.noScan')}</p>
    {:else}
      <ul class="flex flex-col gap-0.5 px-2">
        {#each nav.active as g (g.ecosystem)}
          {@const active = app.view === 'group' && app.group === g.ecosystem}
          <li>
            <button
              type="button"
              data-nav
              aria-current={active ? 'page' : undefined}
              onclick={() => openGroup(g.ecosystem)}
              class="flex h-8 w-full items-center gap-2 rounded-md px-2 text-left transition-colors {active ? 'bg-primary/15 text-primary' : 'hover:bg-raised'}"
            >
              <span class="min-w-0 flex-1 truncate">{t('eco.' + g.ecosystem)}</span>
              {#if g.bytes > 0}<span class="text-[12px] tabular-nums {active ? '' : 'text-muted-foreground'}">{fmt(g.bytes)}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
      {#if nav.empty.length}
        <details class="group px-2">
          <summary data-nav class="flex h-8 cursor-pointer list-none items-center gap-1 rounded-md px-2 text-[12px] text-muted-foreground hover:bg-raised">
            <ChevronRight size={14} class="transition-transform group-open:rotate-90" aria-hidden="true" />{t('nav.emptyGroups', { n: nav.empty.length })}
          </summary>
          <ul class="flex flex-col gap-0.5">
            {#each nav.empty as eco (eco)}
              {@const active = app.view === 'group' && app.group === eco}
              <li>
                <button type="button" data-nav aria-current={active ? 'page' : undefined} onclick={() => openGroup(eco)} class="flex h-8 w-full items-center rounded-md px-2 pl-6 text-left text-muted-foreground hover:bg-raised {active ? 'bg-primary/15 text-primary' : ''}">
                  <span class="truncate">{t('eco.' + eco)}</span>
                </button>
              </li>
            {/each}
          </ul>
        </details>
      {/if}
    {/if}
  </div>

  <div class="shrink-0 pb-2">
    <h2 class="mt-4 mb-1 px-4 text-[12px] font-medium text-muted-foreground">{t('nav.analyze')}</h2>
    {#if !scanned}<p class="mb-1 px-4 text-[12px] text-muted-foreground">{t('nav.noScan')}</p>{/if}
    <ul class="flex flex-col gap-0.5 px-2">
      <li>{@render item('explore', FolderTree, !scanned)}</li>
      <li>{@render item('map', LayoutGrid, !scanned)}</li>
    </ul>
  </div>

  <div class="flex shrink-0 flex-col gap-2 border-t border-border px-2 py-3">
    <Select.Root type="single" value={app.lang} onValueChange={setLang}>
      <Select.Trigger data-nav aria-label={t('nav.lang')} lang={app.lang} class="mx-2 w-auto">
        {LANGS.find((l) => l.code === app.lang)?.name}
      </Select.Trigger>
      <Select.Content>
        {#each LANGS as l (l.code)}
          <Select.Item value={l.code} label={l.name} lang={l.code} />
        {/each}
      </Select.Content>
    </Select.Root>
    {#if showNotice(update) && update.info}
      <!-- Discreto a proposito: no compite con el boton principal de la pantalla. -->
      <button type="button" data-nav aria-haspopup="dialog" onclick={openDialog} class="flex h-8 w-full items-center rounded-md px-2 text-left text-[12px] text-muted-foreground tabular-nums transition-colors hover:bg-raised hover:text-foreground">
        <span class="truncate">{t('update.notice', { v: update.info.version })}</span>
      </button>
    {/if}
    {@render item('about', Info)}
  </div>
</nav>

<Dialog.Root bind:open={pickerOpen}>
  <Dialog.Content class="max-h-[calc(100vh-48px)] gap-5 overflow-auto sm:max-w-[640px]">
    <Dialog.Header>
      <!-- DrivePicker ya lleva el titulo visible. -->
      <Dialog.Title class="sr-only">{t('drive.title')}</Dialog.Title>
    </Dialog.Header>
    <DrivePicker disabled={running} />
    {#if check.reason}
      <p class="text-[13px] {check.rule === 'no' ? 'font-medium text-destructive' : 'text-warning'}">{t(check.reason)}</p>
    {/if}
    {#if work.busy}<p class="text-[13px] text-muted-foreground">{t('home.waitClean')}</p>{/if}
    <Dialog.Footer>
      <Button disabled={running || !check.ok} onclick={scanPicked}>{t('scan.start')}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<!-- El activo se distingue por fondo y color, y lo anuncia aria-current. -->
{#snippet item(view: View, Icon: Component, disabled = false)}
  {@const active = app.view === view}
  <button
    type="button"
    data-nav
    {disabled}
    aria-current={active ? 'page' : undefined}
    onclick={() => (app.view = view)}
    class="flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-50
      {active ? 'bg-primary/15 text-primary' : 'hover:bg-raised'}"
  >
    <Icon size={16} aria-hidden="true" />
    {t('nav.' + view)}
  </button>
{/snippet}
