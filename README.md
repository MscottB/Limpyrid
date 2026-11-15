<div align="center">

<img src="site/icono-256.png" alt="" width="88">

# Limpyrid

**Recupera tu disco, sin borrar a ciegas.**

App de escritorio para Windows que muestra qué ocupa tu disco y limpia, con cuidado, lo que dejan las herramientas de desarrollo: cachés de paquetes, carpetas que se regeneran, worktrees de git, Docker y temporales de agentes de IA.

[![Windows 10 y 11](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-0078d4)](https://mscottb.github.io/Limpyrid/descargar.html)
[![Licencia GPL-3.0-or-later](https://img.shields.io/badge/licencia-GPL--3.0--or--later-22c9e0)](LICENSE)

[**Sitio**](https://mscottb.github.io/Limpyrid/) · [Producto](https://mscottb.github.io/Limpyrid/producto.html) · [Descargar](https://mscottb.github.io/Limpyrid/descargar.html) · [Términos](TERMINOS.md) · [English](#english)

<br>

<img src="site/capturas/grupo-es-1280.webp" alt="Limpyrid con el grupo Python abierto: la lista de cachés con su tamaño y, a la derecha, qué es la caché de uv y qué pasa si se limpia" width="880">

</div>

> **Estado:** versión 0.1.0, en preparación. Todavía no hay instalador publicado; por ahora se compila desde el código.

## Qué ofrece

- **Qué ocupa el disco**, agrupado por herramienta y por lo que cuesta recuperarlo.
- **Una razón para cada cosa:** qué es, qué pasa si la limpias, dónde está y cuándo se usó.
- **Lo marcado siempre a la vista**, con cómo quedaría el disco, hasta que confirmas.
- **Carpetas y Mapa** para investigar lo que no entra en ninguna categoría.

Reconoce npm, pnpm, Yarn, Bun, pip, uv, Poetry, Go, Cargo, Gradle, Maven, NuGet, Docker, Podman, worktrees de git, sesiones de Claude y más: la lista completa está en [`catalog.rs`](src-tauri/src/catalog.rs).

## Cómo protege tus archivos

- **Nada se borra sin confirmación**, y la confirmación lista cada elemento.
- **Las carpetas del sistema están bloqueadas** en el backend, no solo en la interfaz.
- **Solo lo que se regenera:** lo que contiene datos tuyos se muestra, pero no se ofrece limpiar.
- **Worktrees en uso** (cambios sin commit, actividad reciente) nunca se ofrecen.
- **No envía datos de tu equipo.** Su única conexión es la consulta de versión a GitHub, que se puede apagar en Acerca de, y solo pide permisos de administrador para compactar un disco virtual.

El detalle está en la página [Producto](https://mscottb.github.io/Limpyrid/producto.html).

## Compilar

Windows 10 u 11, [Rust](https://www.rust-lang.org/tools/install) y [Node.js](https://nodejs.org/) 22 o superior.

```bash
npm install
```

```bash
npm run tauri dev
```

`npm run tauri build` genera el ejecutable y el instalador. Pruebas: `npm test`, `npm run check` y `cargo test --manifest-path src-tauri/Cargo.toml`. Para añadir un idioma, copia `src/lib/i18n/en.json` con el código del idioma nuevo y tradúcelo.

## Términos y licencia

Copyright © 2026 Pergamio. Software libre bajo `GPL-3.0-or-later` ([LICENSE](LICENSE)). Limpyrid borra archivos de forma definitiva y se entrega tal cual, sin garantías: revisa la lista antes de confirmar. Los términos de uso están en [TERMINOS.md](TERMINOS.md).

---

## English

**Limpyrid is a Windows desktop app that shows what is using your disk and carefully cleans what developer tools leave behind:** package caches, folders that can be rebuilt, git worktrees, Docker data and temporary files from AI agents. Before anything is deleted, it tells you what each item is and what happens if you clean it.

Nothing is deleted without a confirmation that lists every item, system folders are blocked in the backend, and it sends no data from your computer: its only connection is a version check with GitHub, which you can turn off. Version 0.1.0 is in preparation: there is no published installer yet, so for now you build it from source with `npm install` and `npm run tauri dev`.

[Site](https://mscottb.github.io/Limpyrid/) · Free software under `GPL-3.0-or-later`, provided as is, with no warranty. Copyright © 2026 Pergamio.
