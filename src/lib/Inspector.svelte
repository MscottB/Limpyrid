<script lang="ts" module>
  // App.svelte lo lee: con el informe completo abierto, el centro muestra ReportPanel en lugar de la lista.
  export const report = $state({ full: false });
</script>

<script lang="ts">
  // Panel de detalle: el informe de la ultima accion que borra, si lo hay; y debajo la fila enfocada del grupo, el
  // resumen del grupo, en Inicio la leyenda de los niveles y en Carpetas y Mapa el elemento elegido (app.picked).
  import type { Snippet } from 'svelte';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import FolderSearch from '@lucide/svelte/icons/folder-search';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as api from './api';
  import type { CategoryInfo, Engine } from './api';
  import { app, locked, work, type Cleaned } from './state.svelte';
  import { t } from './i18n';
  import { formatAge, formatBytes, formatShare } from './format';
  import { validFocus } from './nav';
  import { IMPACT, impactOf, sentences, type Impact } from './impact';
  import { driveOf, groupTotals } from './recovery';
  import * as actions from './actions.svelte';
  import { currentPlan } from './selection.svelte';
  import ReportPanel from './ReportPanel.svelte';
  import ContainerPanel from './ContainerPanel.svelte';

  const fmt = (n: number) => formatBytes(n, app.lang);
  const days = (hours: number) => t(hours === 24 ? 'clean.days.one' : 'clean.days.other', { n: hours / 24 });
  const when = (s: number | null) => (s == null ? t('clean.noActivity') : formatAge(s, Date.now(), app.lang, t('age.today')));
  const LEVELS: Impact[] = ['auto', 'redownload', 'rebuild', 'careful'];

  const cats = $derived(work.cats ?? []);
  const wtList = $derived(work.wt.list);
  const focus = $derived(app.view === 'group' ? validFocus(app.focus, app.group, cats, wtList?.worktrees ?? []) : null);
  const c = $derived(focus && cats.find((x) => x.id === focus.id));
  const item = $derived(focus?.path ? c?.items.find((i) => i.path === focus.path) : undefined);
  const wt = $derived(focus?.path && c?.kind === 'worktrees' ? wtList?.worktrees.find((w) => w.path === focus.path) : undefined);

  // Fila enfocada de una categoria limpiada que el backend ya no lista (validFocus la descarta: no esta en los datos).
  const gone = $derived.by(() => {
    const f = app.view === 'group' && !focus ? app.focus : null;
    const r = f && f.path === null && !cats.some((x) => x.id === f.id) ? work.cleaned[f.id] : undefined;
    return f && r && r.ecosystem === app.group ? { id: f.id, r } : null;
  });

  // Resumen del grupo sin fila enfocada: cuantas categorias, cuanto limpiable y cuanto marcado.
  const mine = $derived(cats.filter((x) => x.ecosystem === app.group));
  const cleanable = $derived(groupTotals(cats, wtList?.worktrees ?? []).find((g) => g.ecosystem === app.group)?.bytes ?? 0);
  const marked = $derived(currentPlan().filter((e) => mine.some((x) => x.id === e.id)).reduce((a, e) => a + e.size, 0));

  // "Libero X · quedan Y", y en las de elementos cuantos se quitaron; los omitidos tienen sus motivos en el informe.
  function cleanedLine(c: CategoryInfo, r: Cleaned, left: number): string {
    // Lo enviado a la Papelera no se cuenta como liberado.
    const parts = [r.trashed && !r.freed ? t('cleaned.summaryTrash', { size: fmt(r.trashed), left: fmt(left) }) : t('cleaned.summary', { freed: fmt(r.freed), left: fmt(left) })];
    if (r.trashed && r.freed) parts.push(t('cleaned.trashed', { size: fmt(r.trashed) }));
    const present = c.kind === 'worktrees' ? wtList?.worktrees.map((w) => w.path) : c.items.map((i) => i.path);
    if (c.kind !== 'contents' && present) {
      const n = r.paths.filter((p) => !present.includes(p)).length;
      parts.push(t(n === 1 ? 'cleaned.items.one' : 'cleaned.items.other', { n }));
    }
    if (r.skipped) parts.push(t(r.skipped === 1 ? 'cleaned.skipped.one' : 'cleaned.skipped.other', { n: r.skipped.toLocaleString(app.lang) }));
    return parts.join(' · ');
  }

  // Parte del disco del elemento elegido en Carpetas o Mapa: sobre la capacidad de la unidad escaneada.
  const drive = $derived(app.scan.root ? driveOf(app.scan.root, work.drives ?? []) : undefined);

  const o = $derived(work.outcome);
  // Con `op`, solo lo borrado (`delete`) o lo enviado a la Papelera (`trash`), que no es espacio liberado.
  const sum = (f: (r: api.Report) => number, op?: 'delete' | 'trash') =>
    o?.kind === 'batch' ? o.results.filter((r) => !op || r.op === op).reduce((a, r) => a + f(r.report), 0) : 0;
</script>

<div class="flex flex-col gap-5 p-4 text-[13px]">
  {#if o?.kind === 'batch'}
    <section aria-labelledby="det-report" class="flex flex-col gap-2 border-b border-border pb-4" aria-live="polite">
      <h2 id="det-report" class="text-[14px] font-medium">{t(o.error ? 'batch.titleStopped' : 'report.title')}</h2>
      {#if o.error}<p class="text-destructive" role="alert">{t('batch.stopped', { name: t('cat.' + o.error.id) })} {t('err.' + o.error.code)}</p>{/if}
      <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5">
        <dt class="text-muted-foreground">{t('report.freed')}</dt><dd class="num font-medium text-success">{fmt(sum((r) => r.freed, 'delete'))}</dd>
        {#if o.results.some((r) => r.op === 'trash')}<dt class="text-muted-foreground">{t('report.trashed')}</dt><dd class="num font-medium">{fmt(sum((r) => r.freed, 'trash'))}</dd>{/if}
        <dt class="text-muted-foreground">{t(o.results.some((r) => r.op === 'trash') ? 'batch.items' : 'report.deleted')}</dt><dd class="num">{sum((r) => r.deleted).toLocaleString(app.lang)}</dd>
        <dt class="text-muted-foreground">{t('batch.skipped')}</dt><dd class="num">{sum((r) => r.skipped.length).toLocaleString(app.lang)}</dd>
      </dl>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" onclick={() => (report.full = true)}>{t('report.full')}</Button>
        <Button variant="ghost" size="sm" onclick={() => (work.outcome = null)}>{t('report.dismiss')}</Button>
      </div>
    </section>
  {:else if o}
    <ReportPanel outcome={o} ondismiss={() => (work.outcome = null)} />
  {/if}

  {#if app.view === 'explore' || app.view === 'map'}
    {@const e = app.picked}
    {#if e}
      <div class="flex flex-col gap-1">
        <h2 class="text-[14px] font-medium [overflow-wrap:anywhere]">{e.name}</h2>
        <p class="text-muted-foreground">{t(e.is_dir ? 'explore.kind.dir' : 'explore.kind.file')}</p>
        <p class="font-mono text-[12px] text-muted-foreground [overflow-wrap:anywhere]">{e.path}</p>
      </div>
      <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5">
        <dt class="text-muted-foreground">{t('explore.size')}</dt><dd class="num">{fmt(e.size)}</dd>
        {#if e.is_dir}<dt class="text-muted-foreground">{t('explore.files')}</dt><dd class="num">{e.files.toLocaleString(app.lang)}</dd>{/if}
        {#if drive?.total}<dt class="text-muted-foreground">{t('detail.diskShare')}</dt><dd class="num">{formatShare(e.size, drive.total, app.lang)}</dd>{/if}
      </dl>
      <!-- Las etiquetas largas (y las de otros idiomas) parten linea dentro de los 300 px. -->
      <div class="flex flex-col gap-2 [&>button]:h-auto [&>button]:min-h-8 [&>button]:justify-start [&>button]:py-1.5 [&>button]:text-left [&>button]:whitespace-normal">
        <Button variant="outline" size="sm" onclick={() => actions.reveal(e.path)}><FolderSearch aria-hidden="true" />{t('act.reveal')}</Button>
        <Button variant="outline" size="sm" disabled={locked()} onclick={() => actions.trash(e.path, e.size)} class="hover:text-destructive"><Trash2 aria-hidden="true" />{t('act.trash')}</Button>
      </div>
    {:else}
      <p class="text-muted-foreground">{t('detail.pickHint')}</p>
    {/if}
  {:else if app.view === 'home'}
    {#if !o}
      <section aria-labelledby="det-legend" class="flex flex-col gap-3">
        <h2 id="det-legend" class="text-[12px] text-muted-foreground">{t('detail.legend')}</h2>
        {#each LEVELS as l (l)}
          <div class="flex flex-col gap-1">
            <Badge variant={l === 'careful' ? 'warning' : 'secondary'}>{t('impact.' + l)}</Badge>
            <p>{t('impact.' + l + '.note')}</p>
          </div>
        {/each}
      </section>
    {/if}
  {:else if c}
    <!-- Como en la fila: una carpeta de proyecto se nombra con su proyecto (todas se llaman node_modules, target...). -->
    {@const name = wt ? wt.name : item ? item.path.split(/[\\/]/).filter(Boolean).slice(c.kind === 'project' ? -2 : -1).join('\\') : t('cat.' + c.id)}
    <div class="flex flex-col gap-1">
      <h2 class="text-[14px] font-medium [overflow-wrap:anywhere]">{name}</h2>
      <p class="text-muted-foreground">{wt ? t('wt.kind.' + wt.kind) + ' · ' + (wt.branch ?? t('wt.noBranch')) : t('cat.' + c.id + '.hint')}</p>
    </div>
    {#if work.cleaned[c.id] && !focus?.path}
      {@render block(t('cleaned.badge'), cleanedText)}
    {/if}
    <!-- Docker y Podman: el uso y las acciones antes de los parrafos largos de "Si lo limpias", sin desplazar. -->
    {#if c.kind === 'containers'}<ContainerPanel engine={c.id as Engine} />{/if}
    {#if c.id in IMPACT}
      {@render block(t('detail.ifClean'), effect)}
    {/if}
    {#if wt}
      {@render block(t('detail.where'), wtWhere)}
      {@render block(t('detail.lastUse'), lastUse)}
      {#if wt.protection.length}{@render block(t('detail.protected'), protectedBy)}{/if}
    {:else if item}
      {@render block(t('detail.where'), itemWhere)}
      {@render block(t('detail.lastUse'), lastUse)}
    {:else if c.kind === 'worktrees'}
      {@render wtCategory()}
    {:else if c.paths.length && c.kind !== 'containers'}
      {@render block(t('detail.where'), paths)}
    {/if}
    {#if c.kind === 'project' && !item}<p class="text-muted-foreground">{t('project.staleNote')}</p>{/if}
  {:else if gone}
    <!-- Limpiada en esta sesion y ya fuera de los datos: solo queda su fila; se dice lo que libero. -->
    {@const ghost: CategoryInfo = { id: gone.id, ecosystem: gone.r.ecosystem, kind: gone.r.kind, uses_min_age: false, size: 0, paths: [], items: [] }}
    <div class="flex flex-col gap-1">
      <h2 class="text-[14px] font-medium">{t('cat.' + gone.id)}</h2>
      <p class="text-muted-foreground">{t('cat.' + gone.id + '.hint')}</p>
    </div>
    <section class="flex flex-col gap-1">
      <h3 class="text-[12px] text-muted-foreground">{t('cleaned.badge')}</h3>
      <p class="tabular-nums">{cleanedLine(ghost, gone.r, 0)}</p>
    </section>
  {:else if app.group}
    <div class="flex flex-col gap-1">
      <h2 class="text-[14px] font-medium">{t('eco.' + app.group)}</h2>
      <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-0.5">
        <dt class="text-muted-foreground">{t('detail.items')}</dt><dd class="num">{mine.length}</dd>
        <dt class="text-muted-foreground">{t('detail.cleanable')}</dt><dd class="num">{t('detail.upTo', { size: fmt(cleanable) })}</dd>
        <dt class="text-muted-foreground">{t('detail.marked')}</dt><dd class="num">{t('detail.upTo', { size: fmt(marked) })}</dd>
      </dl>
      <p class="mt-3 text-[12px] text-muted-foreground">{t('keys.help')}</p>
    </div>
  {/if}
</div>

{#snippet block(title: string, body: Snippet)}
  <section class="flex flex-col gap-1">
    <h3 class="text-[12px] text-muted-foreground">{title}</h3>
    {@render body()}
  </section>
{/snippet}

{#snippet path(p: string)}
  <div class="flex items-start gap-1">
    <span class="min-w-0 flex-1 font-mono text-[12px] text-muted-foreground [overflow-wrap:anywhere]">{p}</span>
    <Button variant="ghost" size="icon-sm" aria-label={t('detail.open', { path: p })} onclick={() => actions.reveal(p)}><FolderOpen aria-hidden="true" /></Button>
  </div>
{/snippet}

{#snippet cleanedText()}
  {#if c}<p class="tabular-nums">{cleanedLine(c, work.cleaned[c.id], c.kind === 'worktrees' ? (wtList?.worktrees ?? []).reduce((a, w) => a + w.size, 0) : c.size)}</p>{/if}
{/snippet}

{#snippet effect()}
  {#if c}
    {@const level = impactOf(c.id)}
    <Badge variant={level === 'careful' ? 'warning' : 'secondary'}>{t('impact.' + level)}</Badge>
    {#each sentences(t('cat.' + c.id + '.effect')) as s}<p>{s}</p>{/each}
    {#if c.uses_min_age && c.kind !== 'worktrees'}<p class="text-muted-foreground">{t('clean.ageNote', { days: days(work.minAge) })}</p>{/if}
  {/if}
{/snippet}

{#snippet paths()}
  {#if c}{#each c.paths as p (p)}{@render path(p)}{/each}{/if}
{/snippet}

{#snippet itemWhere()}
  {#if item}{@render path(item.path)}{/if}
{/snippet}

{#snippet wtWhere()}
  {#if wt}{@render path(wt.path)}<p class="text-muted-foreground [overflow-wrap:anywhere]">{wt.repo}</p>{/if}
{/snippet}

{#snippet lastUse()}
  <p>{when((wt ?? item)?.modified ?? null)}</p>
{/snippet}

{#snippet protectedBy()}
  {#if wt}<ul class="flex list-disc flex-col gap-1 pl-4">{#each wt.protection as p (p)}<li>{t('wt.protection.' + p)}</li>{/each}</ul>{/if}
{/snippet}

{#snippet wtCategory()}
  {#if !wtList}
    <p class="text-muted-foreground" role="status">{t('wt.loading')}</p>
  {:else if !wtList.git}
    <p>{t('wt.noGit')}</p>
  {:else}
    <p class="text-muted-foreground">{t('wt.ageNote', { days: days(work.minAge) })}</p>
    {#if !wtList.worktrees.length}<p>{t('wt.empty')}</p>{/if}
    {#if wtList.orphans.length}
      <h4 class="mt-2 font-medium">{t('wt.orphans')}</h4>
      <p class="text-muted-foreground">{t('wt.orphansNote')}</p>
      {#each wtList.orphans as o (o.path)}
        {@render path(o.path)}
        <p class="num -mt-1 text-left text-muted-foreground">{fmt(o.size)}</p>
      {/each}
    {/if}
  {/if}
{/snippet}
