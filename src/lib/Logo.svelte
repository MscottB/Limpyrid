<script lang="ts">
  // Marca redibujada en SVG; es un borrador pendiente de aprobacion: cambiarla es reemplazar assets/logo.svg.
  import logo from './assets/logo.svg';

  let { size = 32, class: className = '' }: { size?: number; class?: string } = $props();
</script>

<!-- Decorativa: el nombre "Limpyrid" siempre va al lado como texto. -->
<img src={logo} alt="" width={size} height={size} class={className} draggable="false" />
