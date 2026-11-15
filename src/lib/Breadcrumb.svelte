<script lang="ts">
  // Ruta de la carpeta actual, de la raiz escaneada hacia abajo (nunca por encima de ella), con el boton Subir.
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import HardDrive from '@lucide/svelte/icons/hard-drive';
  import { Button } from '$lib/components/ui/button';
  import { t } from './i18n';
  import { crumbs, isInside, parentPath } from './treemap';

  let { root, path, onnavigate }: { root: string; path: string; onnavigate: (path: string) => void } = $props();

  const items = $derived(crumbs(root, path));
  const canUp = $derived(isInside(path, root) && !isInside(root, path));
</script>

<div class="flex min-w-0 flex-1 basis-48 items-center gap-3">
  <Button variant="outline" size="sm" disabled={!canUp} title={canUp ? t('explore.upTo', { path: parentPath(path) }) : undefined} onclick={() => canUp && onnavigate(parentPath(path))}>
    <ArrowUp aria-hidden="true" />{t('explore.up')}
  </Button>
  <nav aria-label={t('crumb.label')} class="min-w-0 flex-1">
    <ol class="flex flex-wrap items-center gap-x-0.5 gap-y-1">
      {#each items as c, i (c.path)}
        <li class="flex min-w-0 items-center gap-0.5">
          {#if i > 0}<ChevronRight size={14} class="shrink-0 text-muted-foreground" aria-hidden="true" />{/if}
          {#if i === items.length - 1}
            <span aria-current="page" title={c.path} class="flex h-8 min-w-0 items-center gap-2 px-2 font-semibold">
              {#if i === 0}<HardDrive size={16} class="shrink-0 text-muted-foreground" aria-hidden="true" />{/if}<span class="max-w-[32ch] truncate">{c.label}</span>
            </span>
          {:else}
            <button
              type="button"
              title={c.path}
              onclick={() => onnavigate(c.path)}
              class="flex h-8 min-w-0 items-center gap-2 rounded-md px-2 text-muted-foreground underline-offset-4 hover:text-foreground transition-colors hover:bg-raised hover:underline focus-visible:outline-2 focus-visible:outline-ring"
            >
              {#if i === 0}<HardDrive size={16} class="shrink-0" aria-hidden="true" />{/if}<span class="max-w-[24ch] truncate">{c.label}</span>
            </button>
          {/if}
        </li>
      {/each}
    </ol>
  </nav>
</div>
