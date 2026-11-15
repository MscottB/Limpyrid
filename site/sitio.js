// Sitio de Limpyrid: idioma, tema, menú, «Copiar» y descargas. El mecanismo de los dos idiomas es el del sitio de Tramtai:
// los dos van en la página (<span lang>) y estilo.css muestra el de <html lang>.

// Idioma: el elegido antes o, si no, el del navegador. Sin almacenamiento disponible también funciona.
(function () {
  var botones = document.querySelectorAll('[data-idioma]');
  function aplicar(idioma) {
    document.documentElement.lang = idioma;
    // Lo que no admite <span lang> guarda el inglés en un atributo: textos alternativos, etiquetas y el título.
    document.querySelectorAll('img[data-alt-en]').forEach(function (img) {
      if (!img.dataset.altEs) img.dataset.altEs = img.alt;
      img.alt = idioma === 'en' ? img.dataset.altEn : img.dataset.altEs;
    });
    document.querySelectorAll('[data-aria-en]').forEach(function (e) {
      if (!e.dataset.ariaEs) e.dataset.ariaEs = e.getAttribute('aria-label');
      e.setAttribute('aria-label', idioma === 'en' ? e.dataset.ariaEn : e.dataset.ariaEs);
    });
    var titulo = document.querySelector('title[data-en]');
    if (titulo) {
      if (!titulo.dataset.es) titulo.dataset.es = titulo.textContent;
      document.title = idioma === 'en' ? titulo.dataset.en : titulo.dataset.es;
    }
    botones.forEach(function (b) { b.setAttribute('aria-pressed', String(b.dataset.idioma === idioma)); });
  }
  var guardado = null;
  try { guardado = localStorage.getItem('limpyrid.sitio.idioma'); } catch (e) {}
  aplicar(guardado === 'es' || guardado === 'en' ? guardado : (navigator.language || 'es').slice(0, 2) === 'es' ? 'es' : 'en');
  botones.forEach(function (b) {
    b.addEventListener('click', function () {
      aplicar(b.dataset.idioma);
      try { localStorage.setItem('limpyrid.sitio.idioma', b.dataset.idioma); } catch (e) {}
    });
  });
  // El menú de idioma (un <details>) se cierra al elegir, al pulsar fuera o con Escape, que devuelve el foco a su botón
  // y no sigue hasta el menú de la barra.
  var menu = document.querySelector('details.idioma');
  if (!menu) return;
  document.addEventListener('click', function (e) { if (!menu.contains(e.target) || e.target.closest('[data-idioma]')) menu.open = false; });
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Escape' || !menu.open) return;
    menu.open = false;
    menu.querySelector('summary').focus();
    e.stopImmediatePropagation();
  });
})();

// Tema: el elegido antes o, si no, el del sistema; el script de <head> ya puso el guardado antes de pintar.
(function () {
  var boton = document.querySelector('.tema');
  if (!boton) return;
  var sistema = matchMedia('(prefers-color-scheme: dark)');
  function oscuro() {
    var tema = document.documentElement.dataset.tema;
    return tema ? tema === 'oscuro' : sistema.matches;
  }
  function pintar() {
    boton.setAttribute('aria-checked', String(oscuro()));
    // El color de la barra del navegador en el móvil sigue al fondo.
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = getComputedStyle(document.body).backgroundColor;
  }
  boton.addEventListener('click', function () {
    var tema = oscuro() ? 'claro' : 'oscuro';
    document.documentElement.dataset.tema = tema;
    try { localStorage.setItem('limpyrid.sitio.tema', tema); } catch (e) {}
    pintar();
  });
  sistema.addEventListener('change', pintar);
  pintar();
})();

// Menú de la barra en pantallas medianas y pequeñas: se abre con su botón y se cierra al elegir, con Escape o fuera.
(function () {
  var barra = document.querySelector('.barra');
  var boton = document.querySelector('.menu-boton');
  if (!barra || !boton) return;
  boton.hidden = false;
  function abrir(si) {
    barra.classList.toggle('abierta', si);
    boton.setAttribute('aria-expanded', String(si));
  }
  boton.addEventListener('click', function () { abrir(boton.getAttribute('aria-expanded') !== 'true'); });
  document.getElementById('menu').addEventListener('click', function (e) { if (e.target.closest('a')) abrir(false); });
  document.addEventListener('click', function (e) { if (!barra.contains(e.target)) abrir(false); });
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Escape' || !barra.classList.contains('abierta')) return;
    abrir(false);
    boton.focus();
  });
})();

// Botón «Copiar» en cada bloque de código, con los dos idiomas como el resto de la página.
(function () {
  if (!navigator.clipboard) return;
  var copiar = '<span lang="es">Copiar</span><span lang="en">Copy</span>';
  var copiado = '<span lang="es">Copiado</span><span lang="en">Copied</span>';
  document.querySelectorAll('pre.bloque-codigo').forEach(function (pre) {
    var boton = document.createElement('button');
    boton.type = 'button';
    boton.className = 'copiar';
    boton.innerHTML = copiar;
    boton.addEventListener('click', function () {
      navigator.clipboard.writeText(pre.querySelector('code').textContent.trim()).then(function () {
        boton.innerHTML = copiado;
        setTimeout(function () { boton.innerHTML = copiar; }, 1500);
      });
    });
    pre.appendChild(boton);
  });
})();

// Descargas: al publicar una versión se rellena el bloque #descargas de la página con
// {"version": "0.1.0", "archivos": {"Limpyrid-setup.exe": {"url": "…", "sha256": "…"}, …}}.
// Vacío ({}), no hay versión publicada: los enlaces siguen siendo los provisionales y sus avisos se quedan.
(function () {
  var bloque = document.getElementById('descargas');
  var datos = {};
  try { datos = JSON.parse(bloque.textContent) || {}; } catch (e) {}
  if (!datos.version || !datos.archivos) return;
  document.querySelectorAll('[data-archivo]').forEach(function (a) {
    var f = datos.archivos[a.dataset.archivo];
    if (f && f.url) a.href = f.url;
  });
  document.querySelectorAll('[data-suma]').forEach(function (c) {
    var f = datos.archivos[c.dataset.suma];
    if (f && f.sha256) c.textContent = f.sha256;
  });
  document.querySelectorAll('[data-version]').forEach(function (v) { v.textContent = ' · v' + datos.version; });
  document.documentElement.classList.add('publicado');
})();
