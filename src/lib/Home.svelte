<script lang="ts">
  // Inicio: sin escaneo, que escanear; escaneando, el progreso; con escaneo, el disco, lo recuperable por nivel de
  // consecuencia y los grupos mayores.
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { app, locked, openGroup, scanCheck, work } from './state.svelte';
  import { currentPlan, selection, setCategory } from './selection.svelte';
  import { TOP_GROUPS, bytesOn, driveOf, freeing, groupTotals, recovery, topGroups } from './recovery';
  import { levelGroup, levelHasItems, levelPick, levelTotals } from './levels';
  import { scanPercent } from './nav';
  import type { Impact } from './impact';
  import { t } from './i18n';
  import { formatBytes } from './format';
  import DrivePicker from './DrivePicker.svelte';

  let { onscan, error, cancelled, denied }: { onscan: () => void; error: string | null; cancelled: boolean; denied: string[] } = $props();

  const fmt = (n: number) => formatBytes(n, app.lang);
  const running = $derived(app.scan.status === 'running');
  const done = $derived(app.scan.status === 'done');
  const progress = $derived(app.scan.progress);
  const check = $derived(scanCheck());

  const cats = $derived(work.cats ?? []);
  const wts = $derived(work.wt.list?.worktrees ?? []);
  const drive = $derived(app.scan.root ? driveOf(app.scan.root, work.drives ?? []) : undefined);
  const scanPct = $derived(scanPercent(progress?.bytes ?? 0, drive, app.scan.root));
  const driveName = $derived(drive ? drive.root.replace(/\\$/, '') : '');
  const r = $derived(drive ? recovery(drive, bytesOn(freeing(currentPlan(), work.toTrash), cats, drive.root)) : null);
  const pct = (n: number) => (drive && drive.total ? (n / drive.total) * 100 : 0);

  const LEVELS: Impact[] = ['auto', 'redownload', 'rebuild', 'careful'];
  const totals = $derived(levelTotals(cats, wts));
  // Casilla de nivel: solo las caches enteras de ese nivel (levelPick); marcada si estan todas, a medias si algunas.
  function box(level: 'auto' | 'redownload') {
    const ids = levelPick(level, cats);
    const n = ids.filter((id) => selection.categories.has(id)).length;
    return { ids, checked: ids.length > 0 && n === ids.length, indeterminate: n > 0 && n < ids.length };
  }

  const groups = $derived(groupTotals(cats, wts));
  const bars = $derived(topGroups(groups, TOP_GROUPS));
  // Escala solo con los grupos nombrados: "Otros" no reescala las barras.
  const biggest = $derived(Math.max(1, ...bars.filter((g) => g.ecosystem).map((g) => g.bytes)));
  const BAR_ROW = 'grid h-9 w-full grid-cols-[minmax(88px,150px)_minmax(0,1fr)_80px] items-center gap-3 rounded-md px-2 text-left';
</script>

<div class="flex flex-col gap-6 px-6 pt-6 pb-10">
  <h1 class="text-[18px] leading-tight font-semibold">{t('nav.home')}</h1>

  {#if running}
    <section aria-labelledby="scan-title" class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center gap-3">
        <h2 id="scan-title" class="min-w-0 flex-1 font-medium [overflow-wrap:anywhere]">{t('home.running', { root: app.scan.root })}</h2>
        <Button variant="outline" onclick={onscan}>{t('scan.cancel')}</Button>
      </div>
      <!-- Solo con la unidad entera se conoce el total: una carpeta suelta muestra solo los contadores. -->
      {#if scanPct !== null}
        <div class="h-1.5 overflow-hidden rounded-full bg-raised" role="progressbar" aria-label={t('scan.progressBar')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={scanPct}>
          <div class="h-full rounded-full bg-chart-used" style:width="{scanPct}%"></div>
        </div>
      {/if}
      <p class="text-[13px] text-muted-foreground tabular-nums">
        {#if scanPct !== null}{t('scan.percent', { n: scanPct })} ·{/if}
        {progress ? t('scan.progress', { dirs: progress.dirs.toLocaleString(app.lang), size: formatBytes(progress.bytes, app.lang) }) : t('scan.running')}
        · {t('home.runningNote')}
      </p>
      <span class="sr-only" role="status">{t('scan.running')}</span>
    </section>
  {:else if !app.scan.root}
    <p class="-mt-4 max-w-[72ch] text-muted-foreground">{t('home.lead')}</p>
    <DrivePicker disabled={false} />
    <div class="flex flex-wrap items-center gap-4">
      <div class="flex min-w-48 flex-1 flex-col gap-0.5">
        <p class="text-muted-foreground [overflow-wrap:anywhere]">
          {#if check.target}{t(check.rule === 'no' ? 'home.notScanned' : 'home.target', { root: check.target })}{:else}{t('home.noTarget')}{/if}
        </p>
        <!-- "Otra carpeta" sigue la regla de su unidad: el motivo se dice en texto. -->
        {#if check.reason}<p class="text-[13px] {check.rule === 'no' ? 'font-medium text-destructive' : 'text-warning'}">{t(check.reason)}</p>{/if}
        {#if work.busy}<p class="text-[13px] text-muted-foreground">{t('home.waitClean')}</p>{/if}
      </div>
      <Button size="lg" disabled={!check.ok} onclick={onscan}>{t('scan.start')}</Button>
    </div>
    {@render status()}
  {:else}
    <section aria-labelledby="disk-title" class="flex flex-col gap-2">
      <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h2 id="disk-title" class="font-medium">{drive ? t('nav.disk', { drive: `${driveName} ${drive.label}`.trim() }) : app.scan.root}</h2>
        {#if drive && drive.total > 0}<p class="text-[13px] text-muted-foreground tabular-nums">{t('drive.free', { free: fmt(drive.free) })} {t('drive.of', { total: fmt(drive.total) })}</p>{/if}
      </div>
      {#if drive && drive.total > 0 && r}
        <span role="img" aria-label={t('disk.bar', { drive: driveName, used: fmt(drive.total - drive.free), total: fmt(drive.total) })} class="flex h-2 overflow-hidden rounded-full bg-chart-free">
          <span class="h-full bg-chart-used" style:width="{pct(r.usedAfter)}%"></span>
          <span class="h-full bg-success" style:width="{pct(r.recover)}%"></span>
        </span>
      {:else if work.drives}
        <p class="text-[13px] text-muted-foreground">{t('recovery.noDrive')}</p>
      {/if}
      {@render status()}
    </section>

    <section aria-labelledby="levels-title" class="flex flex-col gap-1">
      <div class="flex items-baseline justify-between gap-4">
        <h2 id="levels-title" class="font-medium">{t('home.recover')}</h2>
        {#if work.catsLoading}<span class="text-[13px] text-muted-foreground" role="status">{t('recovery.loading')}</span>{/if}
        <span class="ml-auto px-2 font-medium tabular-nums">{fmt(LEVELS.reduce((a, l) => a + totals[l], 0))}</span>
      </div>
      <ul class="divide-y divide-border border-y border-border">
        {#each LEVELS as level (level)}
          {@const b = level === 'auto' || level === 'redownload' ? box(level) : null}
          {@const goTo = levelGroup(level, cats, wts)}
          {@const more = b && levelHasItems(level, cats)}
          <li class="flex min-h-9 items-center gap-3 px-2 py-1">
            {#if b}
              <Checkbox
                id={'level-' + level}
                disabled={locked() || !b.ids.length}
                bind:checked={() => b.checked, (on) => b.ids.forEach((id) => setCategory(id, on))}
                indeterminate={b.indeterminate}
              />
              <label for={'level-' + level} class="min-w-0 flex-1">{t('impact.' + level)}</label>
            {:else}
              <span class="w-4 shrink-0" aria-hidden="true"></span>
              <span class="min-w-0 flex-1">{t('impact.' + level)}</span>
            {/if}
            {#if more}<span class="text-[12px] text-muted-foreground">{t('home.oneByOne')}</span>{/if}
            {#if goTo && (!b || more)}
              <Button variant="ghost" size="sm" aria-label={t('home.reviewLabel', { level: t('impact.' + level) })} onclick={() => openGroup(goTo)}>{t('home.review')}</Button>
            {/if}
            <span class="w-20 shrink-0 text-right tabular-nums">{fmt(totals[level])}</span>
          </li>
        {/each}
      </ul>
    </section>

    {#if bars.length}
      <section aria-labelledby="groups-title" class="flex flex-col gap-1">
        <h2 id="groups-title" class="font-medium">{t('clean.overview.groups')}</h2>
        <ul class="flex flex-col">
          {#each bars as g (g.ecosystem ?? '')}
            <li>
              {#if g.ecosystem}
                {@const eco = g.ecosystem}
                {@const name = t('eco.' + eco)}
                <button type="button" class="{BAR_ROW} transition-colors hover:bg-raised" aria-label={t('clean.overview.go', { name, size: fmt(g.bytes) })} onclick={() => openGroup(eco)}>
                  {@render bar(name, g.bytes)}
                </button>
              {:else}
                {@const rest = groups.length - TOP_GROUPS}
                <div class={BAR_ROW} role="img" aria-label={t('clean.overview.othersLabel', { n: rest, size: fmt(g.bytes) })}>
                  {@render bar(t('clean.overview.others', { n: rest }), g.bytes)}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
</div>

<!-- Lo que informa el ultimo escaneo: error, cancelado o listo, y las carpetas sin permiso. -->
{#snippet status()}
  <div class="flex flex-wrap items-center gap-x-3 gap-y-2 empty:hidden" role="status" aria-live="polite">
    {#if error}
      <p class="font-medium text-destructive">{t('err.' + error)}</p>
    {:else if cancelled}
      <Badge variant="secondary">{t(app.scan.root ? 'scan.cancelledKept' : 'scan.cancelled')}</Badge>
    {:else if done}
      <Badge variant="secondary">{t('scan.done')}</Badge>
      <!-- Una carpeta, no la unidad entera: se dice cual. -->
      {#if app.scan.root.replace(/\\$/, '').toLowerCase() !== driveName.toLowerCase()}<span class="text-[13px] text-muted-foreground [overflow-wrap:anywhere]">{app.scan.root}</span>{/if}
    {/if}
    <!-- La cifra va una sola vez: en la lista de carpetas sin acceso si hay lista, y si no, aqui. -->
    {#if done && app.scan.denied && !denied.length}<span class="text-[13px] text-muted-foreground tabular-nums">{t('scan.denied', { n: app.scan.denied })}</span>{/if}
  </div>
  {#if denied.length}
    <details class="text-[13px]">
      <summary class="flex min-h-9 cursor-pointer items-center text-muted-foreground hover:text-foreground">{t('scan.deniedList', { n: denied.length })}</summary>
      <ul class="mt-1 max-h-48 overflow-auto rounded-md border border-border bg-surface px-3 py-2 [overflow-wrap:anywhere]">
        {#each denied as p (p)}<li class="py-0.5">{p}</li>{/each}
      </ul>
    </details>
  {/if}
{/snippet}

<!-- El nombre y la cifra van en texto; la barra solo muestra la proporcion (el nombre accesible esta en la fila). -->
{#snippet bar(label: string, bytes: number)}
  <span class="truncate" aria-hidden="true">{label}</span>
  <span class="block h-1.5 overflow-hidden rounded-full bg-chart-free" aria-hidden="true">
    <span class="block h-full min-w-[3px] rounded-full bg-chart-used" style:width="{Math.min(100, (bytes / biggest) * 100)}%"></span>
  </span>
  <span class="num" aria-hidden="true">{fmt(bytes)}</span>
{/snippet}
