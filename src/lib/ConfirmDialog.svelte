<script lang="ts" module>
  export interface ConfirmGroup {
    title: string;
    size: string; // ya formateado
    tag?: string; // p. ej. "Definitivo"
    trash?: boolean; // la etiqueta dice "Papelera": no lleva el color de borrado
    notes?: string[]; // avisos de la categoria (antiguedad minima, ramas que se conservan...)
    items?: { label: string; size: string }[];
    impact?: { label: string; text: string; careful: boolean }; // que conlleva limpiarla: nivel y primera frase
  }
  export interface ConfirmRequest {
    title: string;
    before: string[]; // parrafos antes de la lista
    items?: string[];
    groups?: ConfirmGroup[]; // lista agrupada por categoria
    // Una casilla bajo la lista (p. ej. enviar a la Papelera). `onchange` sustituye la peticion entera por la que toca.
    option?: { label: string; note: string; checked: boolean; onchange: (on: boolean) => void };
    after: string[]; // parrafos despues de la lista
    confirmLabel: string; // nombra la accion y la cantidad
    tone?: 'danger' | 'default'; // default: confirmar algo que no borra (p. ej. escanear una unidad extraible)
  }
</script>

<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { t } from './i18n';

  let { request, onresolve }: { request: ConfirmRequest | null; onresolve: (ok: boolean) => void } = $props();

  let cancelBtn = $state<HTMLButtonElement | null>(null);
  // El texto sale de la peticion en el mismo render que abre el dialogo (los botones existen cuando se da el foco
  // inicial) y se conserva mientras el dialogo se cierra, para que no quede vacio durante la transicion.
  let last: ConfirmRequest | null = null;
  const shown = $derived.by(() => (request ? (last = request) : last));
</script>

<!-- Esc y el cierre interno pasan por el setter: con una peticion abierta cuentan como cancelar. -->
<Dialog.Root bind:open={() => !!request, (open) => !open && request && onresolve(false)}>
  <Dialog.Content
    interactOutsideBehavior="ignore"
    onOpenAutoFocus={(e) => {
      e.preventDefault();
      cancelBtn?.focus(); // el foco inicial va a Cancelar: Enter no borra
    }}
    class="max-h-[calc(100vh-48px)] gap-5 overflow-auto sm:max-w-[600px]"
  >
    {#if shown}
      <Dialog.Header>
        <Dialog.Title class="text-lg leading-snug font-semibold">{shown.title}</Dialog.Title>
      </Dialog.Header>
      <div class="flex flex-col gap-3 text-sm">
        <Dialog.Description class="flex flex-col gap-2 text-foreground">
          {#each shown.before as p}<p>{p}</p>{/each}
        </Dialog.Description>
        {#if shown.items?.length}
          <ul class="max-h-[30vh] list-disc overflow-auto border-y border-border py-2 pr-3 pl-5 [overflow-wrap:anywhere]">
            {#each shown.items as item}<li class="py-0.5">{item}</li>{/each}
          </ul>
        {/if}
        {#if shown.groups?.length}
          <ul class="max-h-[44vh] overflow-auto border-y border-border">
            {#each shown.groups as g}
              <li class="border-b border-border py-3 last:border-b-0">
                <div class="flex items-center gap-3">
                  <span class="min-w-0 flex-1 font-semibold">{g.title}</span>
                  {#if g.tag}<Badge variant={g.trash ? 'secondary' : 'destructive'}>{g.tag}</Badge>{/if}
                  <span class="num w-24 font-semibold">{g.size}</span>
                </div>
                {#if g.impact}
                  <p class="mt-1 text-[13px]"><span class="font-semibold {g.impact.careful ? 'text-warning' : ''}">{g.impact.label}:</span> {g.impact.text}</p>
                {/if}
                {#each g.notes ?? [] as n}<p class="mt-1 text-[13px] text-muted-foreground">{n}</p>{/each}
                {#if g.items?.length}
                  <ul class="mt-2 flex flex-col gap-1 border-l-2 border-border pl-3 text-[13px]">
                    {#each g.items as i}
                      <li class="flex gap-3"><span class="min-w-0 flex-1 [overflow-wrap:anywhere]">{i.label}</span><span class="num w-24">{i.size}</span></li>
                    {/each}
                  </ul>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if shown.option}
          {@const opt = shown.option}
          <label class="flex items-start gap-3">
            <Checkbox class="mt-0.5" aria-label={opt.label} aria-describedby="confirm-option-note" bind:checked={() => opt.checked, (on) => opt.onchange(on)} />
            <span class="flex flex-col gap-0.5"><span class="font-medium">{opt.label}</span><span id="confirm-option-note" class="text-[13px] text-muted-foreground">{opt.note}</span></span>
          </label>
        {/if}
        {#each shown.after as p}<p class="font-semibold">{p}</p>{/each}
      </div>
      <Dialog.Footer>
        <Button variant="outline" bind:ref={cancelBtn} onclick={() => onresolve(false)}>{t('confirm.cancel')}</Button>
        <Button variant={shown.tone === 'default' ? 'default' : 'destructive'} onclick={() => onresolve(true)}>{shown.confirmLabel}</Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
