<script lang="ts">
  // Una fila de la lista del grupo: casilla, nombre, nivel (o nota), tamano y barra. Sin onToggle no hay casilla.
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import type { Impact } from './impact';
  import { app, locked, openDetail } from './state.svelte';
  import { t } from './i18n';
  import { formatBytes } from './format';

  // `warn`: la nota es un aviso (protegido) y va en ambar; `expanded`: la fila tiene anidadas y dice si se ven;
  // `disabled`: la casilla no se puede usar ahora (p. ej. la lista de worktrees se esta recargando).
  let {
    name, size, max, level, note, checked = false, indeterminate = false, onToggle, depth = 0, cleaned, focused, tabindex, onFocus, title,
    warn = false, expanded, disabled = false, description,
  }: {
    name: string;
    size: number;
    max: number;
    level?: Impact;
    note?: string;
    checked?: boolean;
    indeterminate?: boolean;
    onToggle?: (on: boolean) => void;
    depth?: 0 | 1;
    cleaned?: { before: number };
    focused: boolean;
    tabindex: number;
    onFocus: () => void;
    title: string;
    warn?: boolean;
    expanded?: boolean;
    disabled?: boolean;
    description?: string; // dato que no cabe en la fila: lo leen los lectores de pantalla (el title no les llega)
  } = $props();
  const descId = $props.id();

</script>

<!-- Foco itinerante: una sola fila entra en el orden de Tab; data-row la encuentra la navegacion (flecha derecha). -->
<!-- aria-expanded en un group: ARIA 1.2 no lo lista, pero los lectores lo anuncian; la alternativa (tree/treeitem) cambia toda la lista. -->
<!-- El clic abre el detalle superpuesto; su tecla es Enter, que atiende GroupList para toda la lista. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_role_supports_aria_props, a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<div
  data-row
  role="group"
  aria-label={name}
  aria-describedby={description ? descId : undefined}
  aria-expanded={expanded}
  {tabindex}
  onfocusin={onFocus}
  onclick={(e) => !(e.target as HTMLElement).closest('button') && openDetail(e.currentTarget)}
  class="grid h-9 grid-cols-[16px_minmax(0,1fr)_auto_64px_48px] items-center gap-2 border-l-2 pr-4 -outline-offset-2 min-[1200px]:grid-cols-[16px_minmax(0,1fr)_auto_64px_70px]
    {depth ? 'pl-10' : 'pl-4'} {focused ? 'border-primary bg-primary/10' : 'border-transparent'} {cleaned ? 'text-muted-foreground' : ''}"
>
  {#if onToggle}
    <Checkbox aria-label={name} disabled={locked() || disabled} bind:checked={() => checked, (on) => onToggle(on)} {indeterminate} />
  {:else}
    <span aria-hidden="true"></span>
  {/if}
  <!-- Lo que cede cuando falta sitio es el nombre: el texto completo queda en title y en el detalle. -->
  <span class="truncate {cleaned ? 'line-through' : ''} {depth ? '' : 'font-medium'}" {title}>{name}{#if description}<span id={descId} class="sr-only">{description}</span>{/if}</span>
  {#if cleaned}
    <Badge variant="success">{t('cleaned.badge')}</Badge>
  {:else if level}
    <Badge variant={level === 'careful' ? 'warning' : 'secondary'}>{t('impact.' + level)}</Badge>
  {:else}
    <span class="text-[13px] whitespace-nowrap {warn ? 'text-warning' : 'text-muted-foreground'}">{note ?? ''}</span>
  {/if}
  <span class="num">{formatBytes(size, app.lang)}</span>
  <span aria-hidden="true" class="block h-1 overflow-hidden rounded-full bg-chart-free">
    {#if size > 0 && !cleaned}<span class="block h-full min-w-[2px] rounded-full {checked ? 'bg-primary' : 'bg-chart-used'}" style:width="{Math.min(100, (size / max) * 100)}%"></span>{/if}
  </span>
</div>
