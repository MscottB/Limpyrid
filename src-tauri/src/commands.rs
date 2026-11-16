// Funciones internas de los comandos: reciben el estado (y el catalogo) para poder probarse sin Tauri.
use crate::catalog::{self, Category, Found, Kind};
use crate::clean::{self, Report, SkipReason, Skipped};
use crate::scan::{self, Node, Progress, ScanResult};
use crate::worktrees::{self, WorktreeList};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

const REPARSE_POINT: u32 = 0x400;

#[derive(Default)]
pub struct AppState {
    pub scan: Mutex<Option<ScanResult>>,
    // Arboles de rutas de categoria fuera del escaneo, por ruta canonica (scan::parts); se vacia con cada escaneo nuevo.
    pub side: Mutex<HashMap<PathBuf, Node>>,
    pub side_gen: AtomicU64, // sube con cada vaciado o retirada en `side`: una medida empezada antes no se guarda
    pub cancel: AtomicBool,
    pub running: AtomicBool,
    pub busy: AtomicBool,
    pub no_projects: catalog::NoProjects, // las pruebas lo sustituyen: sus arboles viven bajo %LOCALAPPDATA%\Temp
}

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
}

#[derive(Serialize)]
pub struct CategoryInfo {
    pub id: &'static str,
    pub ecosystem: &'static str,
    pub kind: &'static str,
    pub uses_min_age: bool,
    pub size: u64,
    pub paths: Vec<String>,
    pub items: Vec<Found>,
}

// Codigos de error (lista de la seccion Global Constraints del plan; la interfaz los traduce con err.<codigo>):
// no_scan, scan_running, busy, blocked, unknown_category, engine_missing, engine_stopped, engine_remote,
// uac_denied, command_failed.
type Res<T> = Result<T, &'static str>;

impl AppState {
    fn lock(&self) -> MutexGuard<'_, Option<ScanResult>> {
        self.scan.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn side(&self) -> MutexGuard<'_, HashMap<PathBuf, Node>> {
        self.side.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// Limpia la marca de "escaneo en curso" al soltarse, incluso si el hilo entra en panico.
pub struct RunningGuard<'a>(&'a AtomicBool);
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub fn begin_scan(state: &AppState) -> Res<()> {
    state.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).map_err(|_| "scan_running")?;
    if state.busy.load(Ordering::SeqCst) {
        state.running.store(false, Ordering::SeqCst);
        return Err("busy");
    }
    state.cancel.store(false, Ordering::SeqCst);
    Ok(())
}

// Lo ejecuta el hilo del escaneo tras `begin_scan`. Devuelve (cancelado, rutas denegadas).
// Un escaneo cancelado no deja resultado: se conserva el anterior, si lo habia.
pub fn run_scan(state: &AppState, root: &Path, on_progress: &(dyn Fn(Progress) + Sync)) -> (bool, usize) {
    let _guard = RunningGuard(&state.running);
    let result = scan::scan(root, &state.cancel, on_progress);
    let out = (result.cancelled, result.denied.len());
    if !result.cancelled {
        *state.lock() = Some(result);
        let mut side = state.side();
        side.clear();
        state.side_gen.fetch_add(1, Ordering::SeqCst);
    }
    out
}

// Una sola operacion que borra o para el motor a la vez (limpiar, papelera, prune, compactar), nunca durante un
// escaneo. Cada lado marca su bandera y luego mira la otra (SeqCst): de dos que empiezan a la vez, uno ve al otro.
pub fn begin_work(state: &AppState) -> Res<RunningGuard<'_>> {
    state.busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).map_err(|_| "busy")?;
    let guard = RunningGuard(&state.busy);
    if state.running.load(Ordering::SeqCst) {
        return Err("scan_running"); // la guarda suelta la marca
    }
    Ok(guard)
}

// Carpetas sin permiso del ultimo escaneo, en forma plana.
pub fn denied_paths(state: &AppState) -> Vec<String> {
    state.lock().as_ref().map(|r| r.denied.iter().map(|p| scan::plain(p)).collect()).unwrap_or_default()
}

pub fn children(state: &AppState, path: &str) -> Res<Vec<Entry>> {
    let guard = state.lock();
    let result = guard.as_ref().ok_or("no_scan")?;
    // Igual que treemap: una ruta fuera del escaneo cuenta como "sin escaneo" para esa ruta.
    let node = scan::find(result, Path::new(path)).ok_or("no_scan")?;
    Ok(node
        .children
        .iter()
        .map(|c| Entry {
            name: c.name.clone(),
            path: scan::plain(&Path::new(path).join(&c.name)),
            size: c.size,
            files: c.files,
            is_dir: c.is_dir,
        })
        .collect())
}

pub fn treemap(state: &AppState, path: &str) -> Res<Node> {
    let guard = state.lock();
    let result = guard.as_ref().ok_or("no_scan")?;
    // Una ruta fuera del escaneo cuenta como "sin escaneo" para esa ruta.
    scan::find(result, Path::new(path)).map(|n| scan::pruned(n, 3, 40)).ok_or("no_scan")
}

fn excluded(name: &str, exclude: &[&str]) -> bool {
    exclude.iter().any(|e| name.eq_ignore_ascii_case(e))
}

// Tamano de una ruta descontando los nombres de primer nivel de `exclude`: del escaneo si esta dentro, o de `outside`.
// Un escaneo cancelado deja carpetas sin visitar como nodos vacios: entonces siempre se mide.
fn path_size(state: &AppState, path: &Path, exclude: &[&str]) -> u64 {
    let net = |n: &Node| n.size - n.children.iter().filter(|c| excluded(&c.name, exclude)).map(|c| c.size).sum::<u64>();
    if let Some(size) = state.lock().as_ref().filter(|r| !r.cancelled).and_then(|r| scan::find(r, path)).map(net) {
        return size;
    }
    match fs::metadata(scan::verbatim(path)) {
        Ok(m) if !m.is_dir() => m.len(), // p. ej. un .vhdx
        _ => outside(state, path, net),
    }
}

// Carpeta fuera del escaneo: se busca en `side` (ella o una carpeta que la contenga); si no esta, se escanea una vez
// y se guarda. Ningun candado se tiene mientras se recorre el disco.
fn outside<R>(state: &AppState, path: &Path, f: impl Fn(&Node) -> R) -> R {
    let target = scan::parts(path);
    {
        let side = state.side();
        let hit = side.iter().find_map(|(k, tree)| {
            let rel = target.strip_prefix(scan::parts(k).as_slice())?;
            rel.iter().try_fold(tree, |n, p| n.children.iter().find(|c| c.name.to_lowercase() == *p))
        });
        if let Some(n) = hit {
            return f(n);
        }
    }
    let seen = state.side_gen.load(Ordering::SeqCst);
    let tree = scan::scan(path, &AtomicBool::new(false), &|_| {}).tree;
    let out = f(&tree);
    remember(state, PathBuf::from(target.join("\\")), tree, seen);
    out
}

// Guarda una medida de `outside` solo si `side` no se vacio ni perdio entradas desde que empezo (generacion `seen`).
// La generacion se compara y se sube con el candado de `side` puesto.
fn remember(state: &AppState, key: PathBuf, tree: Node, seen: u64) {
    let mut side = state.side();
    if state.side_gen.load(Ordering::SeqCst) == seen {
        side.insert(key, tree);
    }
}

// Tras prune o compactar (acabe bien o mal): el disco del motor `id` pudo cambiar, se vuelven a medir sus rutas.
pub fn refresh_containers(state: &AppState, catalog: &[Category], id: &str) {
    for cat in catalog.iter().filter(|c| c.id == id && matches!(c.kind, Kind::Containers)) {
        cat.paths.iter().flat_map(|p| catalog::expand(p)).for_each(|p| refresh(state, &p));
    }
}

// Fecha mas reciente de una carpeta en segundos Unix: la anotada en el escaneo si la contiene, o en el recorrido
// recordado de `outside`; si es 0 (desconocida), se lee del disco.
fn newest(state: &AppState, path: &Path) -> Option<u64> {
    let scanned = state.lock().as_ref().filter(|r| !r.cancelled).and_then(|r| scan::find(r, path)).map(|n| n.newest);
    let n = scanned.unwrap_or_else(|| outside(state, path, |t| t.newest));
    (n > 0).then_some(n).or_else(|| clean::newest_mtime(&scan::verbatim(path), None).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()))
}

// Todas las carpetas de proyecto del escaneo actual; recorre el arbol entero, asi que se llama una vez por peticion.
fn detected(state: &AppState) -> Vec<Found> {
    state.lock().as_ref().map(|r| catalog::detect_projects(r, &state.no_projects)).unwrap_or_default()
}

pub fn categories(state: &AppState, catalog: &[Category]) -> Vec<CategoryInfo> {
    let mut out = vec![];
    let mut projects = detected(state);
    for cat in catalog {
        // Sin rutas fijas ni lista: aparece solo si el escaneo contiene algun repositorio con worktrees.
        if matches!(cat.kind, Kind::Worktrees { .. }) {
            if state.lock().as_ref().is_some_and(|r| !worktrees::repos(r, &state.no_projects).is_empty()) {
                out.push(CategoryInfo {
                    id: cat.id,
                    ecosystem: cat.ecosystem,
                    kind: "worktrees",
                    uses_min_age: catalog::effective_min_age(cat, None).is_some(),
                    size: 0,
                    paths: vec![],
                    items: vec![],
                });
            }
            continue;
        }
        let paths: Vec<PathBuf> = cat.paths.iter().flat_map(|p| catalog::expand(p)).collect();
        let (kind, items) = match cat.kind {
            Kind::Contents { .. } => ("contents", vec![]),
            Kind::Subfolders { .. } => ("subfolders", catalog::subfolders(cat, &|p| path_size(state, p, &[]), &|p| newest(state, p))),
            // La fecha se lee aqui y no en el recorrido del escaneo: solo para las carpetas listadas.
            Kind::Project => ("project", projects.extract_if(.., |f| f.category == cat.id).map(|f| Found { modified: catalog::project_mtime(Path::new(&f.path)), ..f }).collect()),
            Kind::Containers => ("containers", vec![]),
            Kind::InfoOnly => ("info_only", vec![]),
            Kind::Worktrees { .. } => ("worktrees", vec![]),
        };
        let size = if matches!(cat.kind, Kind::Project) {
            if items.is_empty() {
                continue;
            }
            items.iter().map(|f| f.size).sum()
        } else {
            if paths.is_empty() {
                continue;
            }
            paths.iter().map(|p| path_size(state, p, cat.exclude)).sum()
        };
        out.push(CategoryInfo {
            id: cat.id,
            ecosystem: cat.ecosystem,
            kind,
            uses_min_age: catalog::effective_min_age(cat, None).is_some(),
            size,
            paths: paths.iter().map(|p| scan::plain(p)).collect(),
            items,
        });
    }
    out
}

// Sustituye el nodo en `rel` por `new` y recalcula tamanos y orden de sus ancestros.
fn graft(node: &mut Node, rel: &[String], mut new: Node) -> bool {
    let Some(i) = node.children.iter().position(|c| c.name.to_lowercase() == rel[0]) else { return false };
    if rel.len() == 1 {
        new.name = node.children[i].name.clone();
        node.children[i] = new;
    } else if !graft(&mut node.children[i], &rel[1..], new) {
        return false;
    }
    node.size = node.children.iter().map(|c| c.size).sum();
    node.files = node.children.iter().map(|c| c.files).sum();
    node.children.sort_by(|a, b| b.size.cmp(&a.size));
    true
}

// Deja el arbol en memoria al dia con el disco para `path`: lo quita si ya no existe o lo vuelve a escanear.
fn refresh(state: &AppState, path: &Path) {
    // Las medidas recordadas fuera del escaneo que tocan `path` (ella, dentro o por encima) se vuelven a medir al pedirlas.
    let target = scan::parts(path);
    {
        let mut side = state.side();
        side.retain(|k, _| {
            let k = scan::parts(k);
            !k.starts_with(&target) && !target.starts_with(&k)
        });
        state.side_gen.fetch_add(1, Ordering::SeqCst); // siempre: una medida de `path` en curso aun no esta en `side`
    }
    if !state.lock().as_ref().is_some_and(|r| scan::find(r, path).is_some()) {
        return;
    }
    match fs::symlink_metadata(scan::verbatim(path)) {
        Err(_) => {
            if let Some(r) = state.lock().as_mut() {
                scan::remove(r, path);
            }
        }
        Ok(m) if m.is_dir() && m.file_attributes() & REPARSE_POINT == 0 => {
            let fresh = scan::scan(path, &AtomicBool::new(false), &|_| {}).tree;
            if let Some(r) = state.lock().as_mut() {
                let (root, target) = (scan::parts(&r.root), scan::parts(path));
                match target.strip_prefix(root.as_slice()) {
                    Some([]) => r.tree = fresh,
                    Some(rel) => {
                        graft(&mut r.tree, rel, fresh);
                    }
                    None => {}
                }
            }
        }
        Ok(_) => {}
    }
}

fn absorb(total: &mut Report, part: Report) {
    total.freed += part.freed;
    total.deleted += part.deleted;
    total.skipped.extend(part.skipped);
}

// Las rutas pedidas que no estan en `allowed` no se tocan y constan como bloqueadas.
fn split_allowed(requested: &[String], allowed: &[String], rep: &mut Report) -> Vec<PathBuf> {
    let mut ok = vec![];
    for r in requested {
        match allowed.iter().find(|a| scan::parts(Path::new(a)) == scan::parts(Path::new(r))) {
            Some(a) => ok.push(PathBuf::from(a)),
            None => rep.skipped.push(Skipped { path: r.clone(), reason: SkipReason::Blocked }),
        }
    }
    ok
}

// Eventos de progreso de una limpieza: como mucho uno cada 200 ms.
pub fn should_emit(last_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_ms) >= 200
}

pub fn clean_category(state: &AppState, catalog: &[Category], id: &str, items: &[String], min_age_hours: Option<u64>) -> Res<Report> {
    clean_category_with(state, catalog, id, items, min_age_hours, &|_, _| {}, None)
}

// `emit` recibe (liberado, borrados) acumulados de toda la categoria, sumando sus rutas: espaciado con should_emit y una
// ultima vez con los totales del informe. Solo observa; ningun candado se tiene mientras se llama.
// `to_trash`: lo que se limpiaria va a la Papelera (clean::trash_entry) tras las mismas comprobaciones; el informe cuenta
// entonces lo enviado, no lo liberado.
pub fn clean_category_with(
    state: &AppState,
    catalog: &[Category],
    id: &str,
    items: &[String],
    min_age_hours: Option<u64>,
    emit: clean::OnProgress,
    to_trash: Option<clean::Recycle>,
) -> Res<Report> {
    let _work = begin_work(state)?;
    let cat = catalog.iter().find(|c| c.id == id).ok_or("unknown_category")?;
    let age = catalog::effective_min_age(cat, min_age_hours);
    let mut rep = Report::default();
    let start = std::time::Instant::now();
    let last = AtomicU64::new(0);
    let tick = |freed: u64, deleted: u64| {
        let now = start.elapsed().as_millis() as u64;
        if should_emit(last.load(Ordering::Relaxed), now) {
            last.store(now, Ordering::Relaxed);
            emit(freed, deleted);
        }
    };
    match cat.kind {
        Kind::Contents { .. } => {
            for p in cat.paths.iter().flat_map(|p| catalog::expand(p)) {
                let (f0, n0) = (rep.freed, rep.deleted);
                absorb(&mut rep, clean::delete_contents_except_with(&p, age, cat.exclude, &|f, n| tick(f0 + f, n0 + n), to_trash));
                refresh(state, &p);
            }
        }
        Kind::Project => {
            if state.lock().is_none() {
                return Err("no_scan");
            }
            let allowed: Vec<String> = detected(state).into_iter().filter(|f| f.category == cat.id).map(|f| f.path).collect();
            let dirs = split_allowed(items, &allowed, &mut rep);
            absorb(&mut rep, match to_trash {
                Some(send) => clean::trash_with(&dirs, send, &tick),
                None => clean::delete_dirs_with(&dirs, &tick),
            });
            dirs.iter().for_each(|d| refresh(state, d));
        }
        Kind::Subfolders { .. } => {
            let allowed: Vec<String> = catalog::subfolder_paths(cat).iter().map(|p| scan::plain(p)).collect();
            for dir in split_allowed(items, &allowed, &mut rep) {
                let (f0, n0) = (rep.freed, rep.deleted);
                absorb(&mut rep, clean::delete_contents_except_with(&dir, age, &[], &|f, n| tick(f0 + f, n0 + n), to_trash));
                let _ = fs::remove_dir(scan::verbatim(&dir)); // solo desaparece si quedo vacia
                refresh(state, &dir);
            }
        }
        Kind::Containers | Kind::InfoOnly | Kind::Worktrees { .. } => return Err("unknown_category"),
    }
    emit(rep.freed, rep.deleted);
    Ok(rep)
}

// Solo rutas que estan en el escaneo actual; `clean::trash` aplica ademas el bloqueo de rutas.
pub fn trash_paths(state: &AppState, paths: &[String]) -> Res<Report> {
    let _work = begin_work(state)?;
    let mut rep = Report::default();
    if paths.is_empty() {
        return Ok(rep);
    }
    let allowed: Vec<PathBuf> = {
        let guard = state.lock();
        let result = guard.as_ref().ok_or("no_scan")?;
        paths
            .iter()
            .filter_map(|p| match scan::find(result, Path::new(p)) {
                // La raiz del escaneo no se manda a la papelera.
                Some(_) if scan::parts(Path::new(p)) != scan::parts(&result.root) => Some(PathBuf::from(p)),
                _ => {
                    rep.skipped.push(Skipped { path: p.clone(), reason: SkipReason::Blocked });
                    None
                }
            })
            .collect()
    };
    absorb(&mut rep, clean::trash(&allowed));
    allowed.iter().for_each(|p| refresh(state, p));
    Ok(rep)
}

// Los repositorios del escaneo actual; el candado se suelta antes de que git trabaje (puede tardar minutos).
fn worktree_repos(state: &AppState) -> Res<Vec<PathBuf>> {
    let guard = state.lock();
    let result = guard.as_ref().ok_or("no_scan")?;
    Ok(worktrees::repos(result, &state.no_projects))
}

// Antiguedad de la fila `git-worktrees` del catalogo (minimo 24 h, impuesto por effective_min_age).
fn worktree_age(catalog: &[Category], min_age_hours: Option<u64>) -> Res<std::time::Duration> {
    let cat = catalog.iter().find(|c| matches!(c.kind, Kind::Worktrees { .. })).ok_or("unknown_category")?;
    Ok(catalog::effective_min_age(cat, min_age_hours).unwrap_or(std::time::Duration::from_secs(24 * 3600)))
}

pub fn list_worktrees(state: &AppState, catalog: &[Category], min_age_hours: Option<u64>, conversations: Option<&Path>) -> Res<WorktreeList> {
    if state.running.load(Ordering::SeqCst) {
        return Err("scan_running");
    }
    let age = worktree_age(catalog, min_age_hours)?;
    let repos = worktree_repos(state)?;
    Ok(worktrees::list(&repos, &|p| path_size(state, p, &[]), age, conversations))
}

pub fn remove_worktrees(state: &AppState, catalog: &[Category], paths: &[String], min_age_hours: Option<u64>, conversations: Option<&Path>) -> Res<Report> {
    let _work = begin_work(state)?;
    let age = worktree_age(catalog, min_age_hours)?;
    let repos = worktree_repos(state)?;
    let (rep, removed) = worktrees::remove(&repos, &|p| path_size(state, p, &[]), age, conversations, paths);
    // Con la ruta del propio backend de cada worktree quitado, no con el texto que mando la interfaz.
    removed.iter().for_each(|p| refresh(state, p));
    Ok(rep)
}

// Carpeta de conversaciones de Claude Code: %USERPROFILE%\.claude\projects.
pub fn conversations_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(|u| PathBuf::from(u).join(".claude").join("projects"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CATALOG, Category, Kind};
    use std::fs::{self, File};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    const MB: usize = 1_048_576;

    // Categorias de prueba: apuntan a una carpeta temporal por variable de entorno propia de cada prueba.
    const SUB_CAT: Category = Category {
        id: "t-sub",
        ecosystem: "test",
        paths: &[r"%LIMPYRID_T6_SUB%"],
        kind: Kind::Subfolders { min_age_hours: 24 },
        exclude: &[],
    };
    const EXCL_CAT: Category = Category {
        id: "t-excl",
        ecosystem: "test",
        paths: &[r"%LIMPYRID_T6_EXCL%"],
        kind: Kind::Contents { min_age_hours: 0 },
        exclude: &["claude"],
    };

    fn file(p: &Path, bytes: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![0u8; bytes]).unwrap();
    }

    fn set_env(k: &str, v: &Path) {
        // SAFETY: cada prueba usa un nombre de variable propio; ninguna otra la lee.
        unsafe { std::env::set_var(k, v) };
    }

    fn state_with_scan(root: &Path) -> AppState {
        let s = AppState { no_projects: catalog::NoProjects::none(), ..AppState::default() };
        let r = scan::scan(root, &AtomicBool::new(false), &|_| {});
        *s.scan.lock().unwrap() = Some(r);
        s
    }

    fn s(p: &Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn sin_escaneo() {
        let st = AppState::default();
        assert_eq!(children(&st, r"C:\x").err(), Some("no_scan"));
        assert_eq!(treemap(&st, r"C:\x").err(), Some("no_scan"));
    }

    #[test]
    fn limpiar_durante_escaneo() {
        let st = AppState::default();
        st.running.store(true, Ordering::SeqCst);
        assert_eq!(clean_category(&st, &[], "npm", &[], None).err(), Some("scan_running"));
        assert_eq!(trash_paths(&st, &[]).err(), Some("scan_running"));
    }

    #[test]
    fn sin_escaneo_worktrees() {
        let st = AppState::default();
        assert_eq!(list_worktrees(&st, CATALOG, None, None).err(), Some("no_scan"));
        assert_eq!(remove_worktrees(&st, CATALOG, &[], None, None).err(), Some("no_scan"));
        st.running.store(true, Ordering::SeqCst);
        assert_eq!(list_worktrees(&st, CATALOG, None, None).err(), Some("scan_running"));
        assert_eq!(remove_worktrees(&st, CATALOG, &[], None, None).err(), Some("scan_running"));
        st.running.store(false, Ordering::SeqCst);
        let g = begin_work(&st).unwrap();
        assert_eq!(remove_worktrees(&st, CATALOG, &[], None, None).err(), Some("busy"));
        drop(g);
    }

    #[test]
    fn fila_de_worktrees() {
        const ROW: Category = Category { id: "git-worktrees", ecosystem: "git", paths: &[], kind: Kind::Worktrees { min_age_hours: 24 }, exclude: &[] };
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("q/.git/HEAD"), 1);
        let st = state_with_scan(t.path());
        assert!(categories(&st, &[ROW]).is_empty(), "sin repositorios con worktrees no hay fila");
        file(&t.path().join("p/.git/worktrees/x/gitdir"), 1);
        let st = state_with_scan(t.path());
        let info = categories(&st, &[ROW]);
        assert_eq!(info.len(), 1);
        assert_eq!((info[0].id, info[0].kind, info[0].size, info[0].uses_min_age), ("git-worktrees", "worktrees", 0, true));
        assert!(info[0].items.is_empty() && info[0].paths.is_empty());
        assert_eq!(clean_category(&st, &[ROW], "git-worktrees", &[], None).err(), Some("unknown_category"));
        assert_eq!(clean_category(&st, CATALOG, "git-worktrees", &[], None).err(), Some("unknown_category"));
    }

    #[test]
    fn categoria_desconocida() {
        let st = AppState::default();
        assert_eq!(clean_category(&st, CATALOG, "no-existe", &[], None).err(), Some("unknown_category"));
        assert_eq!(clean_category(&st, CATALOG, "docker", &[], None).err(), Some("unknown_category"));
        assert_eq!(clean_category(&st, CATALOG, "claude-code", &[], None).err(), Some("unknown_category"));
    }

    #[test]
    fn items_fuera_de_lista() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/package.json"), 10);
        file(&t.path().join("proj/node_modules/a.bin"), MB);
        file(&t.path().join("otro/node_modules/b.bin"), MB); // sin package.json: no es detectable
        let st = state_with_scan(t.path());
        let fuera = s(&t.path().join("otro/node_modules"));
        let rep = clean_category(&st, CATALOG, "node-projects", &[fuera.clone()], None).unwrap();
        assert!(t.path().join("otro/node_modules/b.bin").exists());
        assert!(rep.skipped.iter().any(|k| k.path == fuera && k.reason == SkipReason::Blocked));
        assert_eq!(rep.deleted, 0);

        let dentro = s(&t.path().join("proj/node_modules"));
        let rep = clean_category(&st, CATALOG, "node-projects", &[dentro], None).unwrap();
        assert!(!t.path().join("proj/node_modules").exists());
        assert_eq!(rep.deleted, 1);
    }

    #[test]
    fn papelera_actualiza_arbol() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/b/big.bin"), 2 * MB);
        file(&t.path().join("a/c.bin"), 2 * MB);
        let st = state_with_scan(t.path());
        let a = s(&t.path().join("a"));
        let before = treemap(&st, &a).unwrap().size;
        assert!(children(&st, &a).unwrap().iter().any(|e| e.name == "b"));
        let rep = trash_paths(&st, &[s(&t.path().join("a/b"))]).unwrap();
        assert_eq!(rep.deleted, 1);
        assert!(!children(&st, &a).unwrap().iter().any(|e| e.name == "b"));
        assert_eq!(treemap(&st, &a).unwrap().size, before - 2 * MB as u64);
    }

    #[test]
    fn papelera_solo_rutas_del_escaneo() {
        let t = tempfile::tempdir().unwrap();
        let u = tempfile::tempdir().unwrap();
        file(&t.path().join("a.bin"), 10);
        file(&u.path().join("ajeno.bin"), 10);
        let st = state_with_scan(t.path());
        let ajeno = s(&u.path().join("ajeno.bin"));
        let rep = trash_paths(&st, &[ajeno.clone()]).unwrap();
        assert!(u.path().join("ajeno.bin").exists());
        assert!(rep.skipped.iter().any(|k| k.path == ajeno && k.reason == SkipReason::Blocked));
    }

    #[test]
    fn papelera_ruta_desaparecida() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/b.bin"), 2 * MB);
        file(&t.path().join("a/c.bin"), 2 * MB);
        let st = state_with_scan(t.path());
        fs::remove_file(t.path().join("a/b.bin")).unwrap();
        let a = s(&t.path().join("a"));
        let rep = trash_paths(&st, &[s(&t.path().join("a/b.bin"))]).unwrap();
        assert_eq!(rep.deleted, 0);
        assert!(rep.skipped.is_empty());
        assert!(!children(&st, &a).unwrap().iter().any(|e| e.name == "b.bin"));
    }

    #[test]
    fn papelera_rechaza_raiz() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a.bin"), 10);
        let st = state_with_scan(t.path());
        let rep = trash_paths(&st, &[s(t.path())]).unwrap();
        assert!(t.path().join("a.bin").exists());
        assert_eq!(rep.skipped.len(), 1);
        assert_eq!(rep.skipped[0].reason, SkipReason::Blocked);
    }

    // La raiz escrita de otra forma sigue siendo la raiz: ni la papelera ni `find` deben discrepar.
    #[test]
    fn papelera_rechaza_raiz_con_otra_grafia() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a.bin"), 10);
        let st = state_with_scan(t.path());
        let root = s(t.path());
        for p in [root.replace('\\', "/"), format!(r"{root}\"), root.to_uppercase(), format!("//?/{}", root.replace('\\', "/"))] {
            let rep = trash_paths(&st, std::slice::from_ref(&p)).unwrap();
            assert!(t.path().join("a.bin").exists(), "{p}");
            assert_eq!((rep.deleted, rep.skipped.len()), (0, 1), "{p}");
            assert_eq!(rep.skipped[0].reason, SkipReason::Blocked);
        }
    }

    #[test]
    fn contenido_carpeta_desaparecida() {
        const GONE: Category = Category {
            id: "t-gone",
            ecosystem: "test",
            paths: &[r"%LIMPYRID_T6_GONE%"],
            kind: Kind::Contents { min_age_hours: 0 },
            exclude: &[],
        };
        let t = tempfile::tempdir().unwrap();
        let gone = t.path().join("ya-no-existe");
        set_env("LIMPYRID_T6_GONE", &gone);
        let rep = clean_category(&AppState::default(), &[GONE], "t-gone", &[], None).unwrap();
        assert_eq!((rep.deleted, rep.freed), (0, 0));
        assert!(categories(&AppState::default(), &[GONE]).is_empty());
    }

    #[test]
    fn subcarpetas_respeta_edad() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/vieja/x.bin"), 10);
        file(&t.path().join("proj/nueva/y.bin"), 10);
        // Las carpetas se abren con FILE_FLAG_BACKUP_SEMANTICS para poder fijar su mtime.
        // La edad mira todo el subarbol: archivo y carpeta deben ser viejos (de abajo arriba).
        let old = SystemTime::now() - Duration::from_secs(10 * 24 * 3600);
        for p in ["proj/vieja/x.bin", "proj/vieja"] {
            File::options().write(true).custom_flags(0x0200_0000).open(t.path().join(p)).unwrap().set_modified(old).unwrap();
        }
        set_env("LIMPYRID_T6_SUB", t.path());
        let st = AppState::default();
        let item = scan::plain(&catalog::subfolder_paths(&SUB_CAT).remove(0));
        let rep = clean_category(&st, &[SUB_CAT], "t-sub", &[item], None).unwrap();
        assert!(!t.path().join("proj/vieja").exists());
        assert!(t.path().join("proj/nueva").exists());
        assert!(t.path().join("proj").exists());
        assert!(rep.skipped.iter().any(|k| k.path.ends_with("nueva") && k.reason == SkipReason::TooRecent));
    }

    #[test]
    fn contenido_excluye() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("claude/x.bin"), 2 * MB);
        file(&t.path().join("otro.bin"), MB);
        set_env("LIMPYRID_T6_EXCL", t.path());
        let st = AppState::default();
        let info = categories(&st, &[EXCL_CAT]);
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].size, MB as u64);
        assert_eq!(info[0].kind, "contents");
        assert!(!info[0].uses_min_age);
        clean_category(&st, &[EXCL_CAT], "t-excl", &[], None).unwrap();
        assert!(t.path().join("claude/x.bin").exists());
        assert!(!t.path().join("otro.bin").exists());
    }

    #[test]
    fn ocupado_rechaza() {
        let st = AppState::default();
        let g = begin_work(&st).unwrap();
        assert_eq!(begin_work(&st).err(), Some("busy"));
        assert_eq!(clean_category(&st, CATALOG, "npm", &[], None).err(), Some("busy"));
        assert_eq!(trash_paths(&st, &[]).err(), Some("busy"));
        assert_eq!(begin_scan(&st).err(), Some("busy"));
        drop(g);
        assert!(begin_work(&st).is_ok(), "la marca se suelta con la guarda");
        let _scan = begin_scan(&st).unwrap();
        assert_eq!(begin_work(&st).err(), Some("scan_running"));
        assert!(!st.busy.load(Ordering::SeqCst), "un rechazo no deja la marca puesta");
    }

    #[test]
    fn ritmo_de_eventos() {
        assert!(!should_emit(0, 199));
        assert!(should_emit(0, 200));
        assert!(!should_emit(1000, 1150));
        assert!(should_emit(1000, 1250));
    }

    #[test]
    fn evento_final_con_el_informe() {
        const CAT: Category = Category {
            id: "t-prog",
            ecosystem: "test",
            paths: &[r"%LIMPYRID_T20_A%", r"%LIMPYRID_T20_B%"],
            kind: Kind::Contents { min_age_hours: 0 },
            exclude: &[],
        };
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        file(&a.path().join("x.bin"), 100);
        file(&b.path().join("y.bin"), 200);
        file(&b.path().join("z.bin"), 300);
        set_env("LIMPYRID_T20_A", a.path());
        set_env("LIMPYRID_T20_B", b.path());
        let seen = Mutex::new(vec![]);
        let rep = clean_category_with(&AppState::default(), &[CAT], "t-prog", &[], None, &|f, n| seen.lock().unwrap().push((f, n)), None).unwrap();
        let seen = seen.into_inner().unwrap();
        assert_eq!((rep.freed, rep.deleted), (600, 3));
        assert_eq!(seen.last(), Some(&(600, 3)), "el ultimo evento lleva el total de la categoria");
        assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 <= w[1].1), "{seen:?}");
        // Un rechazo no emite nada.
        let seen = Mutex::new(vec![]);
        assert!(clean_category_with(&AppState::default(), &[CAT], "no-existe", &[], None, &|f, n| seen.lock().unwrap().push((f, n)), None).is_err());
        assert!(seen.into_inner().unwrap().is_empty());
    }

    // Papelera simulada: rechaza `grande.bin` como Windows con lo que no cabe; el resto va a la de verdad.
    fn no_cabe(p: &Path) -> Result<(), trash::Error> {
        if p.ends_with("grande.bin") {
            return Err(trash::Error::Os { code: 0x8027_0037_u32 as i32, description: String::new() });
        }
        clean::recycle(p)
    }

    // Una categoria de contenido a la papelera: lo que no cabe se queda en el disco, en el informe y en el arbol.
    #[test]
    fn contenido_a_papelera_no_cabe() {
        const CAT: Category = Category { id: "t-pap", ecosystem: "test", paths: &[r"%LIMPYRID_T22_PAP%"], kind: Kind::Contents { min_age_hours: 0 }, exclude: &["claude"] };
        let t = tempfile::tempdir().unwrap();
        let long = fs::canonicalize(t.path()).unwrap(); // forma larga, como las rutas del catalogo
        file(&t.path().join("grande.bin"), 2 * MB);
        file(&t.path().join("chico/x.bin"), MB);
        file(&t.path().join("claude/y.bin"), MB);
        set_env("LIMPYRID_T22_PAP", t.path());
        let st = state_with_scan(&long);
        let seen = Mutex::new(vec![]);
        let rep = clean_category_with(&st, &[CAT], "t-pap", &[], None, &|f, n| seen.lock().unwrap().push((f, n)), Some(no_cabe)).unwrap();
        assert!(t.path().join("grande.bin").exists() && t.path().join("claude/y.bin").exists());
        assert!(!t.path().join("chico").exists());
        assert_eq!((rep.freed, rep.deleted), (MB as u64, 1));
        assert_eq!(rep.skipped.len(), 1);
        assert!(rep.skipped[0].path.ends_with("grande.bin") && rep.skipped[0].reason == SkipReason::TooLarge);
        assert_eq!(seen.into_inner().unwrap().last(), Some(&(MB as u64, 1)));
        assert_eq!(categories(&st, &[CAT])[0].size, 2 * MB as u64, "el arbol queda al dia: sigue lo que no cabia, sin lo excluido");
    }

    // Proyectos y subcarpetas a la papelera: las mismas listas permitidas y la misma antiguedad que el borrado.
    #[test]
    fn proyectos_y_subcarpetas_a_papelera() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/package.json"), 10);
        file(&t.path().join("proj/node_modules/a.bin"), MB);
        file(&t.path().join("otro/node_modules/b.bin"), MB); // sin package.json: no es detectable
        let st = state_with_scan(t.path());
        let (dentro, fuera) = (s(&t.path().join("proj/node_modules")), s(&t.path().join("otro/node_modules")));
        let rep = clean_category_with(&st, CATALOG, "node-projects", &[dentro, fuera.clone()], None, &|_, _| {}, Some(clean::recycle)).unwrap();
        assert!(!t.path().join("proj/node_modules").exists() && t.path().join("proj/package.json").exists());
        assert!(t.path().join("otro/node_modules/b.bin").exists(), "fuera de la lista no se toca");
        assert_eq!((rep.freed, rep.deleted), (MB as u64, 1));
        assert!(rep.skipped.iter().any(|k| k.path == fuera && k.reason == SkipReason::Blocked));

        const SUB: Category = Category { id: "t-sub-pap", ecosystem: "test", paths: &[r"%LIMPYRID_T22_SUB%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        let u = tempfile::tempdir().unwrap();
        file(&u.path().join("vieja/v/x.bin"), 10);
        file(&u.path().join("mixta/v/x.bin"), 10);
        file(&u.path().join("mixta/n/y.bin"), 10);
        let old = SystemTime::now() - Duration::from_secs(10 * 24 * 3600);
        for p in ["vieja/v/x.bin", "vieja/v", "mixta/v/x.bin", "mixta/v"] {
            File::options().write(true).custom_flags(0x0200_0000).open(u.path().join(p)).unwrap().set_modified(old).unwrap();
        }
        set_env("LIMPYRID_T22_SUB", u.path());
        let items: Vec<String> = catalog::subfolder_paths(&SUB).iter().map(|p| scan::plain(p)).collect();
        let rep = clean_category_with(&AppState::default(), &[SUB], "t-sub-pap", &items, None, &|_, _| {}, Some(clean::recycle)).unwrap();
        assert!(!u.path().join("vieja").exists(), "vacia tras enviar su contenido: desaparece");
        assert!(!u.path().join("mixta/v").exists() && u.path().join("mixta/n/y.bin").exists());
        assert_eq!((rep.freed, rep.deleted), (20, 2));
        assert!(rep.skipped.iter().any(|k| k.path.ends_with("n") && k.reason == SkipReason::TooRecent));
    }

    #[test]
    fn children_fuera_del_escaneo() {
        let t = tempfile::tempdir().unwrap();
        let u = tempfile::tempdir().unwrap();
        let st = state_with_scan(t.path());
        assert_eq!(children(&st, &s(u.path())).err(), Some("no_scan"));
    }

    #[test]
    fn cancelado_mide_la_ruta() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/x.bin"), MB);
        let st = state_with_scan(t.path());
        {
            // Como tras cancelar: la carpeta existe en el arbol pero no se visito.
            let mut g = st.scan.lock().unwrap();
            let r = g.as_mut().unwrap();
            r.cancelled = true;
            r.tree.children = vec![Node { name: "a".into(), size: 0, files: 0, is_dir: true, children: vec![], newest: 0 }];
            r.tree.size = 0;
        }
        assert_eq!(path_size(&st, &t.path().join("a"), &[]), MB as u64);
    }

    #[test]
    fn subcarpetas_usan_el_arbol() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/s1/x.bin"), MB);
        set_env("LIMPYRID_T6_SUB_ARBOL", &t.path().join("proj"));
        const CAT: Category = Category { id: "t-arbol", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SUB_ARBOL%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        // Raiz en forma larga, como las rutas del catalogo (el %TEMP% de las pruebas viene en forma 8.3).
        let st = state_with_scan(&fs::canonicalize(t.path()).unwrap());
        file(&t.path().join("proj/s1/y.bin"), MB); // despues del escaneo: no debe volver a medirse
        let info = categories(&st, &[CAT]);
        assert_eq!(info[0].items[0].size, MB as u64);
    }

    // La fecha de una subcarpeta sale del escaneo; fuera de el, del disco.
    #[test]
    fn subcarpetas_fecha_del_escaneo() {
        const KNOWN: u64 = 1_700_000_000;
        let set = |p: &Path, s: u64| {
            File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(s)).unwrap();
        };
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/s1/a/x.bin"), 10);
        for p in ["proj/s1/a/x.bin", "proj/s1/a", "proj/s1"] {
            set(&t.path().join(p), KNOWN);
        }
        set_env("LIMPYRID_T6_SUB_FECHA", &t.path().join("proj"));
        const CAT: Category = Category { id: "t-fecha", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SUB_FECHA%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        let st = state_with_scan(&fs::canonicalize(t.path()).unwrap());
        set(&t.path().join("proj/s1/a/x.bin"), KNOWN + 1000); // despues del escaneo: no debe leerse del disco
        assert_eq!(categories(&st, &[CAT])[0].items[0].modified, Some(KNOWN));

        let u = tempfile::tempdir().unwrap();
        file(&u.path().join("proj/s/y.bin"), 10);
        for p in ["proj/s/y.bin", "proj/s"] {
            set(&u.path().join(p), KNOWN);
        }
        set_env("LIMPYRID_T6_SUB_FUERA", &u.path().join("proj"));
        const OUT: Category = Category { id: "t-fuera", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SUB_FUERA%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        assert_eq!(categories(&st, &[OUT])[0].items[0].modified, Some(KNOWN));
    }

    // Dentro del escaneo, una fecha desconocida (0) se lee del disco.
    #[test]
    fn fecha_cero_cae_al_disco() {
        const KNOWN: u64 = 1_700_000_000;
        let set = |p: &Path, s: u64| {
            File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(s)).unwrap();
        };
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("proj/s1/x.bin"), 10);
        set_env("LIMPYRID_T6_SUB_CERO", &t.path().join("proj"));
        const CAT: Category = Category { id: "t-cero", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SUB_CERO%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        let st = state_with_scan(&fs::canonicalize(t.path()).unwrap());
        {
            let mut g = st.scan.lock().unwrap();
            let r = g.as_mut().unwrap();
            let proj = r.tree.children.iter_mut().find(|c| c.name == "proj").unwrap();
            proj.children[0].newest = 0; // como una carpeta que no se pudo leer
        }
        for p in ["proj/s1/x.bin", "proj/s1"] {
            set(&t.path().join(p), KNOWN);
        }
        assert_eq!(categories(&st, &[CAT])[0].items[0].modified, Some(KNOWN));
    }

    // Una ruta de categoria fuera del escaneo se recorre una vez y se recuerda hasta un refresh o un escaneo nuevo.
    #[test]
    fn fuera_del_escaneo_se_mide_una_vez() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        file(&a.path().join("x.bin"), 10);
        file(&b.path().join("y.bin"), MB);
        let b_long = fs::canonicalize(b.path()).unwrap(); // forma larga, como las rutas del catalogo
        set_env("LIMPYRID_T6_SIDE", b.path());
        const CAT: Category = Category { id: "t-side", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SIDE%"], kind: Kind::Contents { min_age_hours: 0 }, exclude: &[] };
        let st = state_with_scan(a.path());
        let size = || categories(&st, &[CAT])[0].size;
        assert_eq!(size(), MB as u64);
        file(&b.path().join("z.bin"), MB);
        assert_eq!(size(), MB as u64, "no vuelve a recorrer el disco");
        refresh(&st, &b_long.join("z.bin")); // una ruta dentro de la medida la invalida
        assert_eq!(size(), 2 * MB as u64);
        file(&b.path().join("w.bin"), MB);
        begin_scan(&st).unwrap();
        st.cancel.store(true, Ordering::SeqCst);
        run_scan(&st, a.path(), &|_| {});
        assert_eq!(size(), 2 * MB as u64, "un escaneo cancelado no la invalida");
        begin_scan(&st).unwrap();
        run_scan(&st, a.path(), &|_| {});
        assert_eq!(size(), 3 * MB as u64, "un escaneo completo nuevo si");
    }

    // Tras prune o compactar, la fila del motor se vuelve a medir, este su carpeta fuera o dentro del escaneo.
    #[test]
    fn contenedores_se_refrescan() {
        const CAT: Category = Category { id: "t-cont", ecosystem: "test", paths: &[r"%LIMPYRID_T6_CONT%"], kind: Kind::Containers, exclude: &[] };
        let (a, t) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let disk = t.path().join("wsl");
        file(&disk.join("d.vhdx"), MB);
        set_env("LIMPYRID_T6_CONT", &disk);
        let fuera = state_with_scan(a.path());
        let dentro = state_with_scan(&fs::canonicalize(t.path()).unwrap());
        for st in [&fuera, &dentro] {
            assert_eq!(categories(st, &[CAT])[0].size, MB as u64);
        }
        file(&disk.join("d.vhdx"), 2 * MB);
        for st in [&fuera, &dentro] {
            assert_eq!(categories(st, &[CAT])[0].size, MB as u64, "sin refrescar sigue la medida anterior");
            refresh_containers(st, &[CAT], "t-cont");
            assert_eq!(categories(st, &[CAT])[0].size, 2 * MB as u64);
        }
    }

    // Una medida empezada antes de invalidar `side` no se guarda despues.
    #[test]
    fn medida_vieja_no_se_guarda() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let st = state_with_scan(a.path());
        let tree = || scan::scan(b.path(), &AtomicBool::new(false), &|_| {}).tree;
        let key = PathBuf::from(scan::parts(b.path()).join("\\"));
        let seen = st.side_gen.load(Ordering::SeqCst);
        refresh(&st, b.path());
        remember(&st, key.clone(), tree(), seen);
        assert!(st.side().is_empty(), "tras refresh");
        let seen = st.side_gen.load(Ordering::SeqCst);
        begin_scan(&st).unwrap();
        run_scan(&st, a.path(), &|_| {});
        remember(&st, key.clone(), tree(), seen);
        assert!(st.side().is_empty(), "tras un escaneo nuevo");
        remember(&st, key, tree(), st.side_gen.load(Ordering::SeqCst));
        assert_eq!(st.side().len(), 1, "sin cambios, se guarda");
    }

    // Una ruta dentro de una carpeta ya recordada se responde desde su arbol, sin otro recorrido.
    #[test]
    fn fuera_del_escaneo_usa_la_carpeta_que_la_contiene() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        file(&b.path().join("sub/q.bin"), MB);
        set_env("LIMPYRID_T6_SIDE_PADRE", b.path());
        set_env("LIMPYRID_T6_SIDE_HIJO", &b.path().join("sub"));
        const PADRE: Category = Category { id: "t-padre", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SIDE_PADRE%"], kind: Kind::Contents { min_age_hours: 0 }, exclude: &[] };
        const HIJO: Category = Category { id: "t-hijo", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SIDE_HIJO%"], kind: Kind::Contents { min_age_hours: 0 }, exclude: &[] };
        let st = state_with_scan(a.path());
        assert_eq!(categories(&st, &[PADRE])[0].size, MB as u64);
        file(&b.path().join("sub/r.bin"), MB);
        assert_eq!(categories(&st, &[HIJO])[0].size, MB as u64);
    }

    // La fecha de una subcarpeta fuera del escaneo sale del mismo recorrido recordado.
    #[test]
    fn fecha_fuera_del_escaneo_recordada() {
        const KNOWN: u64 = 1_700_000_000;
        let set = |p: &Path, s: u64| {
            File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(s)).unwrap();
        };
        let (a, u) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        file(&u.path().join("proj/s/y.bin"), 10);
        for p in ["proj/s/y.bin", "proj/s"] {
            set(&u.path().join(p), KNOWN);
        }
        set_env("LIMPYRID_T6_SIDE_FECHA", &u.path().join("proj"));
        const CAT: Category = Category { id: "t-side-fecha", ecosystem: "test", paths: &[r"%LIMPYRID_T6_SIDE_FECHA%"], kind: Kind::Subfolders { min_age_hours: 24 }, exclude: &[] };
        let st = state_with_scan(a.path());
        assert_eq!(categories(&st, &[CAT])[0].items[0].modified, Some(KNOWN));
        set(&u.path().join("proj/s/y.bin"), KNOWN + 1000);
        assert_eq!(categories(&st, &[CAT])[0].items[0].modified, Some(KNOWN));
    }

    // Carpeta de proyecto con un hijo directo de fecha conocida: `modified` es esa fecha, la mas reciente del primer nivel.
    #[test]
    fn proyecto_con_fecha() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("p/package.json"), 1);
        file(&t.path().join("p/node_modules/paquete/index.js"), 1);
        let set = |p: &str, at: SystemTime| {
            File::options().write(true).custom_flags(0x0200_0000).open(t.path().join(p)).unwrap().set_modified(at).unwrap();
        };
        let known = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        set("p/node_modules/paquete", known);
        set("p/node_modules", known - Duration::from_secs(86_400));
        let st = state_with_scan(&fs::canonicalize(t.path()).unwrap());
        let c = CATALOG.iter().find(|c| c.id == "node-projects").unwrap();
        let info = categories(&st, &[Category { id: c.id, ecosystem: c.ecosystem, paths: c.paths, kind: c.kind, exclude: c.exclude }]);
        assert_eq!(info[0].items.len(), 1);
        assert_eq!(info[0].items[0].modified, Some(1_700_000_000));
    }

    #[test]
    fn rutas_denegadas() {
        let st = AppState::default();
        assert!(denied_paths(&st).is_empty());
        let t = tempfile::tempdir().unwrap();
        let st = state_with_scan(t.path());
        st.scan.lock().unwrap().as_mut().unwrap().denied = vec![PathBuf::from(r"C:\x\y")];
        assert_eq!(denied_paths(&st), [r"C:\x\y"]);
    }

    #[test]
    fn escaneo_unico_y_guarda() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a.bin"), 10);
        let st = AppState::default();
        assert!(begin_scan(&st).is_ok());
        assert_eq!(begin_scan(&st).err(), Some("scan_running"));
        let (cancelled, denied) = run_scan(&st, t.path(), &|_| {});
        assert_eq!((cancelled, denied), (false, 0));
        assert!(st.scan.lock().unwrap().is_some());
        assert!(begin_scan(&st).is_ok()); // la marca se limpio al terminar
    }

    #[test]
    fn cancelado_conserva_el_anterior() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        file(&a.path().join("x.bin"), MB);
        file(&b.path().join("y.bin"), 10);
        let st = state_with_scan(a.path());
        begin_scan(&st).unwrap();
        st.cancel.store(true, Ordering::SeqCst);
        assert!(run_scan(&st, b.path(), &|_| {}).0);
        assert!(begin_scan(&st).is_ok(), "la marca se limpio tambien al cancelar");
        let g = st.scan.lock().unwrap();
        let r = g.as_ref().unwrap();
        assert_eq!(r.root, a.path());
        assert!(!r.cancelled);
        assert_eq!(r.tree.size, MB as u64);
        drop(g);

        let st = AppState::default();
        begin_scan(&st).unwrap();
        st.cancel.store(true, Ordering::SeqCst);
        assert!(run_scan(&st, b.path(), &|_| {}).0);
        assert!(st.scan.lock().unwrap().is_none());
    }
}
