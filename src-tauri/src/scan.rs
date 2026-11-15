// Escaneo de disco a un árbol de tamaños en memoria.
use rayon::prelude::*;
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub const SMALL_FILE_LIMIT: u64 = 1_048_576;
pub const SMALL_FILES: &str = "<small>";
const REPARSE_POINT: u32 = 0x400;
const DIRECTORY: u32 = 0x10;

#[derive(Clone, Serialize)]
pub struct Node {
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
    pub children: Vec<Node>, // ordenados por size descendente
    // Mtime mas reciente del elemento y de todo lo que contiene, en segundos Unix; 0 = desconocido. No va a la interfaz.
    #[serde(skip)]
    pub newest: u64,
}

pub struct ScanResult {
    pub root: PathBuf,
    pub tree: Node,
    pub denied: Vec<PathBuf>,
    pub cancelled: bool,
}

#[derive(Clone, Serialize)]
pub struct Progress {
    pub dirs: u64,
    pub bytes: u64,
    pub current: String,
}

struct Ctx<'a> {
    cancel: &'a AtomicBool,
    on_progress: &'a (dyn Fn(Progress) + Sync),
    dirs: AtomicU64,
    bytes: AtomicU64,
    denied: Mutex<Vec<PathBuf>>,
}

// Prefijo \\?\ para superar el límite de 260 caracteres en rutas con unidad.
pub(crate) fn verbatim(p: &Path) -> PathBuf {
    // Los verbatim no normalizan `/`: se convierte a `\` antes de mirar el prefijo y de anteponer \\?\.
    let units: Vec<u16> = p.as_os_str().encode_wide().map(|c| if c == b'/' as u16 { b'\\' as u16 } else { c }).collect();
    let norm = OsString::from_wide(&units);
    if p.is_absolute() && !norm.to_string_lossy().starts_with(r"\\") {
        let mut s = OsString::from(r"\\?\");
        s.push(norm);
        PathBuf::from(s)
    } else {
        PathBuf::from(norm)
    }
}

// Ruta sin el prefijo \\?\ (una sola vez) para mostrarla o compararla con rutas del usuario.
pub(crate) fn plain(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

fn sort_desc(children: &mut [Node]) {
    children.sort_by(|a, b| b.size.cmp(&a.size));
}

fn secs(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

fn dir_node(name: String, own: u64, mut children: Vec<Node>) -> Node {
    sort_desc(&mut children);
    Node {
        name,
        size: children.iter().map(|c| c.size).sum(),
        files: children.iter().map(|c| c.files).sum(),
        is_dir: true,
        newest: children.iter().map(|c| c.newest).fold(own, u64::max),
        children,
    }
}

fn leaf(name: String, size: u64, files: u64, newest: u64) -> Node {
    Node { name, size, files, is_dir: false, children: vec![], newest }
}

// `mtime`: el de la propia carpeta, leido por el padre (o por `scan` para la raiz).
fn scan_dir(path: &Path, name: String, mtime: u64, ctx: &Ctx) -> Node {
    if ctx.cancel.load(Ordering::Relaxed) {
        return dir_node(name, 0, vec![]);
    }
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            if e.kind() == ErrorKind::PermissionDenied {
                ctx.denied.lock().unwrap().push(PathBuf::from(plain(path)));
            }
            return dir_node(name, 0, vec![]);
        }
    };
    let mut children = Vec::new();
    let mut subdirs = Vec::new();
    let (mut small_size, mut small_files, mut small_newest) = (0u64, 0u64, 0u64);
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let attrs = meta.file_attributes();
        // Solo se omiten directorios reparse (junctions/symlinks): los archivos con reparse tag
        // (OneDrive, WOF/CompactOS, dedup) son archivos reales y deben contar.
        if attrs & REPARSE_POINT != 0 && attrs & DIRECTORY != 0 {
            continue;
        }
        let ename = entry.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            subdirs.push((entry.path(), ename, secs(&meta)));
        } else if meta.len() >= SMALL_FILE_LIMIT {
            children.push(leaf(ename, meta.len(), 1, secs(&meta)));
        } else {
            small_size += meta.len();
            small_files += 1;
            small_newest = small_newest.max(secs(&meta));
        }
    }
    if small_files > 0 {
        children.push(leaf(SMALL_FILES.into(), small_size, small_files, small_newest));
    }
    let own: u64 = children.iter().map(|c| c.size).sum();
    let bytes = ctx.bytes.fetch_add(own, Ordering::Relaxed) + own;
    let dirs = ctx.dirs.fetch_add(1, Ordering::Relaxed) + 1;
    (ctx.on_progress)(Progress { dirs, bytes, current: plain(path) });
    children.extend(subdirs.into_par_iter().map(|(p, n, t)| scan_dir(&p, n, t, ctx)).collect::<Vec<_>>());
    dir_node(name, mtime, children)
}

pub fn scan(root: &Path, cancel: &AtomicBool, on_progress: &(dyn Fn(Progress) + Sync)) -> ScanResult {
    let ctx = Ctx {
        cancel,
        on_progress,
        dirs: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        denied: Mutex::new(vec![]),
    };
    let name = root.file_name().unwrap_or(root.as_os_str()).to_string_lossy().into_owned();
    let mtime = fs::metadata(verbatim(root)).map_or(0, |m| secs(&m));
    let tree = scan_dir(&verbatim(root), name, mtime, &ctx);
    ScanResult {
        root: root.to_path_buf(),
        tree,
        denied: ctx.denied.into_inner().unwrap(),
        cancelled: cancel.load(Ordering::Relaxed),
    }
}

// Identidad de ruta para toda la app: componentes en minúsculas, sin prefijo \\?\ ni separadores repetidos.
pub(crate) fn parts(p: &Path) -> Vec<String> {
    let s = p.to_string_lossy().replace('/', "\\");
    s.strip_prefix(r"\\?\")
        .unwrap_or(&s)
        .split('\\')
        .filter(|c| !c.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn rel_parts(result: &ScanResult, path: &Path) -> Option<Vec<String>> {
    let root = parts(&result.root);
    parts(path).strip_prefix(root.as_slice()).map(<[String]>::to_vec)
}

pub fn find<'a>(result: &'a ScanResult, path: &Path) -> Option<&'a Node> {
    let mut node = &result.tree;
    for part in rel_parts(result, path)? {
        node = node.children.iter().find(|c| c.name.to_lowercase() == part)?;
    }
    Some(node)
}

fn remove_in(node: &mut Node, rel: &[String]) -> Option<(u64, u64)> {
    let idx = node.children.iter().position(|c| c.name.to_lowercase() == rel[0])?;
    let (size, files) = if rel.len() == 1 {
        let c = node.children.remove(idx);
        (c.size, c.files)
    } else {
        let r = remove_in(&mut node.children[idx], &rel[1..])?;
        sort_desc(&mut node.children);
        r
    };
    node.size -= size;
    node.files -= files;
    Some((size, files))
}

pub fn remove(result: &mut ScanResult, path: &Path) -> bool {
    match rel_parts(result, path) {
        Some(rel) if !rel.is_empty() => remove_in(&mut result.tree, &rel).is_some(),
        _ => false,
    }
}

pub fn pruned(node: &Node, depth: u8, max_children: usize) -> Node {
    let mut out = Node { children: vec![], ..node.clone() };
    if depth == 0 {
        return out;
    }
    out.children = node.children.iter().take(max_children).map(|c| pruned(c, depth - 1, max_children)).collect();
    let rest = node.children.iter().skip(max_children);
    if rest.len() > 0 {
        let (size, files, newest) = rest.fold((0, 0, 0), |(s, f, n), c| (s + c.size, f + c.files, n.max(c.newest)));
        out.children.push(leaf(SMALL_FILES.into(), size, files, newest));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStringExt;
    use std::process::Command;

    const MB: u64 = 1_048_576;

    fn file(path: &Path, bytes: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes as usize]).unwrap();
    }

    fn run(root: &Path) -> ScanResult {
        scan(root, &AtomicBool::new(false), &|_| {})
    }

    #[test]
    fn suma_tamanos() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/x.bin"), 2 * MB);
        file(&t.path().join("a/b/y.bin"), 3 * MB);
        let r = run(t.path());
        assert_eq!(r.tree.size, 5 * MB);
        assert_eq!(r.tree.files, 2);
        assert_eq!(find(&r, &t.path().join("a")).unwrap().children[0].name, "b");
    }

    #[test]
    fn agrupa_pequenos() {
        let t = tempfile::tempdir().unwrap();
        for i in 0..3 {
            file(&t.path().join(format!("s{i}.txt")), 100);
        }
        file(&t.path().join("big.bin"), MB);
        let r = run(t.path());
        let small = r.tree.children.iter().find(|c| c.name == SMALL_FILES).unwrap();
        assert_eq!((small.size, small.files, small.is_dir), (300, 3, false));
        assert!(r.tree.children.iter().any(|c| c.name == "big.bin" && c.size == MB));
    }

    #[test]
    fn no_sigue_junction() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("real/big.bin"), 2 * MB);
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(t.path().join("link"))
            .arg(t.path().join("real"))
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink fallo");
        assert_eq!(run(t.path()).tree.size, 2 * MB);
    }

    #[test]
    fn ruta_larga() {
        let t = tempfile::tempdir().unwrap();
        let mut p = t.path().to_path_buf();
        while p.as_os_str().len() < 300 {
            p.push("d".repeat(50));
        }
        assert!(p.as_os_str().len() > 260);
        let mut v = OsString::from(r"\\?\");
        v.push(p.as_os_str());
        file(&Path::new(&v).join("f.bin"), MB);
        assert_eq!(run(t.path()).tree.size, MB);
    }

    #[test]
    fn raiz_con_barras() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/x.bin"), 2 * MB);
        let slashes = PathBuf::from(t.path().to_string_lossy().replace('\\', "/"));
        assert_eq!(run(&slashes).tree.size, run(t.path()).tree.size);
        assert_eq!(run(&slashes).tree.size, 2 * MB);
    }

    #[test]
    fn progreso_sin_prefijo() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/x.bin"), MB);
        let seen = Mutex::new(Vec::new());
        scan(t.path(), &AtomicBool::new(false), &|p| seen.lock().unwrap().push(p.current));
        let seen = seen.into_inner().unwrap();
        assert!(!seen.is_empty());
        assert!(seen.iter().all(|c| !c.starts_with(r"\\?\")));
    }

    #[test]
    fn raiz_inexistente() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(run(&t.path().join("no_existe")).tree.size, 0);
    }

    #[test]
    fn nombre_no_unicode() {
        let t = tempfile::tempdir().unwrap();
        let name = OsString::from_wide(&[0x61, 0xD800]);
        fs::write(t.path().join(name), b"x").unwrap();
        assert_eq!(run(t.path()).tree.files, 1);
    }

    #[test]
    fn find_normaliza() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/B/f.bin"), MB);
        let r = run(t.path());
        let n = find(&r, &t.path().join("a").join("B")).unwrap() as *const Node;
        for p in [r"a\b\", "A/b", r"a\B"] {
            assert_eq!(find(&r, &t.path().join(p)).unwrap() as *const Node, n);
        }
    }

    #[test]
    fn prefijo_una_sola_vez() {
        assert_eq!(plain(Path::new(r"\\?\C:\x")), r"C:\x");
        assert_eq!(plain(Path::new(r"\\?\\\?\C:\x")), r"\\?\C:\x");
        assert_eq!(verbatim(Path::new("//?/C:/x")), PathBuf::from(r"\\?\C:\x"));
        assert_eq!(parts(Path::new("//?/C:/x/")), parts(Path::new(r"c:\X")));
    }

    #[test]
    fn remove_resta() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("a/b/y.bin"), 3 * MB);
        file(&t.path().join("a/x.bin"), 2 * MB);
        let mut r = run(t.path());
        assert!(remove(&mut r, &t.path().join("a/b")));
        assert_eq!(r.tree.size, 2 * MB);
        assert!(find(&r, &t.path().join("a/b")).is_none());
    }

    #[test]
    fn cancelado() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("x.bin"), 10);
        let r = scan(t.path(), &AtomicBool::new(true), &|_| {});
        assert!(r.cancelled);
        assert_eq!(r.tree.newest, 0, "una carpeta sin visitar no tiene fecha");
    }

    #[test]
    fn fecha_mas_reciente() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::{Duration, UNIX_EPOCH};
        const OLD: u64 = 1_600_000_000;
        const MID: u64 = 1_650_000_000;
        const NEW: u64 = 1_700_000_000;
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("viejo.txt"), 10);
        file(&t.path().join("grande.bin"), SMALL_FILE_LIMIT);
        file(&t.path().join("nueva/n.txt"), 10);
        file(&t.path().join("vieja/v.txt"), 10);
        fs::create_dir(t.path().join("vacia")).unwrap();
        // Las carpetas se abren con FILE_FLAG_BACKUP_SEMANTICS para poder fijar su mtime.
        let set = |p: &Path, s: u64| {
            fs::File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(UNIX_EPOCH + Duration::from_secs(s)).unwrap()
        };
        for (p, s) in [("viejo.txt", OLD), ("grande.bin", MID), ("nueva/n.txt", NEW), ("vieja/v.txt", OLD), ("nueva", OLD - 1), ("vieja", OLD - 1), ("vacia", MID)] {
            set(&t.path().join(p), s);
        }
        set(t.path(), OLD - 1);
        let r = run(t.path());
        assert_eq!(r.tree.newest, NEW);
        assert_eq!(find(&r, &t.path().join("nueva")).unwrap().newest, NEW);
        assert_eq!(find(&r, &t.path().join("vieja")).unwrap().newest, OLD);
        assert_eq!(find(&r, &t.path().join("grande.bin")).unwrap().newest, MID);
        assert_eq!(find(&r, &t.path().join("vacia")).unwrap().newest, MID, "el mtime propio de la carpeta cuenta");
        assert_eq!(r.tree.children.iter().find(|c| c.name == SMALL_FILES).unwrap().newest, OLD);
        assert!(serde_json::to_value(&r.tree).unwrap().get("newest").is_none());
    }

    #[test]
    fn pruned_recorta() {
        let t = tempfile::tempdir().unwrap();
        for i in 0..10u64 {
            file(&t.path().join(format!("d{i}/sub/f.bin")), (i + 1) * MB);
        }
        let r = run(t.path());
        let p = pruned(&r.tree, 1, 3);
        assert_eq!(p.children.len(), 4);
        assert!(p.children.iter().all(|c| c.children.is_empty()));
        let small = p.children.last().unwrap();
        assert_eq!(small.name, SMALL_FILES);
        assert_eq!(small.size, (1..=7).sum::<u64>() * MB);
    }
}
