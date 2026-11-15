<script lang="ts">
  // Aviso de uso: resume TERMINOS.md. Antes de borrar pide marcar la casilla y aceptar; desde Acerca de solo se lee.
  // Se monta una vez (App.svelte) y lo abren ensureTerms y showTerms de state.svelte.ts.
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { closeTerms, terms } from './state.svelte';
  import { t } from './i18n';

  const POINTS = [1, 2, 3, 4, 5];
  let checked = $state(false);
  let cancelBtn = $state<HTMLButtonElement | null>(null);
  // Se conserva el modo mientras el dialogo se cierra, para que el contenido no cambie durante la transicion.
  let last: 'ask' | 'read' = 'ask';
  const mode = $derived.by(() => (terms.mode ? (last = terms.mode) : last));

  // Cada apertura empieza con la casilla sin marcar.
  $effect(() => {
    if (terms.mode) checked = false;
  });
</script>

<!-- Esc y el cierre interno cuentan como cancelar. -->
<Dialog.Root bind:open={() => !!terms.mode, (open) => !open && terms.mode && closeTerms(false)}>
  <Dialog.Content
    interactOutsideBehavior="ignore"
    onOpenAutoFocus={(e) => {
      e.preventDefault();
      cancelBtn?.focus(); // el foco inicial va a Cancelar (o Cerrar)
    }}
    class="max-h-[calc(100vh-48px)] gap-5 overflow-auto sm:max-w-[600px]"
  >
    <Dialog.Header>
      <Dialog.Title class="text-lg leading-snug font-semibold">{t('terms.title')}</Dialog.Title>
      <Dialog.Description class="text-foreground">{t('terms.lead')}</Dialog.Description>
    </Dialog.Header>
    <ul class="flex list-disc flex-col gap-2 pl-5 text-sm">
      {#each POINTS as n (n)}<li>{t('terms.point.' + n)}</li>{/each}
    </ul>
    <div class="flex flex-col gap-1 border-t border-border pt-4 text-[13px] text-muted-foreground">
      <p>{t('terms.license')}</p>
      <p>{t('terms.copyright')}</p>
    </div>
    {#if mode === 'ask'}
      <label class="flex min-h-11 cursor-pointer items-center gap-3 font-medium">
        <input type="checkbox" class="cb" bind:checked />
        {t('terms.check')}
      </label>
    {/if}
    <Dialog.Footer>
      {#if mode === 'ask'}
        <Button variant="outline" bind:ref={cancelBtn} onclick={() => closeTerms(false)}>{t('confirm.cancel')}</Button>
        <Button disabled={!checked} onclick={() => closeTerms(true)}>{t('terms.accept')}</Button>
      {:else}
        <Button variant="outline" bind:ref={cancelBtn} onclick={() => closeTerms(false)}>{t('terms.close')}</Button>
      {/if}
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
