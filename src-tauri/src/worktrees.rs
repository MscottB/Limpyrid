// Worktrees de git: deteccion, reglas de "en uso" y retiro con `git worktree remove`.
// Unico comando de git que modifica algo: `worktree remove` (sin --force) dentro de `remove`; el resto solo lee.
use crate::catalog::NoProjects;
use crate::clean::{Report, SkipReason, Skipped, canon, is_blocked, newest_mtime};
use crate::scan::{Node, ScanResult, parts, plain, verbatim};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Read;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const REPARSE_POINT: u32 = 0x400;
const DIRECTORY: u32 = 0x10;
const READ_LIMIT: Duration = Duration::from_secs(60);
// Matar un `worktree remove` a medias deja la carpeta medio borrada y registrada (con ` D` ya no se puede quitar): mejor esperar.
const REMOVE_LIMIT: Duration = Duration::from_secs(900);
// Corta a proposito: solo lo que seguro se regenera con un build o una instalacion; todo lo demas ignorado protege.
const REBUILDABLE: [&str; 14] = [
    "node_modules", "target", "dist", "build", ".next", ".nuxt", ".svelte-kit", ".venv", "venv", "__pycache__", ".pytest_cache", ".turbo",
    ".cache", "coverage",
];

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeKind {
    ClaudeAgent,
    Claude,
    Git,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Protection {
    Uncommitted,
    Unreachable,
    Recent,
    Locked,
    Ignored,
    Unknown,
}

#[derive(Serialize)]
pub struct Worktree {
    pub path: String,
    pub repo: String,
    pub name: String,
    pub kind: WorktreeKind,
    pub branch: Option<String>,
    pub size: u64,
    pub modified: Option<u64>,
    pub protection: Vec<Protection>, // vacio = se puede quitar
}

#[derive(Serialize)]
pub struct Orphan {
    pub path: String,
    pub repo: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct WorktreeList {
    pub git: bool,
    pub worktrees: Vec<Worktree>,
    pub orphans: Vec<Orphan>,
}

pub struct Listed {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub detached: bool,
    pub locked: bool,
}

pub struct Facts {
    pub status_known: bool,
    pub dirty: bool,
    pub head_reachable: bool,
    pub locked: bool,
    pub ignored: bool,
    pub newest: Option<SystemTime>,
}

// Un repositorio es la carpeta que contiene `.git\worktrees` o `.claude\worktrees` con alguna carpeta dentro.
fn walk(node: &Node, path: &Path, parts: &mut Vec<String>, zones: &NoProjects, out: &mut Vec<PathBuf>) {
    let dir = |n: &Node, name: &str| n.children.iter().find(|c| c.is_dir && c.name.eq_ignore_ascii_case(name)).cloned();
    let wt_in = |outer: &str| dir(node, outer).and_then(|o| dir(&o, "worktrees")).is_some_and(|w| w.children.iter().any(|c| c.is_dir));
    if (wt_in(".git") || wt_in(".claude")) && !is_blocked(path) {
        out.push(path.to_path_buf());
    }
    // Lo de dentro de .git, node_modules y .claude (los propios worktrees) nunca son repositorios que listar.
    for c in node.children.iter().filter(|c| c.is_dir && ![".git", "node_modules", ".claude"].iter().any(|n| c.name.eq_ignore_ascii_case(n))) {
        parts.push(c.name.to_lowercase());
        if !zones.skips(parts) {
            walk(c, &path.join(&c.name), parts, zones, out);
        }
        parts.pop();
    }
}

pub fn repos(result: &ScanResult, zones: &NoProjects) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut p = canon(&result.root);
    if !zones.skips(&p) {
        walk(&result.tree, &result.root, &mut p, zones, &mut out);
    }
    out
}

// Bloques separados por linea vacia; el primero es la copia principal y los `bare` no tienen carpeta de trabajo.
pub fn parse_list(porcelain: &str) -> Vec<Listed> {
    let text = porcelain.replace("\r\n", "\n");
    text.split("\n\n")
        .skip(1)
        .filter_map(|block| {
            let (mut path, mut branch, mut detached, mut locked, mut bare) = (None, None, false, false, false);
            for line in block.lines() {
                if let Some(p) = line.strip_prefix("worktree ") {
                    path = Some(PathBuf::from(p.replace('/', "\\")));
                } else if let Some(b) = line.strip_prefix("branch ") {
                    branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
                } else if line == "detached" {
                    detached = true;
                } else if line == "locked" || line.starts_with("locked ") {
                    locked = true;
                } else if line == "bare" {
                    bare = true;
                }
            }
            path.filter(|_| !bare).map(|path| Listed { path, branch, detached, locked })
        })
        .collect()
}

pub fn classify(path: &Path) -> WorktreeKind {
    let p = parts(path);
    match p.windows(2).position(|w| w[0] == ".claude" && w[1] == "worktrees").and_then(|i| p.get(i + 2)) {
        Some(name) if name.starts_with("agent-") => WorktreeKind::ClaudeAgent,
        Some(_) => WorktreeKind::Claude,
        None => WorktreeKind::Git,
    }
}

// Nombre de la carpeta de conversaciones de Claude Code para un proyecto: todo lo que no es letra o digito ASCII pasa a `-`.
pub fn conversation_dir_name(path: &Path) -> String {
    plain(path).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

pub fn protections(facts: &Facts, cutoff: SystemTime) -> Vec<Protection> {
    [
        (!facts.status_known, Protection::Unknown),
        (facts.dirty, Protection::Uncommitted),
        (!facts.head_reachable, Protection::Unreachable),
        (facts.ignored, Protection::Ignored),
        (facts.locked, Protection::Locked),
        (facts.newest.is_none_or(|t| t > cutoff), Protection::Recent),
    ]
    .into_iter()
    .filter_map(|(on, p)| on.then_some(p))
    .collect()
}

// Sin --force: git rechaza por si mismo un worktree con cambios (segunda capa tras las comprobaciones propias).
pub fn remove_args(repo: &Path, path: &Path) -> Vec<String> {
    ["-c", "core.longpaths=true", "-C", &plain(repo), "worktree", "remove", &plain(path)].map(String::from).to_vec()
}

// Stdout de git si termina con exito antes del limite; si no, se mata el proceso y cuenta como fallo.
// Sin ventana, sin preguntas, sin tocar el indice al leer y sin programas que el repositorio configure (fsmonitor).
fn git_con_limite<S: AsRef<OsStr>>(args: &[S], limit: Duration) -> Option<String> {
    let mut child = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_NAMESPACE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + limit;
    let status = loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    };
    let out = reader.join().ok()?;
    status.success().then(|| String::from_utf8_lossy(&out).into_owned())
}

fn git<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    git_con_limite(args, READ_LIMIT)
}

// Salida de `status --porcelain=v1 -z` -> (hay cambios o archivos nuevos, hay ignorados que no se regeneran,
// carpetas ignoradas con nombre regenerable: rutas relativas, a comprobar con `has_git_inside`).
// Con -z las rutas no llevan comillas; `R`/`C` van seguidos de un campo extra (la ruta de origen).
pub fn read_status(porcelain: &str) -> (bool, bool, Vec<String>) {
    let (mut dirty, mut ignored, mut rebuildable) = (false, false, vec![]);
    let mut entries = porcelain.split('\0');
    while let Some(e) = entries.next() {
        let (Some(xy), Some(path)) = (e.get(..2), e.get(3..)) else { continue };
        if xy == "!!" {
            let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_lowercase();
            // git muestra una carpeta ignorada entera con `/` final; un archivo ignorado nunca es regenerable.
            if path.ends_with('/') && REBUILDABLE.contains(&last.as_str()) {
                rebuildable.push(path.trim_end_matches('/').to_string());
            } else {
                ignored = true;
            }
        } else {
            dirty = true;
            if xy.contains(['R', 'C']) {
                entries.next();
            }
        }
    }
    (dirty, ignored, rebuildable)
}

// Algo llamado `.git` (archivo o carpeta) dentro de `dir`: un repositorio o worktree anidado no se regenera.
// No entra en carpetas reparse point (pnpm llena node_modules de junctions); lo que no se puede leer cuenta como hallado.
fn has_git_inside(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(verbatim(dir)) else { return true };
    for e in rd {
        let Ok(e) = e else { return true };
        if e.file_name() == ".git" {
            return true;
        }
        let Ok(m) = e.metadata() else { return true };
        if m.file_attributes() & (REPARSE_POINT | DIRECTORY) == DIRECTORY && has_git_inside(&e.path()) {
            return true;
        }
    }
    false
}

fn list_repo(repo: &Path) -> Option<Vec<Listed>> {
    git(&["-C", &plain(repo), "worktree", "list", "--porcelain"]).map(|t| parse_list(&t))
}

// Ruta explicita en todas las opciones: la configuracion del usuario o del repo (showUntrackedFiles=no,
// ignoreSubmodules) no puede ocultar archivos nuevos.
fn status_of(wt: &str) -> Option<(bool, bool)> {
    let out = git(&["-C", wt, "status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=none", "--ignored=matching"])?;
    let (dirty, ignored, rebuildable) = read_status(&out);
    Some((dirty, ignored || rebuildable.iter().any(|d| has_git_inside(&Path::new(wt).join(d)))))
}

// mtime del .jsonl mas reciente de las conversaciones del worktree.
fn latest_conversation(dir: &Path) -> Option<SystemTime> {
    fs::read_dir(verbatim(dir))
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("jsonl")))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

// `siblings`: las demas carpetas de trabajo del repositorio (worktrees listados y copia principal).
fn facts(l: &Listed, siblings: &[PathBuf], cutoff: SystemTime, conversations: Option<&Path>) -> Facts {
    let wt = plain(&l.path);
    let status = status_of(&wt);
    // Solo ramas, etiquetas y remotos: las refs de bisect, stash o `refs/worktree` mueren con la carpeta.
    let head_reachable = l.branch.is_some()
        || git(&["-C", &wt, "for-each-ref", "--contains", "HEAD", "--count=1", "refs/heads/", "refs/tags/", "refs/remotes/"])
            .is_some_and(|o| !o.trim().is_empty());
    let me = parts(&l.path);
    // Un worktree dentro de otro (p. ej. en una carpeta ignorada) se perderia con el contenedor.
    let nested = siblings.iter().map(|s| parts(s)).any(|s| s != me && s.starts_with(&me));
    let own = newest_mtime(&verbatim(&l.path), Some(cutoff));
    let conv = conversations.and_then(|c| latest_conversation(&c.join(conversation_dir_name(&l.path))));
    Facts {
        status_known: status.is_some(),
        dirty: status.is_some_and(|s| s.0),
        head_reachable,
        locked: l.locked,
        ignored: nested || status.is_some_and(|s| s.1),
        newest: own.map(|o| conv.map_or(o, |c| o.max(c))),
    }
}

fn cutoff_for(min_age: Duration) -> SystemTime {
    SystemTime::now().checked_sub(min_age).unwrap_or(UNIX_EPOCH)
}

fn siblings(repo: &Path, listed: &[Listed]) -> Vec<PathBuf> {
    listed.iter().map(|l| l.path.clone()).chain([repo.to_path_buf()]).collect()
}

// `size` mide una ruta (del arbol del escaneo si esta; si no, escaneandola).
pub fn list(repos: &[PathBuf], size: &dyn Fn(&Path) -> u64, min_age: Duration, conversations: Option<&Path>) -> WorktreeList {
    let mut out = WorktreeList { git: git(&["--version"]).is_some(), worktrees: vec![], orphans: vec![] };
    if !out.git {
        return out;
    }
    let cutoff = cutoff_for(min_age);
    for repo in repos {
        // Si git no puede leer el repositorio no se sabe que es huerfano: no se muestra nada de el.
        let Some(listed) = list_repo(repo) else { continue };
        let sibs = siblings(repo, &listed);
        for l in listed.iter().filter(|l| fs::symlink_metadata(verbatim(&l.path)).is_ok()) {
            let f = facts(l, &sibs, cutoff, conversations);
            out.worktrees.push(Worktree {
                path: plain(&l.path),
                repo: plain(repo),
                name: l.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                kind: classify(&l.path),
                branch: l.branch.clone(),
                size: size(&l.path),
                modified: f.newest.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
                protection: protections(&f, cutoff),
            });
        }
        let known: Vec<_> = listed.iter().map(|l| canon(&l.path)).collect();
        let Ok(rd) = fs::read_dir(verbatim(&repo.join(".claude").join("worktrees"))) else { continue };
        for e in rd.flatten().filter(|e| e.metadata().is_ok_and(|m| m.file_attributes() & (REPARSE_POINT | DIRECTORY) == DIRECTORY)) {
            let path = PathBuf::from(plain(&e.path()));
            if !known.contains(&canon(&path)) {
                out.orphans.push(Orphan { size: size(&path), path: plain(&path), repo: plain(repo) });
            }
        }
    }
    out
}

// Busca `want` en la lista que git da ahora de los repositorios (primero los que contienen la ruta) y descarta lo
// bloqueado o enlazado; devuelve el repositorio, el worktree y las demas carpetas de trabajo.
fn find_listed(repos: &[PathBuf], want: &[String]) -> Option<(PathBuf, Listed, Vec<PathBuf>)> {
    let (near, far): (Vec<_>, Vec<_>) = repos.iter().partition(|r| want.starts_with(&parts(r)));
    near.into_iter()
        .chain(far)
        .find_map(|repo| {
            let all = list_repo(repo)?;
            let sibs = siblings(repo, &all);
            let l = all.into_iter().find(|l| parts(&l.path) == want)?;
            Some((repo.clone(), l, sibs))
        })
        .filter(|(_, l, _)| !is_blocked(&l.path) && fs::symlink_metadata(verbatim(&l.path)).is_ok_and(|m| m.file_attributes() & REPARSE_POINT == 0))
}

// Devuelve el informe y las rutas (las del propio backend) de los worktrees quitados.
pub fn remove(
    repos: &[PathBuf],
    size: &dyn Fn(&Path) -> u64,
    min_age: Duration,
    conversations: Option<&Path>,
    paths: &[String],
) -> (Report, Vec<PathBuf>) {
    remove_with(repos, size, min_age, conversations, paths, &|_| {})
}

// `between` corre justo antes de la ultima comprobacion (las pruebas escriben ahi para simular actividad tardia).
fn remove_with(
    repos: &[PathBuf],
    size: &dyn Fn(&Path) -> u64,
    min_age: Duration,
    conversations: Option<&Path>,
    paths: &[String],
    between: &dyn Fn(&Path),
) -> (Report, Vec<PathBuf>) {
    let (mut rep, mut removed) = (Report::default(), vec![]);
    let cutoff = cutoff_for(min_age);
    for req in paths {
        // La ruta pedida solo elige entre lo que git lista ahora; a git se le pasa la ruta del propio backend.
        let want = parts(Path::new(req));
        let Some((_, first, _)) = find_listed(repos, &want) else {
            rep.skipped.push(Skipped { path: req.clone(), reason: SkipReason::Blocked });
            continue;
        };
        // Lo lento (medir) va antes: entre las comprobaciones de uso y `worktree remove` no hay nada mas.
        let bytes = size(&first.path);
        let Some((repo, l, sibs)) = find_listed(repos, &want) else {
            rep.skipped.push(Skipped { path: req.clone(), reason: SkipReason::Blocked });
            continue;
        };
        if !protections(&facts(&l, &sibs, cutoff, conversations), cutoff).is_empty() {
            rep.skipped.push(Skipped { path: req.clone(), reason: SkipReason::InUse });
            continue;
        }
        between(&l.path);
        // Ultima comprobacion: git no mira los ignorados, y el recorrido de actividad de `facts` es lento.
        if status_of(&plain(&l.path)) != Some((false, false)) {
            rep.skipped.push(Skipped { path: req.clone(), reason: SkipReason::InUse });
            continue;
        }
        if git_con_limite(&remove_args(&repo, &l.path), REMOVE_LIMIT).is_some() {
            rep.freed += bytes;
            rep.deleted += 1;
            removed.push(l.path);
        } else {
            let changes = fs::symlink_metadata(verbatim(&l.path)).is_ok() && status_of(&plain(&l.path)) != Some((false, false));
            rep.skipped.push(Skipped { path: req.clone(), reason: if changes { SkipReason::InUse } else { SkipReason::Other } });
        }
    }
    (rep, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clean::SkipReason;
    use crate::scan::{parts, scan};
    use std::fs::{self, File};
    use std::os::windows::fs::OpenOptionsExt;
    use std::process::Command;
    use std::sync::atomic::AtomicBool;

    const OPTS: [&str; 8] = ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main"];

    // Todo git de estas pruebas corre dentro de un tempdir propio, con identidad por argumento.
    // Configuracion global vacia y sin la del sistema, solo para los procesos hijos (el archivo real no se toca).
    fn isolate() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let empty = tempfile::NamedTempFile::new().unwrap().into_temp_path();
            // SAFETY: se fija una vez por proceso de pruebas (Once) y los hijos de git la heredan; el codigo de produccion nunca la fija.
            unsafe {
                std::env::set_var("GIT_CONFIG_GLOBAL", &empty);
                std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            }
            std::mem::forget(empty);
        });
    }

    fn g(dir: &Path, args: &[&str]) -> String {
        isolate();
        let out = Command::new("git").args(OPTS).arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn repo_in(root: &Path, name: &str) -> PathBuf {
        let r = root.join(name);
        fs::create_dir_all(&r).unwrap();
        g(&r, &["init"]);
        fs::write(r.join("a.txt"), "a").unwrap();
        g(&r, &["add", "."]);
        g(&r, &["commit", "-m", "uno"]);
        r
    }

    fn add_wt(repo: &Path, name: &str, extra: &[&str]) -> PathBuf {
        let p = repo.join(".claude").join("worktrees").join(name);
        let ps = p.to_string_lossy().into_owned();
        let mut args = vec!["worktree", "add"];
        args.extend(extra);
        args.push(&ps);
        g(repo, &args);
        p
    }

    fn scanned(root: &Path) -> ScanResult {
        scan(root, &AtomicBool::new(false), &|_| {})
    }

    fn tree_size(result: &ScanResult) -> impl Fn(&Path) -> u64 + '_ {
        move |p| crate::scan::find(result, p).map_or_else(|| scan(p, &AtomicBool::new(false), &|_| {}).tree.size, |n| n.size)
    }

    fn run_list(root: &Path, min_age: Duration, conv: Option<&Path>) -> WorktreeList {
        let r = scanned(root);
        list(&repos(&r, &NoProjects::none()), &tree_size(&r), min_age, conv)
    }

    fn by_name<'a>(l: &'a WorktreeList, n: &str) -> &'a Worktree {
        l.worktrees.iter().find(|w| w.name == n).unwrap_or_else(|| panic!("falta {n}"))
    }

    fn age(p: &Path) {
        let old = SystemTime::now() - Duration::from_secs(10 * 24 * 3600);
        File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(old).unwrap();
    }

    #[test]
    fn parsea_porcelana() {
        let text = "worktree C:/r\nHEAD aaa\nbranch refs/heads/main\n\n\
                    worktree C:/r/.claude/worktrees/a b\nHEAD bbb\nbranch refs/heads/feat/x\n\n\
                    worktree C:/r/.claude/worktrees/agent-1\nHEAD ccc\ndetached\n\n\
                    worktree C:/r/otro\nHEAD ddd\nbranch refs/heads/b\nlocked razon larga\n";
        let l = parse_list(text);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].path, PathBuf::from(r"C:\r\.claude\worktrees\a b"));
        assert_eq!((l[0].branch.as_deref(), l[0].detached, l[0].locked), (Some("feat/x"), false, false));
        assert_eq!((l[1].branch.as_deref(), l[1].detached, l[1].locked), (None, true, false));
        assert_eq!((l[2].branch.as_deref(), l[2].detached, l[2].locked), (Some("b"), false, true));
        assert!(l.iter().all(|w| w.path != PathBuf::from(r"C:\r")));
        assert!(parse_list("worktree C:/r\nHEAD a\nbare\n\nworktree C:/w\nHEAD b\nbare\n").is_empty());
    }

    #[test]
    fn nombre_de_conversacion() {
        let p = Path::new(r"C:\Users\dev\Proyectos\tienda\.claude\worktrees\brave-tu-39aed5");
        assert_eq!(conversation_dir_name(p), "C--Users-dev-Proyectos-tienda--claude-worktrees-brave-tu-39aed5");
    }

    #[test]
    fn clasifica() {
        assert_eq!(classify(Path::new(r"C:\p\.claude\worktrees\agent-a1b2")), WorktreeKind::ClaudeAgent);
        assert_eq!(classify(Path::new(r"C:\p\.claude\worktrees\brave-tu-39aed5")), WorktreeKind::Claude);
        assert_eq!(classify(Path::new(r"C:\p-feature")), WorktreeKind::Git);
        assert_eq!(classify(Path::new(r"C:\p\agent-x")), WorktreeKind::Git);
    }

    #[test]
    fn protecciones() {
        let now = SystemTime::now();
        let old = now - Duration::from_secs(3600);
        let ok = || Facts { status_known: true, dirty: false, head_reachable: true, locked: false, ignored: false, newest: Some(old) };
        assert!(protections(&ok(), now).is_empty());
        assert_eq!(protections(&Facts { dirty: true, ..ok() }, now), [Protection::Uncommitted]);
        assert_eq!(protections(&Facts { head_reachable: false, ..ok() }, now), [Protection::Unreachable]);
        assert_eq!(protections(&Facts { locked: true, ..ok() }, now), [Protection::Locked]);
        assert_eq!(protections(&Facts { ignored: true, ..ok() }, now), [Protection::Ignored]);
        assert_eq!(protections(&Facts { newest: None, ..ok() }, now), [Protection::Recent]);
        assert_eq!(protections(&Facts { newest: Some(now + Duration::from_secs(5)), ..ok() }, now), [Protection::Recent]);
        assert_eq!(protections(&Facts { status_known: false, ..ok() }, now), [Protection::Unknown]);
        let all = Facts { status_known: true, dirty: true, head_reachable: false, locked: true, ignored: true, newest: None };
        assert_eq!(
            protections(&all, now),
            [Protection::Uncommitted, Protection::Unreachable, Protection::Ignored, Protection::Locked, Protection::Recent]
        );
    }

    #[test]
    fn args_sin_force() {
        let a = remove_args(Path::new(r"C:\r"), Path::new(r"C:\r\.claude\worktrees\x y"));
        for want in ["worktree", "remove", r"C:\r\.claude\worktrees\x y", "core.longpaths=true"] {
            assert!(a.iter().any(|s| s == want), "{want}");
        }
        assert!(a.iter().all(|s| s != "--force" && s != "-f"));
    }

    #[test]
    fn repos_desde_el_arbol() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join(r"p\.git\worktrees\x")).unwrap();
        fs::write(t.path().join(r"p\.git\worktrees\x\gitdir"), "x").unwrap();
        fs::create_dir_all(t.path().join(r"q\.git")).unwrap();
        fs::write(t.path().join(r"q\.git\HEAD"), "x").unwrap();
        let r = repos(&scanned(t.path()), &NoProjects::none());
        assert_eq!(r.iter().map(|p| parts(p)).collect::<Vec<_>>(), [parts(&t.path().join("p"))]);
    }

    #[test]
    fn lista_y_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        add_wt(&repo, "limpio", &["-b", "br-limpio"]);
        let nuevo = add_wt(&repo, "nuevo", &["-b", "br-nuevo"]);
        fs::write(nuevo.join("n.txt"), "x").unwrap();
        let suelto = add_wt(&repo, "suelto", &["--detach"]);
        fs::write(suelto.join("s.txt"), "s").unwrap();
        g(&suelto, &["add", "."]);
        g(&suelto, &["commit", "-m", "huerfano"]);
        let bloq = add_wt(&repo, "bloq", &["-b", "br-bloq"]);
        g(&repo, &["worktree", "lock", bloq.to_str().unwrap()]);

        let l = run_list(t.path(), Duration::ZERO, None);
        assert!(l.git && l.orphans.is_empty());
        assert_eq!(l.worktrees.len(), 4);
        assert!(by_name(&l, "limpio").protection.is_empty());
        assert_eq!(by_name(&l, "limpio").branch.as_deref(), Some("br-limpio"));
        assert_eq!(by_name(&l, "limpio").kind, WorktreeKind::Claude);
        assert_eq!(by_name(&l, "nuevo").protection, [Protection::Uncommitted]);
        assert_eq!(by_name(&l, "suelto").protection, [Protection::Unreachable]);
        assert_eq!(by_name(&l, "suelto").branch, None);
        assert_eq!(by_name(&l, "bloq").protection, [Protection::Locked]);
        assert!(by_name(&l, "limpio").size > 0 && by_name(&l, "limpio").modified.is_some());
    }

    #[test]
    fn actividad_reciente_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        add_wt(&repo, "limpio", &["-b", "br-limpio"]);
        let l = run_list(t.path(), Duration::from_secs(24 * 3600), None);
        assert_eq!(by_name(&l, "limpio").protection, [Protection::Recent]);
    }

    #[test]
    fn conversacion_reciente_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        let wt = add_wt(&repo, "limpio", &["-b", "br-limpio"]);
        age(&wt.join(".git"));
        age(&wt.join("a.txt"));
        age(&wt);
        let conv = t.path().join("conv");
        let day = Duration::from_secs(24 * 3600);
        let l = run_list(t.path(), day, Some(&conv));
        assert!(by_name(&l, "limpio").protection.is_empty(), "sin conversacion y con archivos viejos se puede quitar");
        // El nombre sale de la ruta tal como la lista git; se cubre tambien la forma en que la escribio la prueba.
        for p in [PathBuf::from(&by_name(&l, "limpio").path), wt.clone()] {
            let d = conv.join(conversation_dir_name(&p));
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("s.jsonl"), "{}").unwrap();
        }
        let l = run_list(t.path(), day, Some(&conv));
        assert_eq!(by_name(&l, "limpio").protection, [Protection::Recent]);
    }

    #[test]
    fn quita_solo_los_libres() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        let limpio = add_wt(&repo, "limpio", &["-b", "br-limpio"]);
        let nuevo = add_wt(&repo, "nuevo", &["-b", "br-nuevo"]);
        fs::write(nuevo.join("n.txt"), "x").unwrap();
        let otra = t.path().join("otra");
        fs::create_dir_all(&otra).unwrap();
        fs::write(otra.join("f.txt"), "x").unwrap();

        let result = scanned(t.path());
        let (rs, size) = (repos(&result, &NoProjects::none()), tree_size(&result));
        let l = list(&rs, &size, Duration::ZERO, None);
        let req = vec![by_name(&l, "limpio").path.clone(), by_name(&l, "nuevo").path.clone(), otra.to_string_lossy().into_owned()];
        let (rep, removed) = remove(&rs, &size, Duration::ZERO, None, &req);
        assert_eq!(removed.len(), 1);
        assert!(!limpio.exists());
        assert!(g(&repo, &["branch", "--list", "br-limpio"]).contains("br-limpio"));
        assert!(nuevo.join("n.txt").exists());
        assert!(otra.join("f.txt").exists());
        assert_eq!(rep.deleted, 1);
        assert!(rep.freed > 0);
        let reason = |p: &str| rep.skipped.iter().find(|s| parts(Path::new(&s.path)) == parts(Path::new(p))).map(|s| &s.reason);
        assert_eq!(reason(&req[1]), Some(&SkipReason::InUse));
        assert_eq!(reason(&req[2]), Some(&SkipReason::Blocked));
        assert_eq!(rep.skipped.len(), 2);
    }

    #[test]
    fn huerfanas() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        let suelta = repo.join(r".claude\worktrees\suelta");
        fs::create_dir_all(&suelta).unwrap();
        fs::write(suelta.join("f.txt"), "x").unwrap();
        let result = scanned(t.path());
        let (rs, size) = (repos(&result, &NoProjects::none()), tree_size(&result));
        let l = list(&rs, &size, Duration::ZERO, None);
        assert!(l.worktrees.is_empty());
        assert_eq!(l.orphans.len(), 1);
        assert!(l.orphans[0].path.ends_with("suelta") && l.orphans[0].size > 0);
        let (rep, _) = remove(&rs, &size, Duration::ZERO, None, &[l.orphans[0].path.clone()]);
        assert!(suelta.join("f.txt").exists());
        assert_eq!((rep.deleted, rep.skipped.len()), (0, 1));
        assert_eq!(rep.skipped[0].reason, SkipReason::Blocked);
    }

    #[test]
    fn lee_estado() {
        let flags = |s: &str| {
            let (d, i, _) = read_status(s);
            (d, i)
        };
        let nul = |v: &[&str]| v.join("\0") + "\0";
        assert_eq!(flags(""), (false, false));
        assert_eq!(flags(&nul(&["?? sub dir/nuevo archivo.txt"])), (true, false));
        assert_eq!(flags(&nul(&[" M a.txt"])), (true, false));
        // Renombrado: el campo extra (origen) no se lee como entrada.
        assert_eq!(flags(&nul(&["R  nuevo.txt", "!! origen.txt"])), (true, false));
        // Carpetas regenerables ignoradas no protegen; archivos u otras carpetas si.
        assert_eq!(flags(&nul(&["!! node_modules/", "!! web app/Target/", "!! a/.venv/"])), (false, false));
        assert_eq!(flags(&nul(&["!! .env"])), (false, true));
        assert_eq!(flags(&nul(&["!! target"])), (false, true), "un archivo llamado target no es una carpeta");
        assert_eq!(flags(&nul(&["!! .claude/worktrees/"])), (false, true));
        assert_eq!(flags(&nul(&["!! node_modules/", "!! x y/notas \"raras\".txt"])), (false, true));
        assert_eq!(read_status(&nul(&["!! node_modules/", "!! web app/Target/", "!! .env"])).2, ["node_modules", "web app/Target"]);
    }

    #[test]
    fn archivos_nuevos_con_config_oculta() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        g(&repo, &["config", "status.showUntrackedFiles", "no"]);
        let wt = add_wt(&repo, "nuevo", &["-b", "br-nuevo"]);
        fs::write(wt.join("solo nuevo.txt"), "x").unwrap();
        assert_eq!(g(&wt, &["status", "--porcelain"]).trim(), "", "la config oculta el archivo nuevo");
        let l = run_list(t.path(), Duration::ZERO, None);
        assert_eq!(by_name(&l, "nuevo").protection, [Protection::Uncommitted]);
    }

    #[test]
    fn ignorado_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        fs::write(repo.join(".gitignore"), ".env\nnode_modules/\n").unwrap();
        g(&repo, &["add", "."]);
        g(&repo, &["commit", "-m", "ignore"]);
        let wt = add_wt(&repo, "con-env", &["-b", "br-env"]);
        fs::write(wt.join(".env"), "SECRETO=1").unwrap();
        let result = scanned(t.path());
        let (rs, size) = (repos(&result, &NoProjects::none()), tree_size(&result));
        let l = list(&rs, &size, Duration::ZERO, None);
        assert_eq!(by_name(&l, "con-env").protection, [Protection::Ignored]);
        let (rep, removed) = remove(&rs, &size, Duration::ZERO, None, &[by_name(&l, "con-env").path.clone()]);
        assert!(wt.join(".env").exists() && removed.is_empty());
        assert_eq!((rep.deleted, rep.skipped.len()), (0, 1));
        assert_eq!(rep.skipped[0].reason, SkipReason::InUse);
    }

    #[test]
    fn ignorado_regenerable_no_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        fs::write(repo.join(".gitignore"), "node_modules/\n").unwrap();
        g(&repo, &["add", "."]);
        g(&repo, &["commit", "-m", "ignore"]);
        let wt = add_wt(&repo, "con-modulos", &["-b", "br-mod"]);
        fs::create_dir_all(wt.join("node_modules")).unwrap();
        fs::write(wt.join("node_modules").join("x.js"), "x").unwrap();
        let l = run_list(t.path(), Duration::ZERO, None);
        assert!(by_name(&l, "con-modulos").protection.is_empty());
    }

    #[test]
    fn anidado_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        fs::write(repo.join(".gitignore"), ".claude/worktrees/\n").unwrap();
        g(&repo, &["add", "."]);
        g(&repo, &["commit", "-m", "ignore"]);
        let x = add_wt(&repo, "x", &["-b", "br-x"]);
        g(&x, &["worktree", "add", "-b", "br-y", x.join(r".claude\worktrees\y").to_str().unwrap()]);
        let l = run_list(t.path(), Duration::ZERO, None);
        assert!(by_name(&l, "x").protection.contains(&Protection::Ignored));
        assert!(!by_name(&l, "y").protection.contains(&Protection::Ignored));
    }

    #[test]
    fn ref_efimera_no_cuenta() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        let wt = add_wt(&repo, "suelto", &["--detach"]);
        fs::write(wt.join("s.txt"), "s").unwrap();
        g(&wt, &["add", "."]);
        g(&wt, &["commit", "-m", "solo en ref efimera"]);
        g(&wt, &["update-ref", "refs/worktree/x", "HEAD"]);
        assert!(g(&wt, &["for-each-ref", "--contains", "HEAD", "--count=1"]).contains("refs/worktree/x"));
        let l = run_list(t.path(), Duration::ZERO, None);
        assert_eq!(by_name(&l, "suelto").protection, [Protection::Unreachable]);
    }

    #[test]
    fn git_con_limite_corta() {
        isolate();
        assert!(git_con_limite(&["--version"], Duration::ZERO).is_none());
        assert!(git_con_limite(&["--version"], Duration::from_secs(60)).is_some());
    }

    #[test]
    fn busca_git_dentro() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join(r"a\b\c")).unwrap();
        fs::write(t.path().join(r"a\b\c\f.txt"), "x").unwrap();
        assert!(!has_git_inside(&t.path().join("a")));
        fs::write(t.path().join(r"a\b\c\.git"), "gitdir: x").unwrap();
        assert!(has_git_inside(&t.path().join("a")));
        // Un .git solo detras de una junction no se ve: no se entra y su destino esta fuera.
        let fuera = tempfile::tempdir().unwrap();
        fs::write(fuera.path().join(".git"), "gitdir: x").unwrap();
        fs::create_dir_all(t.path().join("j")).unwrap();
        let out = Command::new("cmd").args(["/c", "mklink", "/J"]).arg(t.path().join(r"j\enlace")).arg(fuera.path()).output().unwrap();
        assert!(out.status.success(), "mklink fallo");
        assert!(!has_git_inside(&t.path().join("j")));
        assert!(has_git_inside(&t.path().join("no-existe")), "lo ilegible cuenta como hallado");
    }

    #[test]
    fn repo_anidado_en_regenerable_protege() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        g(&repo, &["add", "."]);
        g(&repo, &["commit", "-m", "ignore"]);
        let wt = add_wt(&repo, "d", &["-b", "br-d"]);
        let _otro = repo_in(&wt.join("target"), "x");
        assert_eq!(g(&wt, &["status", "--porcelain", "--ignored=matching"]).trim(), "!! target/");
        let l = run_list(t.path(), Duration::ZERO, None);
        assert_eq!(by_name(&l, "d").protection, [Protection::Ignored]);
    }

    #[test]
    fn actividad_tardia_se_revisa_al_final() {
        let t = tempfile::tempdir().unwrap();
        let repo = repo_in(t.path(), "r");
        fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        g(&repo, &["add", "."]);
        g(&repo, &["commit", "-m", "ignore"]);
        let wt = add_wt(&repo, "tarde", &["-b", "br-tarde"]);
        let result = scanned(t.path());
        let (rs, size) = (repos(&result, &NoProjects::none()), tree_size(&result));
        let l = list(&rs, &size, Duration::ZERO, None);
        assert!(by_name(&l, "tarde").protection.is_empty());
        let req = [by_name(&l, "tarde").path.clone()];
        // Un .env escrito despues de las comprobaciones de `facts` y antes de `worktree remove`.
        let (rep, removed) = remove_with(&rs, &size, Duration::ZERO, None, &req, &|p| fs::write(p.join(".env"), "S=1").unwrap());
        assert!(wt.join(".env").exists() && removed.is_empty());
        assert_eq!(rep.skipped[0].reason, SkipReason::InUse);
    }
}
