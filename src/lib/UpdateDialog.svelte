<script lang="ts">
  // Dialogo de actualizacion: version actual y nueva, notas y un solo boton principal. Se monta una vez (App.svelte) y
  // lo abren el aviso de la navegacion y Acerca de. Mientras descarga no se puede cerrar: la app se cerrara sola.
  import { tick } from 'svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import * as api from './api';
  import type { UpdateInfo } from './api';
  import { app, locked } from './state.svelte';
  import { canInstall, mainAction } from './update';
  import { checkNow, closeDialog, dismiss, install, update } from './update.svelte';
  import { formatBytes, formatShare } from './format';
  import { t } from './i18n';

  // Una comprobacion que no encuentra nada borra info: se conserva la ultima para que el dialogo no quede vacio.
  let last: UpdateInfo | null = null;
  const info = $derived.by(() => (update.info ? (last = update.info) : last));
  const busy = $derived(update.phase === 'downloading');
  const current = $derived(update.phase === 'current');
  const portable = $derived(mainAction(update) === 'open-page');
  const blocked = $derived(update.phase === 'available' && !portable && !canInstall(update, locked()));
  const date = $derived.by(() => {
    const d = info?.date ? new Date(info.date) : null;
    return d && !isNaN(d.getTime()) ? d.toLocaleDateString(app.lang, { dateStyle: 'long' }) : null;
  });

  let laterBtn = $state<HTMLButtonElement | null>(null);
  let contentEl = $state<HTMLElement | null>(null);
  let origin: HTMLElement | null = null;
  let next: HTMLElement | null = null;
  // Al cambiar de fase cambian los botones: el foco vuelve a "Mas tarde" o, descargando (sin botones), al propio dialogo.
  // (bind:ref no vuelve a null al quitarse el boton: se mira si sigue en la pagina.)
  const focusHome = () => (laterBtn?.isConnected ? laterBtn : contentEl)?.focus();
  $effect(() => {
    void update.phase;
    tick().then(() => {
      if (update.open) focusHome();
    });
  });

  function openPage() {
    void api.openDownloads();
    closeDialog();
  }
</script>

<Dialog.Root bind:open={() => update.open, (open) => !open && !busy && closeDialog()}>
  <Dialog.Content
    bind:ref={contentEl}
    tabindex={-1}
    interactOutsideBehavior="ignore"
    escapeKeydownBehavior={busy ? 'ignore' : 'close'}
    onOpenAutoFocus={(e) => {
      e.preventDefault();
      origin = document.activeElement as HTMLElement | null;
      next = origin?.nextElementSibling as HTMLElement | null;
      focusHome(); // el foco inicial va a "Mas tarde": Enter no cierra la app
    }}
    onCloseAutoFocus={(e) => {
      // "Mas tarde" quita el aviso que abrio el dialogo: entonces el foco pasa al elemento que lo seguia (Acerca de).
      if (origin?.isConnected || !next?.isConnected) return;
      e.preventDefault();
      next.focus();
    }}
    class="max-h-[calc(100vh-48px)] gap-5 overflow-auto sm:max-w-[560px]"
  >
    {#if info}
      <Dialog.Header>
        <!-- "Reintentar" puede encontrar que ya no hay version nueva: entonces se dice eso y nada de la anterior. -->
        <Dialog.Title class="text-lg leading-snug font-semibold">{current ? t('update.upToDate') : t('update.notice', { v: info.version })}</Dialog.Title>
        <Dialog.Description class="text-muted-foreground tabular-nums">
          {t('update.have', { v: info.current })}{#if date && !current}{' '}{t('update.published', { date })}{/if}
        </Dialog.Description>
      </Dialog.Header>

      {#if info.notes.trim() && !current}
        <section aria-labelledby="update-notes" class="flex flex-col gap-2">
          <h3 id="update-notes" class="text-[13px] font-medium text-muted-foreground">{t('update.notes')}</h3>
          <!-- Texto plano de fuera: nunca como HTML. Se desplaza solo si es largo, tambien con el teclado. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div tabindex="0" role="region" aria-labelledby="update-notes" class="max-h-[min(40vh,280px)] overflow-auto border-y border-border py-3 pr-3 text-sm outline-offset-4 whitespace-pre-wrap [overflow-wrap:anywhere]">{info.notes.trim()}</div>
        </section>
      {/if}

      {#if busy}
        {@const pct = update.total ? Math.min(100, (update.done / update.total) * 100) : null}
        <section aria-label={t('update.downloading', { v: info.version })} class="flex flex-col gap-2 tabular-nums">
          <div class="flex items-baseline justify-between gap-4">
            <span class="font-medium">{t('update.downloading', { v: info.version })}</span>
            <span class="shrink-0 text-[13px] text-muted-foreground">
              {update.total ? formatShare(update.done, update.total, app.lang) : t('update.downloadedBytes', { size: formatBytes(update.done, app.lang) })}
            </span>
          </div>
          {#if pct != null}
            <div role="progressbar" aria-label={t('update.downloading', { v: info.version })} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)} class="h-1.5 overflow-hidden rounded-full bg-chart-free">
              <span class="block h-full rounded-full bg-chart-used transition-[width] duration-150" style:width="{pct}%"></span>
            </div>
          {/if}
          <p class="text-[13px] text-muted-foreground">{t('update.restartNote')}</p>
        </section>
      {:else}
        {#if update.phase === 'error' && update.error}
          <p role="alert" class="text-[13px] font-medium text-destructive">{t('err.' + update.error)}</p>
        {:else if portable && !current}
          <p class="text-[13px] text-muted-foreground">{t('update.portable')}</p>
        {:else if blocked}
          <p id="update-blocked" class="text-[13px] text-muted-foreground">{t(app.scan.status === 'running' ? 'update.blockedScan' : 'update.blockedClean')}</p>
        {/if}
        <Dialog.Footer>
          {#if update.phase === 'current'}
            <Button variant="outline" bind:ref={laterBtn} onclick={closeDialog}>{t('terms.close')}</Button>
          {:else}
            <Button variant="outline" bind:ref={laterBtn} onclick={dismiss}>{t('update.later')}</Button>
            {#if update.phase === 'error' || update.phase === 'checking'}
              <!-- Tras un fallo no se reinstala a ciegas: se vuelve a comprobar y, si sigue habiendo version, vuelve "Actualizar ahora". -->
              <Button disabled={update.phase === 'checking'} onclick={() => checkNow(true)}>{update.phase === 'checking' ? t('update.checking') : t('update.retry')}</Button>
            {:else if portable}
              <Button onclick={openPage}>{t('update.openPage')}</Button>
            {:else}
              <Button disabled={blocked} aria-describedby={blocked ? 'update-blocked' : undefined} onclick={install}>{t('update.install')}</Button>
            {/if}
          {/if}
        </Dialog.Footer>
      {/if}
    {/if}
  </Dialog.Content>
</Dialog.Root>
