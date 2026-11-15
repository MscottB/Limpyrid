<script lang="ts">
  // Estados de Explorador y Mapa: cargando o escaneando, error y vacio. El estado se dice en texto, no solo con color.
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  let { kind, text }: { kind: 'busy' | 'error' | 'empty'; text: string } = $props();
</script>

{#if kind === 'busy'}
  <div class="flex items-center gap-3 rounded-xl border border-border bg-surface p-6 text-muted-foreground tabular-nums" role="status">
    <LoaderCircle size={18} class="shrink-0 animate-spin" aria-hidden="true" />{text}
  </div>
{:else if kind === 'error'}
  <div class="flex items-center gap-3 rounded-xl border border-[color-mix(in_srgb,var(--danger)_45%,transparent)] bg-surface p-6 text-destructive" role="alert">
    <TriangleAlert size={18} class="shrink-0" aria-hidden="true" />{text}
  </div>
{:else}
  <div class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-input p-8 text-center text-muted-foreground">
    <FolderOpen size={20} aria-hidden="true" />{text}
  </div>
{/if}
