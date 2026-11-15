<script lang="ts">
  import { tick } from 'svelte';
  import X from '@lucide/svelte/icons/x';
  import { Button } from '$lib/components/ui/button';
  import { app, closeDetail, dataRev, narrow, scanCheck, scanTarget, work, type View } from './lib/state.svelte';
  import { keyAction } from './lib/keys';
  import { t } from './lib/i18n';
  import * as api from './lib/api';
  import { isInside, normalizeRoot } from './lib/treemap';
  import { barVisible, navGroups, resolveGroup } from './lib/nav';
  import GroupList from './lib/GroupList.svelte';
  import Inspector, { report } from './lib/Inspector.svelte';
  import ReportPanel from './lib/ReportPanel.svelte';
  import Explorador from './lib/views/Explorador.svelte';
  import Mapa from './lib/views/Mapa.svelte';
  import Acerca from './lib/views/Acerca.svelte';
  import NavPanel from './lib/NavPanel.svelte';
  import Home from './lib/Home.svelte';
  import TermsDialog from './lib/TermsDialog.svelte';
  import ConfirmDialog, { type ConfirmRequest } from './lib/ConfirmDialog.svelte';
  import ActionBar from './lib/ActionBar.svelte';
  import { confirm, resolveConfirm } from './lib/guard.svelte';
  import { autoCheck, checkNow } from './lib/update.svelte';

  // Una sola comprobacion de version por apertura, cuando la interfaz ya esta lista; la opcion se lee al vencer el plazo.
  $effect(() => {
    const id = setTimeout(() => {
      if (autoCheck.on) void checkNow(false);
    }, 5000);
    return () => clearTimeout(id);
  });

  let scanError = $state<string | null>(null);
  let cancelled = $state(false);
  let denied = $state<string[]>([]); // carpetas sin permiso del ultimo escaneo
  let prev = { root: '', cwd: '', denied: [] as string[], deniedN: 0 }; // estado antes de empezar el escaneo
  // Un escaneo cancelado o que no pudo empezar deja el resultado anterior tal cual.
  function restorePrev() {
    app.scan.root = prev.root;
    app.cwd = prev.cwd;
    app.scan.denied = prev.deniedN;
    denied = prev.denied;
    app.scan.status = prev.root ? 'done' : 'idle';
  }

  $effect(() => {
    document.documentElement.lang = app.lang;
    document.title = 'Limpyrid';
  });

  // Sin entorno Tauri (p. ej. vite dev en un navegador) listen() rechaza: no es un fallo de la app.
  $effect(() => {
    const offs = [
      api.onScanProgress((p) => (app.scan.progress = p)),
      api.onScanDone(async (d) => {
        cancelled = d.cancelled;
        app.scan.progress = null;
        // Un escaneo cancelado no deja resultado: el backend conserva el anterior y la interfaz vuelve a el tal cual.
        if (d.cancelled) return restorePrev();
        app.scan.status = 'done';
        app.scan.denied = d.denied;
        work.cleaned = {}; // las filas "Limpiado" duran hasta el siguiente escaneo
        denied = d.denied ? await api.deniedPaths().catch(() => []) : [];
        // Explorador y Mapa parten de la raiz escaneada si la carpeta actual quedo fuera de ella.
        if (app.scan.root && !isInside(app.cwd, app.scan.root)) app.cwd = app.scan.root;
        dataRev.n++;
      }),
    ].map((o) => o.catch(() => () => {}));
    return () => offs.forEach((o) => o.then((off) => off()));
  });

  async function toggleScan() {
    scanError = null;
    try {
      if (app.scan.status === 'running') {
        await api.cancelScan();
        return;
      }
      prev = { root: app.scan.root, cwd: app.cwd, denied, deniedN: app.scan.denied };
      cancelled = false;
      denied = [];
      app.scan.status = 'running';
      app.scan.progress = null;
      app.view = 'home'; // alli estan el progreso y Cancelar
      const root = normalizeRoot(scanTarget()); // unico punto por el que la raiz escaneada entra en el estado
      if (app.pick === null) app.scanRoot = root;
      app.scan.root = root;
      app.cwd = root;
      try {
        await api.startScan(root);
      } catch (e) {
        restorePrev();
        scanError = api.errCode(e);
      }
    } catch (e) {
      app.scan.status = 'idle';
      scanError = api.errCode(e);
    }
  }

  // Escanear (o cancelar) desde Inicio o la navegacion: una unidad extraible pide confirmar antes.
  let confirmReq = $state<ConfirmRequest | null>(null);
  function requestScan() {
    if (app.scan.status === 'running') return void toggleScan(); // cancelar
    const s = scanCheck();
    if (!s.ok) return;
    if (s.drive && s.rule === 'confirm') {
      const root = s.drive.root;
      confirmReq = {
        title: t('drive.confirmTitle', { root }),
        before: [t(s.drive.usb ? 'drive.confirmUsb' : 'drive.confirmRemovable', { root }), t('drive.confirmBody')],
        after: [],
        confirmLabel: t('drive.confirmScan', { root }),
        tone: 'default',
      };
      return;
    }
    void toggleScan();
  }
  function resolveScan(ok: boolean) {
    if (!confirmReq) return; // un segundo clic mientras se cierra otro dialogo no es una respuesta a esta confirmacion
    confirmReq = null;
    if (ok && scanCheck().ok) void toggleScan(); // la eleccion pudo cambiar mientras se confirmaba
  }

  // Un grupo que deja de existir tras un escaneo o una limpieza devuelve a Inicio.
  $effect(() => {
    if (app.view !== 'group' || !work.cats) return;
    const g = resolveGroup(app.group, navGroups(work.cats, work.wt.list?.worktrees ?? []));
    app.group = g;
    if (g === null) app.view = 'home';
  });

  // Vistas con titulo propio en el centro; Acerca de ocupa ademas la columna del detalle.
  const PAGES: Record<Exclude<View, 'home' | 'group'>, [title: string, lead: string]> = {
    explore: ['nav.explore', 'page.explore.lead'],
    map: ['nav.map', 'page.map.lead'],
    about: ['about.title', 'about.lead'],
  };

  // Lo elegido en Carpetas o Mapa es de esa carpeta y de esos datos; el detalle superpuesto, de esa vista.
  $effect(() => {
    void app.view;
    void app.cwd;
    void dataRev.n;
    app.picked = null;
  });
  // Navegar cambia el centro: tambien deja el informe completo (sigue resumido en el detalle).
  $effect(() => {
    void app.view;
    void app.group;
    app.detail = false;
    report.full = false;
  });
  // Al ensanchar la ventana el detalle vuelve a ser columna: estrecharla otra vez no lo reabre sin un gesto.
  let isNarrow = $state(narrow());
  $effect(() => {
    const mq = matchMedia('(max-width: 999px)');
    const change = () => {
      isNarrow = mq.matches;
      if (!mq.matches) app.detail = false;
    };
    mq.addEventListener('change', change);
    return () => mq.removeEventListener('change', change);
  });
  // En Carpetas y Mapa el detalle es del elemento elegido: sin el, se cierra.
  $effect(() => {
    if (app.detail && !app.picked && (app.view === 'explore' || app.view === 'map')) closeDetail();
  });
  // Al abrirse, el foco entra en el detalle por su boton de cerrar.
  let closeBtn = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    if (app.detail) tick().then(() => closeBtn?.focus());
  });
  function onkeydown(e: KeyboardEvent) {
    if (!app.detail || !narrow() || e.defaultPrevented || document.querySelector('[role="dialog"]')) return;
    const el = e.target as HTMLElement;
    const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) || el.isContentEditable;
    if (keyAction({ key: e.key, target: typing ? 'input' : 'other' }) !== 'close') return;
    e.preventDefault();
    closeDetail();
  }

  // Un informe cerrado no deja abierta su vista completa para el siguiente.
  $effect(() => {
    if (!work.outcome) report.full = false;
  });
  // Por debajo de 1000 px el detalle esta cerrado: un informe sin cerrar se abre en el centro para que no pase sin verse
  // (al llegar o al estrechar la ventana).
  $effect(() => {
    if (work.outcome && isNarrow) report.full = true;
  });

  // Cada pagina empieza arriba: el centro tiene su propio desplazamiento.
  let mainEl = $state<HTMLElement>();
  $effect(() => {
    void app.view;
    mainEl?.scrollTo(0, 0);
  });
</script>

<svelte:window {onkeydown} />

<div class="desk h-full">
  <NavPanel onscan={requestScan} />
  <main bind:this={mainEl} id="center" class="relative min-w-0 overflow-y-auto {app.view === 'about' ? 'wide' : ''}">
    {#if report.full && work.outcome}
      <!-- Por debajo de 1000 px no hay detalle donde quede resumido: Cerrar lo descarta y no reaparece al ensanchar. -->
      <div class="px-6 pt-6 pb-10"><ReportPanel outcome={work.outcome} ondismiss={() => (isNarrow ? (work.outcome = null) : (report.full = false))} /></div>
    {:else if app.view === 'home'}
      <Home onscan={requestScan} error={scanError} {cancelled} {denied} />
    {:else if app.view === 'group'}
      <!-- Cada grupo empieza con su filtro, plegados y listas largas sin tocar. -->
      {#key app.group}<GroupList />{/key}
    {:else}
      {@const page = PAGES[app.view]}
      <div class="max-w-[1360px] px-6 pt-6 pb-12">
        <header class="mb-5">
          <h1 class="text-[18px] leading-tight font-semibold">{t(page[0])}</h1>
          <p class="mt-1 max-w-[72ch] text-[13px] text-muted-foreground">{t(page[1])}</p>
        </header>
        {#if app.view === 'about'}
          <Acerca />
        {:else if app.view === 'explore'}
          <!-- Escanear lleva a Inicio, donde se elige la unidad (y se confirma si es extraible). -->
          <Explorador onscan={() => (app.view = 'home')} />
        {:else}
          <Mapa onscan={() => (app.view = 'home')} />
        {/if}
      </div>
    {/if}
  </main>
  {#if app.view !== 'about'}
    <aside class="detail min-h-0 overflow-y-auto border-l border-border bg-surface" class:open={app.detail} aria-label={t('detail.title')}>
      <div class="sheet-head flex justify-end px-2 pt-2">
        <Button variant="ghost" size="icon-sm" bind:ref={closeBtn} aria-label={t('detail.close')} title={t('detail.close')} onclick={closeDetail}><X aria-hidden="true" /></Button>
      </div>
      <Inspector />
    </aside>
  {/if}
  {#if barVisible(app.scan.root, app.scan.status, work.busy)}<ActionBar />{/if}
</div>

<TermsDialog />
<!-- Un solo dialogo: las acciones que borran (guard.svelte.ts) y la confirmacion de escanear una unidad extraible. -->
<ConfirmDialog request={confirm.req ?? confirmReq} onresolve={(ok) => (confirm.req ? resolveConfirm(ok) : resolveScan(ok))} />

<style>
  /* Fila de abajo: la barra de accion. */
  .desk {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr) 300px;
    grid-template-rows: minmax(0, 1fr) auto;
  }
  .wide { grid-column: 2 / 4; }
  .sheet-head { display: none; }
  /* Por debajo de 1000 px el detalle deja de ser columna: se superpone al centro, en su misma celda, solo si se abrio. */
  @media (max-width: 999px) {
    .desk { grid-template-columns: 220px minmax(0, 1fr); }
    #center { grid-area: 1 / 2; } /* con el detalle colocado a mano, el centro tambien: si no, la colocacion automatica lo baja de fila */
    .detail { display: none; grid-area: 1 / 2; justify-self: end; width: 300px; z-index: 20; }
    .detail.open { display: block; }
    .sheet-head { display: flex; }
  }
</style>
