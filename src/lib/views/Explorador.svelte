<script lang="ts">
  // Explorador: la carpeta actual como lista ordenable y filtrable. Es tambien la forma accesible del Mapa (teclado y
  // lectores de pantalla). Los elementos se identifican por ruta, nunca por nombre: puede haber dos <small> hermanos.
  import { tick, untrack } from 'svelte';
  import Folder from '@lucide/svelte/icons/folder';
  import File from '@lucide/svelte/icons/file';
  import Boxes from '@lucide/svelte/icons/boxes';
  import Search from '@lucide/svelte/icons/search';
  import ArrowDownWideNarrow from '@lucide/svelte/icons/arrow-down-wide-narrow';
  import PanelRight from '@lucide/svelte/icons/panel-right';
  import ScanSearch from '@lucide/svelte/icons/scan-search';
  import { Button } from '$lib/components/ui/button';
  import * as Select from '$lib/components/ui/select';
  import * as api from '../api';
  import type { Entry } from '../api';
  import { app, dataRev, openDetail } from '../state.svelte';
  import { t } from '../i18n';
  import { formatBytes, formatShare } from '../format';
  import { filterEntries, isInside, isSmall, parentPath, sortEntries, type SortBy } from '../treemap';
  import Breadcrumb from '../Breadcrumb.svelte';
  import ViewState from '../ViewState.svelte';

  let { onscan }: { onscan: () => void } = $props();

  let entries = $state<Entry[] | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let loadToken = 0;
  let filter = $state('');
  let sortBy = $state<SortBy>('size');
  let listEl = $state<HTMLElement>();
  let refocus = false; // tras navegar desde la vista, el foco pasa a la primera fila (el boton pulsado ya no existe)

  const root = $derived(app.scan.root || app.cwd);
  const locked = $derived(app.scan.status === 'running');
  const total = $derived((entries ?? []).reduce((a, e) => a + e.size, 0));
  const biggest = $derived(Math.max(1, ...(entries ?? []).map((e) => e.size)));
  const shown = $derived(sortEntries(filterEntries(entries ?? [], filter, app.lang), sortBy, app.lang));
  const fmt = (n: number) => formatBytes(n, app.lang);
  const count = (n: number) => n.toLocaleString(app.lang);

  async function load() {
    const token = ++loadToken;
    loading = true;
    try {
      const list = await api.children(app.cwd);
      if (token !== loadToken) return;
      entries = list;
      error = null;
    } catch (e) {
      if (token !== loadToken) return;
      entries = null;
      error = api.errCode(e);
    } finally {
      if (token === loadToken) loading = false;
    }
    if (token === loadToken && refocus) {
      refocus = false;
      await tick();
      listEl?.querySelector<HTMLElement>('button')?.focus();
    }
  }

  // Recarga al cambiar de carpeta (aqui o en el Mapa) y cuando el backend tiene datos nuevos (fin de un escaneo, Papelera).
  $effect(() => {
    void app.cwd;
    void dataRev.n;
    untrack(load);
  });

  function go(path: string) {
    filter = ''; // el filtro es de la carpeta actual
    refocus = true;
    app.cwd = path;
  }
  function pick(e: Entry, from: Element | null) {
    app.picked = e;
    openDetail(from);
  }
  const canUp = $derived(isInside(app.cwd, root) && !isInside(root, app.cwd));

  // Backspace sube de carpeta, salvo escribiendo en un campo o con un dialogo abierto.
  function onkeydown(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    if (e.key !== 'Backspace' || e.defaultPrevented || !canUp || locked) return;
    if (['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) || el.isContentEditable || el.closest('[role="dialog"], [data-slot^="select-"]')) return;
    e.preventDefault();
    go(parentPath(app.cwd));
  }
</script>

<svelte:window {onkeydown} />

<section class="flex flex-col gap-5" aria-busy={loading}>
  {#if error !== 'no_scan'}
    <!-- Una fila con linea inferior, como la cabecera de un grupo. -->
    <div class="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-2 border-b border-border pb-2">
      <Breadcrumb {root} path={app.cwd} onnavigate={go} />
      {#if entries && !locked}
        <span class="text-[13px] text-muted-foreground tabular-nums">{t(entries.length === 1 ? 'explore.summary.one' : 'explore.summary.other', { size: fmt(total), n: count(entries.length) })}</span>
      {/if}
      {#if filter.trim() && entries}
        <span class="text-[13px] text-muted-foreground tabular-nums" aria-live="polite">· {t('explore.shown', { shown: count(shown.length), n: count(entries.length) })}</span>
      {/if}
      <span class="flex-1"></span>
      {#if entries?.length && !locked}
        <Select.Root type="single" bind:value={sortBy}>
          <Select.Trigger size="sm" aria-label={t('explore.sort')} title={t('explore.sort')}><ArrowDownWideNarrow class="text-muted-foreground" aria-hidden="true" />{t(sortBy === 'size' ? 'explore.size' : 'explore.name')}</Select.Trigger>
          <Select.Content>
            <Select.Item value="size" label={t('explore.size')} />
            <Select.Item value="name" label={t('explore.name')} />
          </Select.Content>
        </Select.Root>
        <div class="relative w-52">
          <Search size={14} class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
          <input type="search" bind:value={filter} aria-label={t('filter.label')} placeholder={t('filter.label')} spellcheck="false" class="h-8 w-full rounded-md border border-input bg-transparent pr-2 pl-8 text-[13px] focus-visible:border-primary" />
        </div>
      {/if}
    </div>
  {/if}

  {#if locked}
    <ViewState kind="busy" text={t('scan.running') + (app.scan.progress ? ' ' + t('scan.progress', { dirs: count(app.scan.progress.dirs), size: fmt(app.scan.progress.bytes) }) : '')} />
  {:else if error === 'no_scan'}
    <div class="flex flex-col items-start gap-4 rounded-xl border border-border bg-surface p-6">
      <span class="grid size-10 place-items-center rounded-lg bg-raised text-muted-foreground"><ScanSearch size={20} aria-hidden="true" /></span>
      <p class="max-w-[60ch]">{t('view.noScan')}</p>
      <Button onclick={onscan}>{t('view.goHome')}</Button>
    </div>
  {:else if error}
    <ViewState kind="error" text={t('err.' + error)} />
  {:else if entries && !entries.length}
    <ViewState kind="empty" text={t('view.empty')} />
  {:else if entries}
    {#if !shown.length}
      <ViewState kind="empty" text={t('filter.none')} />
    {:else}
      <div bind:this={listEl} class="@container -mt-3">
        <table class="w-full table-fixed border-collapse text-left">
          <caption class="sr-only">{t('explore.title')}: {app.cwd}</caption>
          <thead>
            <tr class="border-b border-border text-[13px] text-muted-foreground">
              <th scope="col" class="w-10 py-2.5 pl-3"><span class="sr-only">{t('explore.kind')}</span></th>
              <th scope="col" class="py-2.5 pr-3 font-medium">{t('explore.name')}</th>
              <th scope="col" class="w-[16%] py-2.5 pr-3 @max-xl:hidden"><span class="sr-only">{t('explore.bar')}</span></th>
              <th scope="col" class="num w-16 py-2.5 pr-3 font-medium" title={t('explore.share')}>%<span class="sr-only"> {t('explore.share')}</span></th>
              <th scope="col" class="num w-24 py-2.5 pr-3 font-medium @max-2xl:hidden">{t('explore.files')}</th>
              <th scope="col" class="num w-24 py-2.5 pr-3 font-medium">{t('explore.size')}</th>
              <th scope="col" class="w-12 py-2.5 pr-3"><span class="sr-only">{t('detail.title')}</span></th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            {#each shown as e (e.path)}
              {@const small = isSmall(e.name)}
              {@const Icon = small ? Boxes : e.is_dir ? Folder : File}
              {@const on = app.picked?.path === e.path}
              <!-- El clic en la fila elige (el boton del nombre entra en la carpeta); con el teclado, el boton de detalle. -->
              <tr aria-current={on ? 'true' : undefined} class="h-9 border-l-2 transition-colors {on ? 'border-l-primary bg-primary/10' : 'border-l-transparent hover:bg-raised/50'} {small ? '' : 'cursor-pointer'}"
                onclick={(ev) => !small && !(ev.target as HTMLElement).closest('button') && pick(e, ev.currentTarget.querySelector('td:last-child button'))}>
                <td class="pl-3 text-muted-foreground">
                  <Icon size={16} aria-hidden="true" />
                  <span class="sr-only">{t(small ? 'explore.kind.small' : e.is_dir ? 'explore.kind.dir' : 'explore.kind.file')}</span>
                </td>
                <th scope="row" class="pr-3 font-normal">
                  {#if small}
                    <span class="block truncate text-muted-foreground italic">{t('node.small')}</span>
                  {:else if e.is_dir}
                    <button type="button" title={e.name} onclick={() => go(e.path)} class="-ml-2 flex h-8 max-w-full items-center rounded-md px-2 font-medium underline-offset-4 transition-colors hover:bg-raised hover:underline">
                      <span class="truncate">{e.name}</span>
                    </button>
                  {:else}
                    <span class="block truncate" title={e.name}>{e.name}</span>
                  {/if}
                </th>
                <td class="pr-3 @max-xl:hidden" aria-hidden="true">
                  <span class="block h-1.5 overflow-hidden rounded-full bg-raised">
                    <span class="block h-full min-w-px rounded-full {small ? 'bg-input' : 'bg-chart-used'}" style:width="{(e.size / biggest) * 100}%"></span>
                  </span>
                </td>
                <td class="num pr-3 text-muted-foreground">{formatShare(e.size, total, app.lang)}</td>
                <td class="num pr-3 text-muted-foreground @max-2xl:hidden">{count(e.files)}</td>
                <td class="num pr-3 font-semibold">{fmt(e.size)}</td>
                <td class="pr-3 text-right">
                  {#if !small}
                    <Button variant="ghost" size="icon-sm" aria-label={t('explore.details', { name: e.name })} title={t('explore.details', { name: e.name })} onclick={(ev) => pick(e, ev.currentTarget)}><PanelRight aria-hidden="true" /></Button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {:else if loading}
    <ViewState kind="busy" text={t('view.loading')} />
  {/if}
</section>

