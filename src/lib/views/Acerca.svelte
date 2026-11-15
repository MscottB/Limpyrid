<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import Ban from '@lucide/svelte/icons/ban';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as api from '../api';
  import { app, showTerms, terms } from '../state.svelte';
  import { t } from '../i18n';
  import Logo from '../Logo.svelte';
  import { autoCheck, checkNow, openDialog, setAutoCheck, update } from '../update.svelte';

  const PROTECTS = [1, 2, 3, 4];
  const DOES_NOT = [1, 2, 3];

  let version = $state<string | null>(null);
  $effect(() => {
    api.appVersion().then((v) => (version = v), () => (version = null));
  });
</script>

<!-- Secciones separadas por lineas finas, sin tarjetas. -->
<div class="flex flex-col divide-y divide-border border-y border-border">
  <section class="py-6">
    <div class="flex items-center gap-4">
      <Logo size={56} />
      <div class="flex flex-col gap-1.5">
        <h2 class="text-xl font-semibold tracking-tight">Limpyrid</h2>
        {#if version}<Badge variant="secondary" class="tabular-nums">{t('about.version', { v: version })}</Badge>{/if}
      </div>
    </div>
    <p class="mt-4 max-w-[64ch]">{t('about.description')}</p>
  </section>

  <section aria-labelledby="about-updates" class="flex flex-col gap-3 py-5">
    <h2 id="about-updates" class="font-semibold">{t('about.updates')}</h2>
    <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
      <!-- Sin disabled: el boton con el foco no lo pierde al pulsarlo; checkNow ya ignora una segunda llamada. -->
      <Button variant="outline" onclick={() => checkNow(true)}>{t('update.check')}</Button>
      <!-- Resultado de la ultima comprobacion; una version nueva no abre el dialogo sola: lo abre su enlace. -->
      <p role="status" class="text-[13px] tabular-nums">
        {#if update.phase === 'checking'}
          <span class="text-muted-foreground">{t('update.checking')}</span>
        {:else if update.phase === 'current'}
          {t('update.upToDate')}
        {:else if (update.phase === 'available' || update.phase === 'downloading') && update.info}
          <Button variant="link" aria-haspopup="dialog" class="h-auto p-0 text-[13px]" onclick={openDialog}>{t('update.notice', { v: update.info.version })}</Button>
        {:else if update.phase === 'error' && update.error}
          <span class="font-medium text-destructive">{t('err.' + update.error)}</span>
        {/if}
      </p>
    </div>
    <div class="flex min-h-11 items-center gap-3">
      <Checkbox id="about-autocheck" bind:checked={() => autoCheck.on, setAutoCheck} />
      <label for="about-autocheck">{t('update.auto')}</label>
    </div>
    <p class="max-w-[64ch] text-[13px] text-muted-foreground">{t('update.privacy')}</p>
  </section>

  <div class="grid gap-x-8 py-2 md:grid-cols-2">
    {@render list('protects', t('about.protectsTitle'), PROTECTS, true)}
    {@render list('does-not', t('about.doesNotTitle'), DOES_NOT, false)}
  </div>

  <section aria-labelledby="about-license" class="flex flex-wrap items-center gap-x-6 gap-y-3 py-5">
    <div class="flex min-w-60 flex-1 flex-col gap-1">
      <h2 id="about-license" class="font-semibold">{t('about.license')}</h2>
      <p>{t('about.licenseName')} · {t('about.copyright')}</p>
      <p class="text-[13px] text-muted-foreground">
        {terms.at == null ? t('about.termsNotAccepted') : t('about.termsAccepted', { date: new Date(terms.at).toLocaleDateString(app.lang, { dateStyle: 'long' }) })}
      </p>
    </div>
    <Button variant="outline" onclick={showTerms}>{t('about.readTerms')}</Button>
  </section>
</div>

{#snippet list(id: string, title: string, items: number[], safe: boolean)}
  <section aria-labelledby={'about-' + id}>
    <h2 id={'about-' + id} class="py-3 font-semibold">{title}</h2>
    <ul class="divide-y divide-border">
      {#each items as n (n)}
        <li class="flex min-h-11 gap-3 py-3">
          {#if safe}
            <ShieldCheck size={18} class="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          {:else}
            <Ban size={18} class="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          {/if}
          <span>{t((safe ? 'about.protects.' : 'about.doesNot.') + n)}</span>
        </li>
      {/each}
    </ul>
  </section>
{/snippet}
