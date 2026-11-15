<script lang="ts">
  // Mapa: treemap en lienzo de la carpeta actual. Area = tamano; color = profundidad (rampa de un solo tono); lo agrupado
  // en un neutro. El teclado y los lectores de pantalla usan la lista (Explorador), en la misma carpeta.
  import { untrack } from 'svelte';
  import List from '@lucide/svelte/icons/list';
  import ScanSearch from '@lucide/svelte/icons/scan-search';
  import { Button } from '$lib/components/ui/button';
  import * as api from '../api';
  import type { TreeNode } from '../api';
  import { app, dataRev, openDetail } from '../state.svelte';
  import { t } from '../i18n';
  import { formatBytes, formatShare } from '../format';
  import { HEADER, hit, isInside, isSmall, labelInk, layout, parentPath, type Rect } from '../treemap';
  import Breadcrumb from '../Breadcrumb.svelte';
  import ViewState from '../ViewState.svelte';

  let { onscan }: { onscan: () => void } = $props();

  const MIN_LABEL_W = 60;
  const MIN_LABEL_H = 28;

  let tree = $state.raw<TreeNode | null>(null); // raw: sin proxies, la identidad de los objetos permite saltarse redibujos
  let treePath = $state(''); // carpeta a la que pertenece `tree`: app.cwd puede cambiar mientras carga el siguiente
  let error = $state<string | null>(null);
  let loading = $state(false);
  let loadToken = 0;

  let wrap = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let size = $state({ w: 0, h: 0 });
  let hovered = $state.raw<Rect | null>(null);
  let pointer = $state({ x: 0, y: 0 });
  let tipEl = $state<HTMLDivElement>();

  const root = $derived(app.scan.root || app.cwd);
  const locked = $derived(app.scan.status === 'running');
  // Una vez por cambio de arbol o de tamano; el puntero no la recalcula.
  const rects = $derived(tree && size.w > 0 && size.h > 0 ? layout(tree, treePath, size.w, size.h) : []);
  const fmt = (n: number) => formatBytes(n, app.lang);
  const count = (n: number) => n.toLocaleString(app.lang);
  const shown = (r: Rect) => (isSmall(r.name) ? `${parentPath(r.path)} (${t('node.small')})` : r.path);

  async function load() {
    const token = ++loadToken;
    loading = true;
    const path = app.cwd;
    try {
      const next = await api.treemap(path);
      if (token !== loadToken) return;
      tree = next;
      treePath = path;
      error = null;
    } catch (e) {
      if (token !== loadToken) return;
      tree = null;
      error = api.errCode(e);
    } finally {
      if (token === loadToken) loading = false;
    }
  }

  $effect(() => {
    void app.cwd;
    void dataRev.n;
    untrack(load);
  });

  // Con otra disposicion el rectangulo senalado ya no existe.
  $effect(() => {
    void rects;
    hovered = null;
  });

  // Un observador por montaje: se suelta al desmontar o cuando el lienzo desaparece.
  $effect(() => {
    if (!wrap) return;
    const ro = new ResizeObserver(([e]) => (size = { w: Math.floor(e.contentRect.width), h: Math.floor(e.contentRect.height) }));
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  function fit(ctx: CanvasRenderingContext2D, text: string, max: number): string {
    if (ctx.measureText(text).width <= max) return text;
    let s = text;
    while (s.length > 1 && ctx.measureText(s + '…').width > max) s = s.slice(0, -1);
    return s + '…';
  }

  // Dibujo plano: sin degradados, sombras ni relieve. Sin animacion.
  $effect(() => {
    const list = rects;
    const hot = hovered;
    const picked = app.picked?.path;
    const lang = app.lang;
    const { w, h } = size;
    if (!canvas || w <= 0 || h <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const css = getComputedStyle(document.documentElement);
    const token = (n: string) => css.getPropertyValue(n).trim();
    const surface = token('--surface');
    const ramp = [token('--depth-1'), token('--depth-2'), token('--depth-3')];
    const neutral = token('--chart-free');
    const family = getComputedStyle(document.body).fontFamily;
    const fillOf = (r: Rect) => (isSmall(r.name) ? neutral : ramp[Math.min(r.depth, 3) - 1]);

    ctx.fillStyle = surface;
    ctx.fillRect(0, 0, w, h);
    const parents = new Set(list.map((r) => parentPath(r.path)));
    ctx.lineWidth = 2;
    ctx.strokeStyle = surface;
    for (const r of list) {
      ctx.fillStyle = fillOf(r);
      ctx.fillRect(r.x, r.y, r.w, r.h);
      ctx.strokeRect(r.x, r.y, r.w, r.h); // 1 px a cada lado: 2 px de superficie entre vecinos
    }
    // Etiquetas despues de todos los rellenos, solo en rectangulos de 60x28 o mas, recortadas a su rectangulo. Una
    // carpeta con hijos lleva nombre y tamano en una linea en su franja superior; las demas, en dos lineas.
    for (const r of list) {
      if (r.w < MIN_LABEL_W || r.h < MIN_LABEL_H) continue;
      const small = isSmall(r.name);
      const name = small ? t('node.small') : r.name;
      const max = r.w - 12;
      ctx.save();
      ctx.beginPath();
      ctx.rect(r.x + 1, r.y + 1, r.w - 2, parents.has(r.path) ? HEADER - 1 : r.h - 2);
      ctx.clip();
      ctx.fillStyle = small ? token('--text') : labelInk(fillOf(r));
      ctx.font = `600 12px ${family}`;
      if (parents.has(r.path)) {
        ctx.fillText(fit(ctx, `${name} · ${formatBytes(r.size, lang)}`, max), r.x + 6, r.y + 14);
      } else {
        ctx.fillText(fit(ctx, name, max), r.x + 6, r.y + 15);
        ctx.font = `12px ${family}`;
        ctx.fillText(fit(ctx, formatBytes(r.size, lang), max), r.x + 6, r.y + 29);
      }
      ctx.restore();
    }
    // El elegido (en cian) y el senalado; linea interior de la superficie: el contorno se separa de cualquier relleno.
    const outline = (r: Rect, color: string) => {
      ctx.lineWidth = 2;
      ctx.strokeStyle = color;
      ctx.strokeRect(r.x + 1, r.y + 1, r.w - 2, r.h - 2);
      ctx.lineWidth = 1;
      ctx.strokeStyle = surface;
      ctx.strokeRect(r.x + 2.5, r.y + 2.5, r.w - 5, r.h - 5);
    };
    const sel = list.find((r) => r.path === picked);
    if (sel) outline(sel, token('--primary'));
    if (hot) outline(hot, token('--focus'));
  });

  // El tooltip sigue al puntero sin salirse de la vista.
  $effect(() => {
    if (!tipEl || !hovered) return;
    void pointer;
    const { offsetWidth: tw, offsetHeight: th } = tipEl;
    tipEl.style.left = `${Math.max(4, pointer.x + 14 + tw > size.w ? pointer.x - tw - 10 : pointer.x + 14)}px`;
    tipEl.style.top = `${Math.max(4, pointer.y + 14 + th > size.h ? pointer.y - th - 10 : pointer.y + 14)}px`;
  });

  function local(e: MouseEvent) {
    const b = canvas!.getBoundingClientRect();
    return { x: e.clientX - b.left, y: e.clientY - b.top };
  }
  function onmove(e: PointerEvent) {
    pointer = local(e);
    const r = hit(rects, pointer.x, pointer.y) ?? null;
    if (r !== hovered) hovered = r; // solo cambia al pasar a otro rectangulo: no se redibuja en cada movimiento
  }
  // Lo agrupado no tiene ruta: no se elige. Al cerrar el detalle superpuesto el foco vuelve al lienzo.
  function pick(r: Rect) {
    if (isSmall(r.name)) return;
    app.picked = { name: r.name, path: r.path, size: r.size, files: r.files, is_dir: r.isDir };
    canvas?.focus();
    openDetail(canvas ?? null);
  }
  // Una carpeta entra (un <small> entra en la suya) y un archivo se elige; el clic derecho elige cualquiera.
  function onclick(e: MouseEvent) {
    const p = local(e);
    const r = hit(rects, p.x, p.y);
    if (!r) return;
    if (!r.isDir && !isSmall(r.name)) return pick(r);
    const target = r.isDir ? r.path : parentPath(r.path);
    if (isInside(target, app.cwd) && !isInside(app.cwd, target)) app.cwd = target;
  }
  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    const p = local(e);
    const r = hit(rects, p.x, p.y);
    if (r) pick(r);
  }
</script>

<section class="flex flex-col gap-5" aria-busy={loading}>
  {#if error !== 'no_scan'}
    <!-- Una fila con linea inferior, como la cabecera de un grupo. -->
    <div class="flex min-h-12 flex-wrap items-center gap-x-3 gap-y-2 border-b border-border pb-2">
      <Breadcrumb {root} path={app.cwd} onnavigate={(p) => (app.cwd = p)} />
      {#if tree && !locked}
        <span class="text-[13px] text-muted-foreground tabular-nums">{t(tree.children.length === 1 ? 'explore.summary.one' : 'explore.summary.other', { size: fmt(tree.size), n: count(tree.children.length) })}</span>
      {/if}
      <span class="flex-1"></span>
      <Button variant="outline" size="sm" onclick={() => (app.view = 'explore')}><List aria-hidden="true" />{t('map.asList')}</Button>
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
  {:else if tree && !tree.children.length}
    <ViewState kind="empty" text={t('view.empty')} />
  {:else if tree}
    <div class="relative h-[clamp(320px,calc(100vh-380px),720px)] overflow-hidden rounded-xl border border-border bg-surface" bind:this={wrap}>
      <!-- El lienzo es solo una representacion visual: el teclado y los lectores de pantalla usan la lista (boton "Ver como lista"). -->
      <!-- svelte-ignore a11y_no_interactive_element_to_noninteractive_role, a11y_no_noninteractive_element_interactions -->
      <canvas bind:this={canvas} tabindex="-1" role="img" aria-label={t('map.label', { name: treePath, size: fmt(tree.size) })} class="block size-full cursor-pointer outline-none" onpointermove={onmove} onpointerleave={() => (hovered = null)} {onclick} {oncontextmenu}></canvas>
      {#if hovered}
        <div class="pointer-events-none absolute z-10 max-w-[min(480px,90%)] rounded-lg border border-input bg-raised px-3 py-2 text-[13px]" bind:this={tipEl}>
          <p class="font-medium [overflow-wrap:anywhere]">{shown(hovered)}</p>
          <p class="mt-0.5 text-muted-foreground tabular-nums">{t('map.tip', { size: fmt(hovered.size), files: count(hovered.files), share: formatShare(hovered.size, tree.size, app.lang) })}</p>
        </div>
      {/if}
    </div>
    <div class="flex flex-wrap items-center gap-x-5 gap-y-2 text-[13px]">
      <span class="text-muted-foreground">{t('map.legend')}</span>
      {#each [1, 2, 3] as n (n)}
        <span class="inline-flex items-center gap-2"><span class="h-3 w-4 rounded-sm" style:background="var(--depth-{n})" aria-hidden="true"></span>{t('map.depth.' + n)}</span>
      {/each}
      <span class="inline-flex items-center gap-2"><span class="h-3 w-4 rounded-sm bg-chart-free" aria-hidden="true"></span>{t('node.small')}</span>
    </div>
    <p class="-mt-3 text-[13px] text-muted-foreground">{t('map.hint')}</p>
  {:else if loading}
    <ViewState kind="busy" text={t('view.loading')} />
  {/if}
</section>

