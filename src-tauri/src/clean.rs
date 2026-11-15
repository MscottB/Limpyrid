// Borrado seguro: rutas bloqueadas, borrado definitivo, papelera e informe.
use crate::scan::{plain, verbatim};
use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    InUse,
    Denied,
    Blocked,
    TooRecent,
    TooLarge, // no cabe en la Papelera de reciclaje
    Other,
}

#[derive(Serialize)]
pub struct Skipped {
    pub path: String,
    pub reason: SkipReason,
}

#[derive(Serialize, Default)]
pub struct Report {
    pub freed: u64,
    pub deleted: u64,
    pub skipped: Vec<Skipped>,
}

const REPARSE_POINT: u32 = 0x400;
const DIRECTORY: u32 = 0x10;
const SHARING_VIOLATION: i32 = 32;
const LOCK_VIOLATION: i32 = 33;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLongPathNameW(short: *const u16, long: *mut u16, len: u32) -> u32;
}

// Componentes sin prefijo \\?\, con `.` y `..` resueltos (evita `x\..\Windows`).
fn clean(p: &Path) -> Vec<String> {
    let s = p.to_string_lossy().replace('/', "\\");
    let mut out: Vec<String> = vec![];
    for c in s.trim_start_matches(r"\\?\").split('\\').filter(|c| !c.is_empty() && *c != ".") {
        if c == ".." {
            out.pop();
        } else {
            out.push(c.to_string());
        }
    }
    out
}

// Forma larga (sin nombres 8.3) del prefijo existente mas largo; no sigue enlaces, asi el ultimo
// componente que sea un enlace se compara como tal. Si nada existe, queda la forma lexica.
pub(crate) fn long_name(path: &str) -> Option<String> {
    let wide: Vec<u16> = format!(r"\\?\{path}").encode_utf16().chain([0]).collect();
    let mut buf = vec![0u16; 32768];
    let n = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
}

// Componentes en minusculas y en forma larga.
pub(crate) fn canon(p: &Path) -> Vec<String> {
    let parts = clean(p);
    let lower = |v: &[String]| v.iter().map(|c| c.to_lowercase()).collect::<Vec<_>>();
    for k in (1..=parts.len()).rev() {
        let mut prefix = parts[..k].join("\\");
        if k == 1 {
            prefix.push('\\');
        }
        if let Some(long) = long_name(&prefix) {
            let mut out = clean(Path::new(&long));
            out.extend_from_slice(&parts[k..]);
            return lower(&out);
        }
    }
    lower(&parts)
}

// shortcut: lexico, no resuelve junctions dentro de la ruta ni unidades de `subst` o de red mapeadas (una letra
// que apunta a otra carpeta o a otro equipo se juzga por su texto); los llamadores solo pasan rutas del escaneo
// (que nunca entra en junctions) o del catalogo. Mejorar si se admiten raices de escaneo en esas unidades.
pub fn is_blocked(path: &Path) -> bool {
    blocked_with(path, &|k| std::env::var_os(k).map(|v| canon(Path::new(&v))).filter(|v| !v.is_empty()))
}

// Solo rutas con letra de unidad (`X:\`, con o sin un prefijo \\?\): UNC, \\?\UNC\ y \\.\ quedan fuera.
fn drive_rooted(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('/', "\\");
    let b = s.strip_prefix(r"\\?\").unwrap_or(&s).as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

// `env` entrega cada raiz protegida ya canonica. Si falta una obligatoria, todo queda bloqueado.
fn blocked_with(path: &Path, env: &dyn Fn(&str) -> Option<Vec<String>>) -> bool {
    if !path.is_absolute() || !drive_rooted(path) {
        return true;
    }
    let p = canon(path);
    if p.is_empty() || (p.len() == 1 && p[0].ends_with(':')) {
        return true;
    }
    let (Some(sys), Some(pf), Some(pd), Some(user)) =
        (env("SystemRoot"), env("ProgramFiles"), env("ProgramData"), env("USERPROFILE"))
    else {
        return true;
    };
    let pf86 = env("ProgramFiles(x86)"); // puede faltar en Windows de 32 bits
    let roots = [Some(&sys), Some(&pf), pf86.as_ref(), Some(&pd), Some(&user)];
    // Un ancestro de una ubicacion protegida (p. ej. C:\Users) tambien se bloquea.
    if roots.iter().flatten().any(|r| r.starts_with(&p)) {
        return true;
    }
    if p.starts_with(&sys) {
        let mut temp = sys;
        temp.push("temp".into());
        return !(p.starts_with(&temp) && p.len() > temp.len());
    }
    if [Some(&pf), pf86.as_ref(), Some(&pd)].iter().flatten().any(|r| p.starts_with(r)) {
        return true;
    }
    if p.starts_with(&user) {
        let rel: Vec<&str> = p[user.len()..].iter().map(String::as_str).collect();
        return matches!(rel[..], [] | ["appdata"] | ["appdata", "local" | "roaming" | "locallow"]);
    }
    false
}

fn reason(e: &io::Error) -> SkipReason {
    match e.raw_os_error() {
        Some(SHARING_VIOLATION | LOCK_VIOLATION) => SkipReason::InUse,
        _ if e.kind() == ErrorKind::PermissionDenied => SkipReason::Denied,
        _ => SkipReason::Other,
    }
}

impl Report {
    fn skip(&mut self, path: &Path, reason: SkipReason) {
        self.skipped.push(Skipped { path: plain(path), reason });
    }

    fn blocked(&mut self, path: &Path) -> bool {
        let b = is_blocked(path);
        if b {
            self.skip(path, SkipReason::Blocked);
        }
        b
    }
}

// Si el sistema niega el acceso, quita solo lectura y reintenta una vez.
fn remove(path: &Path, is_dir: bool) -> io::Result<()> {
    let op = || if is_dir { fs::remove_dir(path) } else { fs::remove_file(path) };
    match op() {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            if let Ok(m) = fs::symlink_metadata(path) {
                let mut perm = m.permissions();
                perm.set_readonly(false);
                let _ = fs::set_permissions(path, perm);
            }
            op()
        }
        r => r,
    }
}

// Avisador de progreso: (bytes liberados, elementos borrados) acumulados en la llamada. Solo observa: se llama despues
// de anotar cada borrado en el informe y nada de lo que haga cambia que se borra.
pub type OnProgress<'a> = &'a (dyn Fn(u64, u64) + Sync);
const QUIET: OnProgress = &|_, _| {};

// Borra una entrada archivo por archivo. Un reparse point de carpeta se quita sin entrar.
fn remove_entry(path: &Path, rep: &mut Report, on: OnProgress) {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => return rep.skip(path, reason(&e)),
    };
    let attrs = meta.file_attributes();
    if attrs & DIRECTORY == 0 {
        match remove(path, false) {
            Ok(()) => {
                rep.freed += meta.len();
                rep.deleted += 1;
                on(rep.freed, rep.deleted);
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => rep.skip(path, reason(&e)),
        }
        return;
    }
    let skipped_before = rep.skipped.len();
    if attrs & REPARSE_POINT == 0 {
        match fs::read_dir(path) {
            Ok(entries) => entries.flatten().for_each(|e| remove_entry(&e.path(), rep, on)),
            Err(e) if e.kind() == ErrorKind::NotFound => return,
            Err(e) => return rep.skip(path, reason(&e)),
        }
    }
    match remove(path, true) {
        Err(e) if e.kind() != ErrorKind::NotFound && rep.skipped.len() == skipped_before => rep.skip(path, reason(&e)),
        _ => {} // si falla por hijos omitidos, ya constan en el informe
    }
}

// Mtime mas reciente de `path` y todo lo que contiene, sin entrar en carpetas reparse point. Deja de bajar en cuanto
// encuentra algo posterior a `stop`. None si algo no se puede leer: quien juzga la edad lo trata como reciente.
pub(crate) fn newest_mtime(path: &Path, stop: Option<SystemTime>) -> Option<SystemTime> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mut newest = meta.modified().ok()?;
    if meta.file_attributes() & (DIRECTORY | REPARSE_POINT) != DIRECTORY {
        return Some(newest);
    }
    for entry in fs::read_dir(path).ok()? {
        if stop.is_some_and(|s| newest > s) {
            break;
        }
        newest = newest.max(newest_mtime(&entry.ok()?.path(), stop)?);
    }
    Some(newest)
}

// Carpeta bloqueada pero con contenido limpiable: solo %SystemRoot%\Temp.
fn contents_allowed(dir: &Path) -> bool {
    !is_blocked(dir) || std::env::var_os("SystemRoot").is_some_and(|r| canon(dir) == canon(&Path::new(&r).join("Temp")))
}

pub fn delete_contents(dir: &Path, min_age: Option<Duration>) -> Report {
    delete_contents_except(dir, min_age, &[])
}

// `exclude`: nombres de primer nivel (sin distinguir mayusculas) que no se visitan: ni se borran, ni cuentan, ni se listan.
pub fn delete_contents_except(dir: &Path, min_age: Option<Duration>, exclude: &[&str]) -> Report {
    delete_contents_except_with(dir, min_age, exclude, QUIET)
}

pub fn delete_contents_except_with(dir: &Path, min_age: Option<Duration>, exclude: &[&str], on: OnProgress) -> Report {
    let mut rep = Report::default();
    // Una carpeta de catalogo que es un enlace se rechaza: su destino real nunca pasa por is_blocked.
    let is_link = fs::symlink_metadata(verbatim(dir)).is_ok_and(|m| m.file_attributes() & REPARSE_POINT != 0);
    if !contents_allowed(dir) || is_link {
        rep.skip(dir, SkipReason::Blocked);
        return rep;
    }
    let entries = match fs::read_dir(verbatim(dir)) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return rep,
        Err(e) => {
            rep.skip(dir, reason(&e));
            return rep;
        }
    };
    for entry in entries.flatten() {
        if exclude.iter().any(|n| entry.file_name().to_string_lossy().eq_ignore_ascii_case(n)) {
            continue;
        }
        let path = entry.path();
        if rep.blocked(&path) {
            continue;
        }
        if let Some(min) = min_age {
            // El mtime de una carpeta solo cambia al crear o quitar hijos directos: se mira todo el subarbol.
            let cutoff = SystemTime::now().checked_sub(min).unwrap_or(SystemTime::UNIX_EPOCH);
            if newest_mtime(&path, Some(cutoff)).is_none_or(|t| t > cutoff) {
                rep.skip(&path, SkipReason::TooRecent);
                continue;
            }
        }
        remove_entry(&path, &mut rep, on);
    }
    rep
}

pub fn delete_dirs(dirs: &[PathBuf]) -> Report {
    delete_dirs_with(dirs, QUIET)
}

pub fn delete_dirs_with(dirs: &[PathBuf], on: OnProgress) -> Report {
    let mut rep = Report::default();
    for dir in dirs {
        if !rep.blocked(dir) {
            remove_entry(&verbatim(dir), &mut rep, on);
        }
    }
    rep
}

// (bytes, archivos) sin entrar en carpetas reparse point.
fn measure(path: &Path) -> (u64, u64) {
    let Ok(meta) = fs::symlink_metadata(path) else { return (0, 0) };
    let attrs = meta.file_attributes();
    if attrs & DIRECTORY == 0 {
        return (meta.len(), 1);
    }
    if attrs & REPARSE_POINT != 0 {
        return (0, 0);
    }
    let Ok(entries) = fs::read_dir(path) else { return (0, 0) };
    entries.flatten().map(|e| measure(&e.path())).fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}

// El crate `trash` (5.x) entrega el HRESULT del motor de copia del shell por elemento (sherrors.h) o un error Win32.
fn trash_reason(e: &trash::Error) -> SkipReason {
    let trash::Error::Os { code, .. } = e else { return SkipReason::Other };
    match *code as u32 {
        0x8027_0037 => SkipReason::TooLarge, // COPYENGINE_E_RECYCLE_SIZE_TOO_BIG
        0x8027_0027 | 0x8007_0020 | 0x8007_0021 => SkipReason::InUse, // SHARING_VIOLATION_SRC, ERROR_SHARING/LOCK_VIOLATION
        0x8027_0021 | 0x8027_003F | 0x8027_0002 | 0x8007_0005 => SkipReason::Denied, // ACCESS_DENIED_SRC, READONLY, REQUIRES_ELEVATION
        _ => SkipReason::Other,
    }
}

pub fn trash(paths: &[PathBuf]) -> Report {
    let mut rep = Report::default();
    for p in paths {
        if rep.blocked(p) {
            continue;
        }
        let v = verbatim(p);
        if matches!(fs::symlink_metadata(&v), Err(e) if e.kind() == ErrorKind::NotFound) {
            continue;
        }
        let (bytes, files) = measure(&v);
        // La papelera de Windows no acepta el prefijo \\?\.
        match trash::delete(plain(&v)) {
            Ok(()) => {
                rep.freed += bytes;
                rep.deleted += files;
            }
            Err(e) => rep.skip(p, trash_reason(&e)),
        }
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs::{self, File};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::OpenOptionsExt;
    use std::process::Command;
    use std::time::SystemTime;

    fn env(k: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(k).unwrap())
    }

    fn file(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn is_empty_dir(p: &Path) -> bool {
        fs::read_dir(p).unwrap().next().is_none()
    }

    fn assert_one_blocked(r: &Report) {
        assert_eq!((r.deleted, r.freed, r.skipped.len()), (0, 0, 1));
        assert_eq!(r.skipped[0].reason, SkipReason::Blocked);
    }

    #[test]
    fn bloquea_sistema() {
        let sys = env("SystemRoot");
        for p in [
            PathBuf::from(r"C:\"),
            sys.clone(),
            sys.join(r"System32\x"),
            env("ProgramFiles").join("x"),
            env("ProgramData").join("x"),
            env("USERPROFILE"),
            env("LOCALAPPDATA"),
            env("USERPROFILE").parent().unwrap().to_path_buf(),
            PathBuf::from("resources"),
        ] {
            assert!(is_blocked(&p), "{p:?}");
        }
    }

    unsafe extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, len: u32) -> u32;
    }

    fn short(p: &Path) -> PathBuf {
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain([0]).collect();
        let mut buf = vec![0u16; 32768];
        let n = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        assert!(n > 0, "GetShortPathNameW fallo");
        PathBuf::from(OsString::from_wide(&buf[..n]))
    }

    #[test]
    fn bloquea_nombres_cortos() {
        let (user, pf) = (env("USERPROFILE"), env("ProgramFiles"));
        assert!(is_blocked(&short(&user)) && is_blocked(&short(&pf).join("x")));
        // Con 8.3 desactivado la forma corta coincide con la larga y las afirmaciones siguen valiendo.
        assert!(contents_allowed(&env("TEMP")));
        assert!(contents_allowed(&short(&env("TEMP"))));
        assert!(!contents_allowed(&short(&user)));
    }

    #[test]
    fn sin_variables_bloquea_todo() {
        let t = tempfile::tempdir().unwrap();
        let real = |k: &str| std::env::var_os(k).map(|v| canon(Path::new(&v))).filter(|v| !v.is_empty());
        assert!(!blocked_with(t.path(), &real));
        for missing in ["SystemRoot", "ProgramFiles", "ProgramData", "USERPROFILE"] {
            assert!(blocked_with(t.path(), &|k| if k == missing { None } else { real(k) }), "{missing}");
        }
        assert!(!blocked_with(t.path(), &|k| if k == "ProgramFiles(x86)" { None } else { real(k) }));
    }

    #[test]
    fn contenido_error_se_informa() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("archivo.txt");
        file(&f, 1);
        let r = delete_contents(&f, None);
        assert_eq!((r.deleted, r.skipped.len()), (0, 1));
        assert_eq!(r.skipped[0].reason, SkipReason::Other);
        assert!(f.exists());
    }

    #[test]
    fn permite_usuario() {
        for p in [
            env("SystemRoot").join(r"Temp\x"),
            env("USERPROFILE").join("proyecto"),
            env("LOCALAPPDATA").join("npm-cache"),
            env("TEMP").join("x"),
        ] {
            assert!(!is_blocked(&p), "{p:?}");
        }
    }

    #[test]
    fn contenido_permitido() {
        for p in [
            PathBuf::from(r"C:\"),
            env("USERPROFILE"),
            env("LOCALAPPDATA"),
            env("APPDATA"),
            env("ProgramFiles"),
            env("SystemRoot"),
        ] {
            assert!(!contents_allowed(&p), "{p:?}");
        }
        for p in [env("SystemRoot").join("Temp"), env("TEMP"), env("LOCALAPPDATA").join("npm-cache")] {
            assert!(contents_allowed(&p), "{p:?}");
        }
    }

    #[test]
    fn contenido_rechaza_bloqueada() {
        assert_one_blocked(&delete_contents(&env("ProgramData").join("limpyrid-test-inexistente"), None));
    }

    #[test]
    fn bloqueo_normaliza() {
        let sys = env("SystemRoot").to_string_lossy().into_owned();
        let user = env("USERPROFILE").to_string_lossy().into_owned();
        for p in [
            format!(r"\\?\{sys}\System32"),
            format!("{}/system32/", sys.replace('\\', "/")),
            sys.to_uppercase(),
            format!(r"{sys}\Temp"),
            format!(r"{user}\proyecto\.."),
            format!(r"{user}\proyecto\..\AppData"),
            format!(r"{user}\AppData\Local\"),
            format!(r"\\?\{}", user.to_lowercase()),
            r"C:\Users\..".to_string(),
            r"C:/".to_string(),
        ] {
            assert!(is_blocked(Path::new(&p)), "{p}");
        }
        assert!(!is_blocked(Path::new(&format!(r"\\?\{sys}\TEMP\x"))));
    }

    #[test]
    fn borra_contenido() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&d.join("a.bin"), 1000);
        file(&d.join("sub/b.bin"), 500);
        let r = delete_contents(&d, None);
        assert_eq!((r.freed, r.deleted), (1500, 2));
        assert!(r.skipped.is_empty());
        assert!(d.exists() && is_empty_dir(&d));
    }

    #[test]
    fn excluye_nombres() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&d.join("claude/x.bin"), 10);
        file(&d.join("otro.bin"), 10);
        let r = delete_contents_except(&d, None, &["CLAUDE"]);
        assert!(!d.join("otro.bin").exists());
        assert!(d.join("claude/x.bin").exists());
        assert_eq!((r.deleted, r.skipped.len()), (1, 0));
    }

    #[test]
    fn respeta_24h() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&d.join("viejo.bin"), 10);
        file(&d.join("nuevo.bin"), 10);
        let h48 = SystemTime::now() - Duration::from_secs(48 * 3600);
        File::options().write(true).open(d.join("viejo.bin")).unwrap().set_modified(h48).unwrap();
        let r = delete_contents(&d, Some(Duration::from_secs(24 * 3600)));
        assert!(!d.join("viejo.bin").exists());
        assert!(d.join("nuevo.bin").exists());
        assert_eq!((r.deleted, r.skipped.len()), (1, 1));
        assert_eq!(r.skipped[0].reason, SkipReason::TooRecent);
        assert!(r.skipped[0].path.ends_with("nuevo.bin") && !r.skipped[0].path.starts_with(r"\\?\"));
    }

    // FILE_FLAG_BACKUP_SEMANTICS permite abrir carpetas para fijar su mtime.
    fn set_mtime(p: &Path, t: SystemTime) {
        File::options().write(true).custom_flags(0x0200_0000).open(p).unwrap().set_modified(t).unwrap();
    }

    #[test]
    fn edad_mira_dentro() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&d.join("viva/a/b/nuevo.bin"), 10);
        file(&d.join("muerta/a/b/viejo.bin"), 10);
        let h48 = SystemTime::now() - Duration::from_secs(48 * 3600);
        // De abajo arriba: crear un hijo cambia el mtime de su carpeta.
        for p in ["muerta/a/b/viejo.bin", "muerta/a/b", "muerta/a", "muerta", "viva/a/b", "viva/a", "viva"] {
            set_mtime(&d.join(p), h48);
        }
        let r = delete_contents(&d, Some(Duration::from_secs(24 * 3600)));
        assert!(d.join("viva/a/b/nuevo.bin").exists(), "una carpeta vieja con un archivo reciente dentro no se borra");
        assert!(!d.join("muerta").exists());
        assert_eq!(r.deleted, 1);
        assert_eq!(r.skipped.len(), 1);
        assert!(r.skipped[0].path.ends_with("viva") && r.skipped[0].reason == SkipReason::TooRecent);
    }

    #[test]
    fn salta_en_uso() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&d.join("libre.bin"), 10);
        let lock = d.join("uso.bin");
        let _open = File::options().write(true).create_new(true).share_mode(0).open(&lock).unwrap();
        let r = delete_contents(&d, None);
        assert!(lock.exists() && !d.join("libre.bin").exists());
        assert_eq!((r.deleted, r.skipped.len()), (1, 1));
        assert_eq!(r.skipped[0].reason, SkipReason::InUse);
    }

    #[test]
    fn solo_lectura() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        let f = d.join("ro.bin");
        file(&f, 10);
        let mut perm = fs::metadata(&f).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&f, perm).unwrap();
        let r = delete_contents(&d, None);
        assert!(!f.exists());
        assert_eq!((r.deleted, r.skipped.len()), (1, 0));
    }

    #[test]
    fn no_atraviesa_junction() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        file(&t.path().join("fuera/keep.bin"), 10);
        fs::create_dir_all(&d).unwrap();
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(d.join("link"))
            .arg(t.path().join("fuera"))
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink fallo");
        let r = delete_contents(&d, None);
        assert!(!d.join("link").exists());
        assert!(t.path().join("fuera/keep.bin").exists());
        assert_eq!((r.deleted, r.freed), (0, 0));
    }

    fn junction(link: &Path, target: &Path) {
        let out = Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().unwrap();
        assert!(out.status.success(), "mklink fallo");
    }

    #[test]
    fn contenido_rechaza_carpeta_junction() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("real/keep.bin"), 10);
        junction(&t.path().join("link"), &t.path().join("real"));
        assert_one_blocked(&delete_contents(&t.path().join("link"), None));
        assert!(t.path().join("real/keep.bin").exists());
    }

    #[test]
    fn delete_dirs_desengancha_junction() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("fuera/keep.bin"), 10);
        let nm = t.path().join(r"p\node_modules");
        file(&nm.join("a.bin"), 5);
        junction(&nm.join("link"), &t.path().join("fuera"));
        let r = delete_dirs(std::slice::from_ref(&nm));
        assert!(!nm.exists());
        assert!(t.path().join("fuera/keep.bin").exists());
        assert_eq!((r.deleted, r.freed), (1, 5));
    }

    #[test]
    fn trash_no_toca_destino_de_junction() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join("fuera/keep.bin"), 10);
        let d = t.path().join(r"p\d");
        file(&d.join("a.bin"), 5);
        junction(&d.join("link"), &t.path().join("fuera"));
        let r = trash(std::slice::from_ref(&d));
        assert!(!d.exists());
        assert!(t.path().join("fuera/keep.bin").exists());
        assert_eq!((r.deleted, r.freed), (1, 5));
    }

    #[test]
    fn motivos_de_papelera() {
        let os = |code: u32| trash::Error::Os { code: code as i32, description: String::new() };
        assert_eq!(trash_reason(&os(0x8027_0037)), SkipReason::TooLarge); // COPYENGINE_E_RECYCLE_SIZE_TOO_BIG
        assert_eq!(trash_reason(&os(0x8027_0027)), SkipReason::InUse); // COPYENGINE_E_SHARING_VIOLATION_SRC
        assert_eq!(trash_reason(&os(0x8007_0020)), SkipReason::InUse); // ERROR_SHARING_VIOLATION
        assert_eq!(trash_reason(&os(0x8027_0021)), SkipReason::Denied); // COPYENGINE_E_ACCESS_DENIED_SRC
        assert_eq!(trash_reason(&os(0x8007_0005)), SkipReason::Denied); // E_ACCESSDENIED
        assert_eq!(trash_reason(&os(0x8027_0036)), SkipReason::Other); // RECYCLE_FORCE_NUKE: papelera no disponible
        assert_eq!(trash_reason(&trash::Error::Unknown { description: String::new() }), SkipReason::Other);
    }

    #[test]
    fn bloquea_unc() {
        for p in [r"\\localhost\C$\Users\x", r"\\?\UNC\localhost\C$\x", "//localhost/C$/x", r"\\.\C:\x", r"\\?\\\?\C:\x"] {
            assert!(is_blocked(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn ruta_larga() {
        let t = tempfile::tempdir().unwrap();
        let mut p = t.path().to_path_buf();
        while p.as_os_str().len() < 300 {
            p.push("d".repeat(50));
        }
        let f = verbatim(&p).join("f.bin");
        fs::create_dir_all(verbatim(&p)).unwrap();
        fs::write(&f, b"x").unwrap();
        let r = delete_contents(t.path(), None);
        assert_eq!((r.deleted, r.freed), (1, 1));
        assert!(!f.exists());
    }

    #[test]
    fn inexistente() {
        let t = tempfile::tempdir().unwrap();
        let no = t.path().join("no_existe");
        for r in [delete_dirs(std::slice::from_ref(&no)), delete_contents(&no, None), trash(&[no])] {
            assert_eq!((r.freed, r.deleted, r.skipped.len()), (0, 0, 0));
        }
    }

    // Solo rutas bloqueadas que no existen: si el bloqueo fallara, no se tocaria nada real.
    #[test]
    fn rechaza_bloqueada() {
        let r = delete_dirs(&[env("SystemRoot").join("limpyrid-test-inexistente")]);
        assert_one_blocked(&r);
        let r = trash(&[env("ProgramData").join("limpyrid-test-inexistente")]);
        assert_one_blocked(&r);
    }

    #[test]
    fn delete_dirs_completa() {
        let t = tempfile::tempdir().unwrap();
        let nm = t.path().join("p/node_modules");
        file(&nm.join("a/x.js"), 100);
        file(&nm.join("y.js"), 50);
        let r = delete_dirs(&[nm.clone()]);
        assert!(!nm.exists() && t.path().join("p").exists());
        assert_eq!((r.deleted, r.freed), (2, 150));
    }

    #[test]
    fn avisa_progreso() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        for (n, b) in [("a.bin", 100), ("b.bin", 200), ("c.bin", 300)] {
            file(&d.join(n), b);
        }
        let seen = std::sync::Mutex::new(vec![]);
        let r = delete_contents_except_with(&d, None, &[], &|f, n| seen.lock().unwrap().push((f, n)));
        let seen = seen.into_inner().unwrap();
        assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 <= w[1].1), "nunca baja: {seen:?}");
        assert_eq!(seen.last(), Some(&(600, 3)));
        assert_eq!((r.freed, r.deleted), (600, 3));
    }

    // Arbol con un archivo viejo, uno reciente y una carpeta vieja que contiene una junction hacia `fuera`.
    fn arbol(root: &Path) -> PathBuf {
        let d = root.join("d");
        file(&root.join("fuera/keep.bin"), 10);
        file(&d.join("viejo.bin"), 100);
        file(&d.join("nuevo.bin"), 200);
        file(&d.join("sub/x.bin"), 300);
        junction(&d.join(r"sub\link"), &root.join("fuera")); // mklink no acepta "/"
        let h48 = SystemTime::now() - Duration::from_secs(48 * 3600);
        for p in ["viejo.bin", "sub/x.bin", "sub"] {
            set_mtime(&d.join(p), h48);
        }
        d
    }

    // Informe completo con las rutas omitidas relativas a la raiz de su arbol.
    fn resumen(r: &Report, root: &Path) -> (u64, u64, Vec<(PathBuf, String)>) {
        let rel = |p: &str| Path::new(p).strip_prefix(root).unwrap().to_path_buf();
        (r.freed, r.deleted, r.skipped.iter().map(|k| (rel(&k.path), format!("{:?}", k.reason))).collect())
    }

    #[test]
    fn avisador_no_cambia_resultado() {
        use std::sync::atomic::{AtomicU64, Ordering};
        for age in [None, Some(Duration::from_secs(24 * 3600))] {
            let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
            let (da, db) = (arbol(a.path()), arbol(b.path()));
            let calls = AtomicU64::new(0);
            let sin = delete_contents_except(&da, age, &[]);
            let con = delete_contents_except_with(&db, age, &[], &|_, _| {
                calls.fetch_add(1, Ordering::Relaxed);
            });
            let (rs, rc) = (resumen(&sin, a.path()), resumen(&con, b.path()));
            assert_eq!(rs, rc, "{age:?}");
            // Con antiguedad se omiten el archivo reciente y la carpeta con la junction (recien creada).
            assert_eq!(rc.2.len(), if age.is_some() { 2 } else { 0 }, "{rc:?}");
            assert_eq!(calls.load(Ordering::Relaxed), rc.1);
            assert!(a.path().join("fuera/keep.bin").exists() && b.path().join("fuera/keep.bin").exists());
        }
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (da, db) = (arbol(a.path()), arbol(b.path()));
        let sin = delete_dirs(std::slice::from_ref(&da));
        let con = delete_dirs_with(std::slice::from_ref(&db), &|_, _| {});
        assert_eq!(resumen(&sin, a.path()), resumen(&con, b.path()));
        assert_eq!((con.freed, con.deleted), (600, 3));
        assert!(!db.exists() && b.path().join("fuera/keep.bin").exists());
    }

    #[test]
    fn trash_envia_a_papelera() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("p/borrar.bin");
        file(&f, 123);
        let r = trash(std::slice::from_ref(&f));
        assert!(!f.exists());
        assert_eq!((r.deleted, r.freed, r.skipped.len()), (1, 123, 0));
    }
}
