<script lang="ts">
  // Lista del grupo elegido en la navegacion (app.group): cabecera con casilla de grupo, filtro y selectores, y una
  // fila por categoria con sus carpetas o worktrees anidados.
  import { untrack } from 'svelte';
  import Search from '@lucide/svelte/icons/search';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Select from '$lib/components/ui/select';
  import type { CategoryInfo, Found, Worktree } from './api';
  import { AGE_CHOICES, app, dataRev, ensureWorktrees, loadCats, locked, openDetail, setMinAge, work } from './state.svelte';
  import { t } from './i18n';
  import { formatAge, formatBytes } from './format';
  import { removable, sortWorktrees } from './worktrees';
  import { isPicked, pruneSelection, selection, setCategory, setItems } from './selection.svelte';
  import { groupTotals, magnitudeRef } from './recovery';
  import { impactOf } from './impact';
  import { groupTargets, staleItems } from './pick';
  import { keyAction } from './keys';
  import ItemRow from './ItemRow.svelte';

  const eco = $derived(app.group ?? '');
  let filter = $state('');
  let filterEl = $state<HTMLInputElement>();
  let listEl = $state<HTMLElement>();
  let open = $state<Record<string, boolean>>({});

  const cats = $derived(work.cats);
  // Una categoria limpiada en esta sesion no desaparece aunque el backend ya no la liste: se dibuja con 0 B.
  const rows = $derived.by(() => {
    const list = cats ?? [];
    const gone = Object.entries(work.cleaned).filter(([id]) => !list.some((c) => c.id === id));
    return [...list, ...gone.map(([id, r]): CategoryInfo => ({ id, ecosystem: r.ecosystem, kind: r.kind, uses_min_age: false, size: 0, paths: [], items: [] }))];
  });
  const wtList = $derived(work.wt.list);
  const hasWorktrees = $derived((cats ?? []).some((c) => c.kind === 'worktrees'));

  const days = (hours: number) => t(hours === 24 ? 'clean.days.one' : 'clean.days.other', { n: hours / 24 });
  const name = (c: CategoryInfo) => t('cat.' + c.id);
  const leaf = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  const parentOf = (p: string) => p.slice(0, Math.max(0, p.length - leaf(p).length)).replace(/[\\/]+$/, '');
  const age = (s: number | null) => (s == null ? t('project.unknownDate') : formatAge(s, Date.now(), app.lang, t('age.today')));

  $effect(() => {
    void dataRev.n;
    void work.minAge;
    void hasWorktrees;
    void app.scan.status;
    untrack(() => void ensureWorktrees()?.then(pruneSelection));
  });
  const retry = () => void loadCats().then((l) => l && pruneSelection());

  // --- Filtro y lo que se puede marcar
  // Sin distinguir mayusculas ni tildes: "cache" encuentra "Caché".
  const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase(app.lang);
  const q = $derived(fold(filter.trim()));
  const has = (...texts: (string | null)[]) => texts.some((s) => s != null && fold(s).includes(q));

  const sortedWts = $derived(sortWorktrees(wtList?.worktrees ?? []));
  const itemsOf = (c: CategoryInfo): Found[] => (c.kind === 'project' || c.kind === 'subfolders' ? [...c.items].sort((a, b) => b.size - a.size) : []);
  // Elementos que se ven: si el nombre de la categoria coincide con el filtro, todos; si no, los que coinciden.
  function shownItems(c: CategoryInfo): Found[] {
    const all = itemsOf(c);
    return !q || has(name(c)) ? all : all.filter((f) => has(f.path));
  }
  function shownWts(c: CategoryInfo): Worktree[] {
    const all = sortedWts;
    return !q || has(name(c)) ? all : all.filter((w) => has(w.name, w.repo, w.branch));
  }
  // Rutas marcables visibles de una categoria por elementos (la casilla de la categoria actua sobre ellas).
  const targets = (c: CategoryInfo): string[] =>
    c.kind === 'worktrees' ? shownWts(c).filter(removable).map((w) => w.path) : shownItems(c).map((f) => f.path);
  const cleanable = (c: CategoryInfo) => (c.kind === 'contents' ? c.size > 0 : targets(c).length > 0);
  // Worktrees: la cifra es lo que se puede quitar, como en la cabecera; el total con los protegidos va en el title.
  const wtTotal = $derived((wtList?.worktrees ?? []).reduce((a, w) => a + w.size, 0));
  const sizeOf = (c: CategoryInfo) => (c.kind === 'worktrees' ? (wtList?.worktrees ?? []).filter(removable).reduce((a, w) => a + w.size, 0) : c.size);
  function visible(c: CategoryInfo): boolean {
    if (!q || has(name(c))) return true;
    return c.kind === 'worktrees' ? shownWts(c).length > 0 : shownItems(c).length > 0;
  }
  // Sin nada que limpiar: al final del grupo, apagada y sin casilla.
  const empty = (c: CategoryInfo) => (c.kind === 'contents' || c.kind === 'project' || c.kind === 'subfolders') && !cleanable(c) && !work.cleaned[c.id];

  // Orden: una limpiada ordena por el tamano que tenia antes, asi no cambia de sitio al volver a medirse.
  const rank = (c: CategoryInfo) => work.cleaned[c.id]?.before ?? sizeOf(c);
  const list = $derived(rows.filter((c) => c.ecosystem === eco && visible(c)).sort((a, b) => Number(empty(a)) - Number(empty(b)) || rank(b) - rank(a)));
  // Referencia de las barras: la fila mayor del grupo, sin contar contenedores (magnitudeRef).
  const biggestRow = $derived(magnitudeRef(list.map((c) => ({ kind: c.kind, size: sizeOf(c) }))));
  // Las anidadas se miden contra la mayor de su categoria (con todo protegido la fila da 0 B y no sirve de referencia).
  const nestedRef = $derived(
    Object.fromEntries(list.map((c) => [c.id, Math.max(1, ...(c.kind === 'worktrees' ? (wtList?.worktrees ?? []) : itemsOf(c)).map((x) => x.size))])),
  );
  const cleanableBytes = $derived(groupTotals(cats ?? [], wtList?.worktrees ?? []).find((g) => g.ecosystem === eco)?.bytes ?? 0);

  // Estado de cada fila en texto: el nivel de consecuencia o, si no es limpiable por seleccion, lo que es.
  function status(c: CategoryInfo): [string, 'level' | null] {
    if (c.kind === 'info_only') return ['status.info', null];
    if (c.kind === 'containers') return ['status.own', null];
    if (c.kind === 'worktrees') return wtList && !wtList.git ? ['status.info', null] : (wtList?.worktrees ?? []).some(removable) ? ['status.removable', null] : ['status.protected', null];
    return ['impact.' + impactOf(c.id), 'level'];
  }

  // Casilla de una categoria: entera (Contenido) o todas sus rutas visibles (por elementos).
  function boxState(c: CategoryInfo): { checked: boolean; indeterminate: boolean } {
    if (c.kind === 'contents') return { checked: selection.categories.has(c.id), indeterminate: false };
    const ts = targets(c);
    const n = ts.filter((p) => isPicked(c.id, p)).length;
    return { checked: ts.length > 0 && n === ts.length, indeterminate: n > 0 && n < ts.length };
  }
  function toggleBox(c: CategoryInfo, on: boolean) {
    if (c.kind === 'contents') setCategory(c.id, on);
    else setItems(c.id, targets(c), on);
  }
  const hasBox = (c: CategoryInfo) => (c.kind === 'contents' || c.kind === 'project' || c.kind === 'subfolders' || c.kind === 'worktrees') && cleanable(c);
  function toggleItem(c: CategoryInfo, path: string, on: boolean) {
    if (locked()) return;
    if (c.kind === 'worktrees') {
      const w = wtList?.worktrees.find((x) => x.path === path);
      if (!w || !removable(w) || work.wt.loading) return;
    }
    setItems(c.id, [path], on);
  }

  // --- Casilla de grupo (groupTargets): todo el grupo, sin filtro; solo escribe en la seleccion.
  function groupBox(eco: string) {
    const g = groupTargets(cats ?? [], eco);
    const items = Object.entries(g.items);
    const total = g.categories.length + items.reduce((a, [, ps]) => a + ps.length, 0);
    const n = g.categories.filter((id) => selection.categories.has(id)).length + items.reduce((a, [id, ps]) => a + ps.filter((p) => isPicked(id, p)).length, 0);
    return { g, total, checked: total > 0 && n === total, indeterminate: n > 0 && n < total };
  }
  function toggleGroup(eco: string, on: boolean) {
    const { g } = groupBox(eco);
    for (const id of g.categories) setCategory(id, on);
    for (const [id, ps] of Object.entries(g.items)) setItems(id, ps, on);
  }
  // Lo marcable del grupo que su casilla no toca (worktrees y lo que pide revision): se dice bajo la cabecera.
  const leftOut = (eco: string) => (cats ?? []).filter((c) => c.ecosystem === eco && hasBox(c) && (c.kind === 'worktrees' || impactOf(c.id) === 'careful')).map(name);
  const gb = $derived(groupBox(eco));
  const out = $derived(gb.total ? leftOut(eco) : []);

  // --- Antiguedad minima y carpetas de proyecto inactivas (staleItems), y listas largas: 100 y "mostrar mas".
  // Sin filtro: el selector no desaparece porque el filtro oculte sus categorias.
  const ageCats = $derived(rows.filter((c) => c.ecosystem === eco && c.uses_min_age));
  // Marcado en bloque, como la casilla de grupo: nunca una categoria de nivel careful.
  const projects = $derived(list.filter((c) => c.kind === 'project' && impactOf(c.id) !== 'careful'));
  const STALE_DAYS = [30, 90, 180];
  const LIST_CAP = 100;
  let staleDays = $state<Record<string, number>>({});
  let staleSel = $state('');
  let showAll = $state<Record<string, boolean>>({});
  const capped = <T,>(id: string, list: T[]) => (showAll[id] ? list : list.slice(0, LIST_CAP));
  const staleOf = (c: CategoryInfo) => staleItems(shownItems(c), staleDays[c.id] ?? 90, Date.now());
  const staleCount = (d: number) => projects.reduce((a, c) => a + staleItems(shownItems(c), d, Date.now()).length, 0);
  function markStale(v: string) {
    if (!v) return;
    for (const c of projects) {
      staleDays[c.id] = Number(v);
      setItems(c.id, staleOf(c), true);
    }
    staleSel = '';
  }

  // Desplegada a mano (de entrada, si tiene pocas anidadas: una lista larga taparia el resto del grupo), o porque el
  // filtro coincide con algo de dentro y no con su nombre.
  const nested = (c: CategoryInfo) => (c.kind === 'worktrees' ? (wtList?.worktrees.length ?? 0) : itemsOf(c).length);
  const opened = (c: CategoryInfo) => (open[c.id] ?? nested(c) <= 10) || (!!q && !has(name(c)));

  // Filas que se dibujan, en orden: categoria, y debajo sus carpetas o sus worktrees por repositorio.
  type Entry = { key: string; c: CategoryInfo; f?: Found; w?: Worktree; repo?: string; more?: number };
  const keyOf = (id: string, path: string | null) => (path === null ? id : id + '\n' + path);
  const entries = $derived.by(() => {
    const out: Entry[] = [];
    for (const c of list) {
      out.push({ key: c.id, c });
      if (!opened(c)) continue;
      const all: (Found | Worktree)[] = c.kind === 'worktrees' ? (wtList ? shownWts(c) : []) : shownItems(c);
      const shown = capped(c.id, all);
      if (c.kind === 'worktrees') {
        for (const repo of new Set((shown as Worktree[]).map((w) => w.repo))) {
          out.push({ key: c.id + '\nrepo\n' + repo, c, repo });
          for (const w of shown as Worktree[]) if (w.repo === repo) out.push({ key: keyOf(c.id, w.path), c, w });
        }
      } else for (const f of shown as Found[]) out.push({ key: keyOf(c.id, f.path), c, f });
      if (all.length > LIST_CAP && !showAll[c.id]) out.push({ key: c.id + '\nmore', c, more: all.length });
    }
    return out;
  });
  const focusKey = $derived(app.focus ? keyOf(app.focus.id, app.focus.path) : null);
  const tabKey = $derived(entries.some((e) => e.key === focusKey) ? focusKey : (entries[0]?.key ?? null));
  // Una fila enfocada que el filtro oculta deja de estarlo: el detalle vuelve al resumen del grupo.
  $effect(() => {
    void q;
    untrack(() => {
      if (focusKey && !entries.some((e) => e.key === focusKey)) app.focus = null;
    });
  });
  const focusRow = (id: string, path: string | null) => {
    app.focus = { id, path };
  };

  // --- Teclado (keyAction): "/" al filtro; en una fila, flechas, espacio, Enter y flecha izquierda. Nunca en un campo ni
  // con un dialogo.
  function onkey(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey || el.closest?.('[role="dialog"]') || document.querySelector('[role="dialog"]')) return;
    const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) || el.isContentEditable;
    const onRow = !!listEl?.contains(el) && el.matches('[data-row]');
    const action = keyAction({ key: e.key, target: typing ? 'input' : onRow ? 'row' : 'other' });
    if (!action || action === 'to-list' || action === 'close') return;
    e.preventDefault();
    if (action === 'focus-filter') return filterEl?.focus();
    if (action === 'to-nav') return document.querySelector<HTMLElement>('nav [aria-current="page"]')?.focus();
    const f = app.focus;
    const c = f && rows.find((x) => x.id === f.id);
    if (!f || !c) return;
    if (action === 'toggle') {
      if (f.path !== null) toggleItem(c, f.path, !isPicked(c.id, f.path));
      else if (hasBox(c) && !locked() && !(c.kind === 'worktrees' && work.wt.loading)) toggleBox(c, !boxState(c).checked);
    } else if (action === 'expand') {
      if (f.path === null && nested(c)) open[c.id] = !opened(c);
      else openDetail();
    } else {
      const all = [...(listEl?.querySelectorAll<HTMLElement>('[data-row]') ?? [])];
      all[all.indexOf(el) + (action === 'next' ? 1 : -1)]?.focus();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<section class="flex min-h-full flex-col" aria-labelledby="group-title" aria-busy={work.catsLoading}>
  <header class="sticky top-0 z-10 border-b border-border bg-background">
    <div class="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-2 py-2 pr-4 pl-[18px]">
      {#if gb.total}
        <Checkbox aria-label={t('group.checkLabel', { name: t('eco.' + eco) })} disabled={locked()} bind:checked={() => gb.checked, (on) => toggleGroup(eco, on)} indeterminate={gb.indeterminate} />
      {/if}
      <h1 id="group-title" class="font-medium">{t('eco.' + eco)}</h1>
      <span class="text-[13px] text-muted-foreground tabular-nums">{t(list.length === 1 ? 'group.count.one' : 'group.count.other', { n: list.length })}</span>
      {#if cleanableBytes}<span class="text-[13px] text-muted-foreground tabular-nums">· {t('group.cleanable', { size: formatBytes(cleanableBytes, app.lang) })}</span>{/if}
      {#if work.catsLoading || work.wt.loading}<span class="text-[12px] text-muted-foreground" role="status">{t('clean.updating')}</span>{/if}
      {#if ageCats.length}
        <Select.Root type="single" value={String(work.minAge)} onValueChange={(v) => setMinAge(Number(v))} disabled={locked()}>
          <Select.Trigger size="sm" aria-label={t('clean.minAge')}>{t('group.age', { days: days(work.minAge) })}</Select.Trigger>
          <Select.Content>
            {#each AGE_CHOICES as h (h)}<Select.Item value={String(h)} label={t('group.age', { days: days(h) })} />{/each}
          </Select.Content>
        </Select.Root>
      {/if}
      {#if projects.length}
        <Select.Root type="single" bind:value={staleSel} onValueChange={markStale} disabled={locked()}>
          <Select.Trigger size="sm">{t('group.stale')}</Select.Trigger>
          <Select.Content>
            {#each STALE_DAYS as d (d)}
              {@const n = staleCount(d)}
              <Select.Item value={String(d)} disabled={!n} label={t('group.staleOption', { days: t('clean.days.other', { n: d }), n })} />
            {/each}
          </Select.Content>
        </Select.Root>
      {/if}
      <!-- El filtro ocupa lo que quede de la linea: a 1100 px la cabecera cabe en una sola. -->
      <div class="relative min-w-[140px] flex-1">
        <Search size={14} class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
        <input type="search" bind:value={filter} bind:this={filterEl} aria-label={t('filter.label')} placeholder={t('filter.placeholder')} spellcheck="false" class="h-8 w-full rounded-md border border-input bg-transparent pr-2 pl-8 text-[13px] focus-visible:border-primary" />
      </div>
    </div>
    <!-- Fallo al recargar con datos ya en pantalla: la lista que sigue es la anterior. -->
    {#if cats && work.catsError && !work.catsLoading}
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1 pr-4 pb-2 pl-[18px] text-[13px]" role="alert">
        <p class="text-destructive">{t('err.' + work.catsError)}</p>
        <Button variant="outline" size="sm" onclick={retry}>{t('group.retry')}</Button>
      </div>
    {/if}
    {#if out.length || filter.trim()}
      <div class="flex flex-col gap-0.5 pr-4 pb-2 pl-[18px] text-[12px] text-muted-foreground">
        {#if out.length}<p>{t('group.leavesOut', { names: out.join(', ') })}</p>{/if}
        {#if filter.trim()}<p>{t('group.filterNote')}</p>{/if}
      </div>
    {/if}
  </header>

  {#if !cats && !work.catsLoading && work.catsError}
    <div class="flex flex-col items-start gap-3 p-6" role="alert">
      <p class="text-destructive">{t('err.' + work.catsError)}</p>
      <Button variant="outline" onclick={retry}>{t('group.retry')}</Button>
    </div>
  {:else if !cats}
    <ul class="divide-y divide-border" aria-label={t('clean.loading')}>
      {#each [70, 55, 62, 40, 48, 35] as w}
        <li class="flex h-9 items-center gap-3 px-4"><span class="size-4 rounded bg-raised"></span><span class="h-3 animate-pulse rounded bg-raised" style:width="{w}%"></span></li>
      {/each}
    </ul>
  {:else if !list.length}
    <p class="p-6 text-muted-foreground">{t(q ? 'filter.none' : 'clean.empty')}</p>
  {:else}
    <ul bind:this={listEl} class="divide-y divide-border border-b border-border">
      {#each entries as e (e.key)}
        {@const c = e.c}
        {#if e.repo}
          <li class="truncate pt-2 pr-4 pb-1 pl-10 text-[12px] text-muted-foreground" title={e.repo}>{leaf(e.repo)}</li>
        {:else if e.more}
          <li class="flex min-h-9 flex-wrap items-center gap-x-4 pr-4 pl-10 text-[13px]">
            <span class="text-muted-foreground tabular-nums">{t('list.partial', { shown: LIST_CAP, n: e.more })}</span>
            <Button variant="ghost" size="sm" onclick={() => (showAll[c.id] = true)}>{t('list.more', { n: e.more - LIST_CAP })}</Button>
          </li>
        {:else if e.f}
          {@const p = e.f.path}
          <li>
            <ItemRow depth={1} name={c.kind === 'project' ? leaf(parentOf(p)) + '\\' + leaf(p) : leaf(p)} title={p} size={e.f.size} max={nestedRef[c.id]} note={age(e.f.modified)}
              checked={isPicked(c.id, p)} onToggle={(on) => toggleItem(c, p, on)} focused={focusKey === e.key} tabindex={tabKey === e.key ? 0 : -1} onFocus={() => focusRow(c.id, p)} />
          </li>
        {:else if e.w}
          {@const w = e.w}
          <li>
            <ItemRow depth={1} name={w.name} title={w.path} size={w.size} max={nestedRef[c.id]} note={removable(w) ? age(w.modified) : t('wt.short.' + w.protection[0])}
              checked={isPicked(c.id, w.path)} onToggle={removable(w) ? (on) => toggleItem(c, w.path, on) : undefined} warn={!removable(w)} disabled={work.wt.loading}
              focused={focusKey === e.key} tabindex={tabKey === e.key ? 0 : -1} onFocus={() => focusRow(c.id, w.path)} />
          </li>
        {:else}
          {@const running = work.progress?.id === c.id}
          {@const [key, isLevel] = status(c)}
          {@const box = boxState(c)}
          {@const done = work.cleaned[c.id]}
          {@const total = c.kind === 'worktrees' && wtList ? t('group.wtTotal', { name: name(c), size: formatBytes(wtTotal, app.lang) }) : undefined}
          <li class="relative {empty(c) ? 'text-muted-foreground' : ''}">
            {#if nested(c)}
              {@const isOpen = opened(c)}
              <!-- Para el raton; con el teclado, Enter sobre la fila, que es la que entra en el orden de Tab (foco itinerante). -->
              <button type="button" tabindex="-1" aria-expanded={isOpen} aria-label={t(isOpen ? 'group.collapse' : 'group.expand', { name: name(c) })}
                class="absolute inset-y-0 left-0 z-[1] grid w-4 place-items-center text-muted-foreground hover:text-foreground" onclick={() => (open[c.id] = !isOpen)}>
                <ChevronRight size={14} class="transition-transform {isOpen ? 'rotate-90' : ''}" aria-hidden="true" />
              </button>
            {/if}
            <ItemRow name={name(c)} title={total ?? name(c)} description={total} size={c.kind === 'worktrees' && !wtList ? 0 : sizeOf(c)} max={biggestRow}
              level={!running && !empty(c) && isLevel ? impactOf(c.id) : undefined}
              note={running ? t('clean.running') : empty(c) ? t('group.nothing') : t(key)}
              checked={box.checked} indeterminate={box.indeterminate} onToggle={hasBox(c) ? (on) => toggleBox(c, on) : undefined}
              disabled={c.kind === 'worktrees' && work.wt.loading} warn={!running && key === 'status.protected'} expanded={nested(c) ? opened(c) : undefined}
              cleaned={done && !running && !cleanable(c) ? done : undefined}
              focused={focusKey === e.key} tabindex={tabKey === e.key ? 0 : -1} onFocus={() => focusRow(c.id, null)} />
          </li>
        {/if}
      {/each}
    </ul>
  {/if}
</section>
