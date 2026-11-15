// Restos del actualizador de Tauri en la carpeta temporal y deteccion de la version portable.
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

const REPARSE_POINT: u32 = 0x400;
const DIRECTORY: u32 = 0x10;

// Lo que sigue a `<app>-`, en minusculas, si empieza por un digito: asi `Limpyrid-Pro-1.0` no pasa por Limpyrid.
fn after_app(name: &str, app: &str) -> Option<String> {
    let rest = name.to_ascii_lowercase().strip_prefix(&format!("{}-", app.to_ascii_lowercase()))?.to_string();
    rest.starts_with(|c: char| c.is_ascii_digit()).then_some(rest)
}

// `<app>-<version>-updater-<sufijo>`, version y sufijo no vacios.
fn leftover_name(name: &str, app: &str) -> bool {
    after_app(name, app).is_some_and(|rest| rest.split_once("-updater-").is_some_and(|(_, suffix)| !suffix.is_empty()))
}

// `<app>-<version>-installer.exe|msi`.
fn installer_name(name: &str, app: &str) -> bool {
    after_app(name, app).is_some_and(|rest| rest.ends_with("-installer.exe") || rest.ends_with("-installer.msi"))
}

// Archivos de la carpeta con su tamano, o None si dentro hay algo que no sea un instalador normal o no se puede leer.
fn installers(dir: &Path, app: &str) -> Option<Vec<(PathBuf, u64)>> {
    let mut out = vec![];
    for entry in fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        let meta = fs::symlink_metadata(&path).ok()?;
        let name_ok = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| installer_name(n, app));
        if !name_ok || meta.file_attributes() & (DIRECTORY | REPARSE_POINT) != 0 {
            return None;
        }
        out.push((path, meta.len()));
    }
    Some(out)
}

// Borra las carpetas que deja el actualizador en `base` (reglas en el spec, "Limpieza de restos"), sin borrado
// recursivo. Devuelve (carpetas borradas, bytes liberados); lo que falla se deja para el siguiente arranque.
// Release compila con panic = "abort": un panico aqui cierra la app aunque corra en otro hilo.
pub fn clean_leftovers(base: &Path, app: &str) -> (usize, u64) {
    let mut done = (0, 0);
    if app.trim().is_empty() {
        return done;
    }
    let Ok(entries) = fs::read_dir(base) else { return done };
    for entry in entries.flatten() {
        let dir = entry.path();
        let name_ok = entry.file_name().to_str().is_some_and(|n| leftover_name(n, app));
        let real_dir = fs::symlink_metadata(&dir).is_ok_and(|m| m.file_attributes() & (DIRECTORY | REPARSE_POINT) == DIRECTORY);
        if !name_ok || !real_dir {
            continue;
        }
        let Some(files) = installers(&dir, app) else { continue };
        let bytes: u64 = files.iter().filter(|(f, _)| fs::remove_file(f).is_ok()).map(|(_, len)| len).sum();
        if fs::remove_dir(&dir).is_ok() {
            done = (done.0 + 1, done.1 + bytes);
        }
    }
    done
}

// La version portable lleva este marcador junto al ejecutable.
pub fn is_portable(exe_dir: &Path) -> bool {
    exe_dir.join("limpyrid.portable").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    const DIR: &str = "Limpyrid-0.2.0-updater-abc123";
    const EXE: &str = "Limpyrid-0.2.0-installer.exe";

    fn file(p: &Path, len: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![0u8; len]).unwrap();
    }

    // Mismo ayudante que las pruebas de clean.rs.
    fn junction(link: &Path, target: &Path) {
        let out = Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().unwrap();
        assert!(out.status.success(), "mklink fallo");
    }

    #[test]
    fn borra_resto_valido() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join(DIR).join(EXE), 1000);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (1, 1000));
        assert!(!t.path().join(DIR).exists());
    }

    #[test]
    fn mayusculas_distintas() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("limpyrid-0.2.0-UPDATER-x");
        file(&d.join("LIMPYRID-0.2.0-installer.EXE"), 300);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (1, 300));
        assert!(!d.exists());
    }

    #[test]
    fn msi_tambien() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join(DIR).join("Limpyrid-0.2.0-installer.msi"), 50);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (1, 50));
        assert!(!t.path().join(DIR).exists());
    }

    #[test]
    fn archivo_ajeno_deja_todo() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join(DIR);
        file(&d.join(EXE), 1000);
        file(&d.join("notas.txt"), 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(d.join(EXE).is_file() && d.join("notas.txt").is_file());
    }

    #[test]
    fn subcarpeta_deja_todo() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join(DIR);
        file(&d.join(EXE), 1000);
        fs::create_dir(d.join("sub")).unwrap();
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(d.join(EXE).is_file() && d.join("sub").is_dir());
    }

    #[test]
    fn otra_app_no_se_toca() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("Otra-1.0-updater-x").join("Otra-1.0-installer.exe");
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn nombres_parecidos_no_valen() {
        let t = tempfile::tempdir().unwrap();
        let dirs = ["Limpyrid-updater", "Limpyrid--updater-x", "Limpyrid-1-updater-", "LimpyridX-1-updater-y"];
        for d in dirs {
            file(&t.path().join(d).join(EXE), 10);
        }
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(dirs.iter().all(|d| t.path().join(d).join(EXE).is_file()));
    }

    #[test]
    fn archivo_con_nombre_de_carpeta() {
        let t = tempfile::tempdir().unwrap();
        file(&t.path().join(DIR), 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(t.path().join(DIR).is_file());
    }

    #[test]
    fn no_baja_niveles() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("x").join(DIR).join(EXE);
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn enlace_no_se_sigue() {
        let (t, u) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let destino = u.path().join("destino");
        file(&destino.join(EXE), 10);
        let link = t.path().join("Limpyrid-0.2.0-updater-j");
        junction(&link, &destino);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(fs::symlink_metadata(&link).is_ok(), "el enlace sigue");
        assert!(destino.join(EXE).is_file(), "el destino intacto");
    }

    #[test]
    fn carpeta_vacia_valida() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join(DIR)).unwrap();
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (1, 0));
        assert!(!t.path().join(DIR).exists());
    }

    #[test]
    fn base_inexistente() {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().join("no-existe");
        assert_eq!(clean_leftovers(&base, "Limpyrid"), (0, 0));
        assert!(!base.exists());
    }

    #[test]
    fn app_vacio_no_borra() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("-1-updater-x").join("-1-installer.exe");
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), ""), (0, 0));
        assert_eq!(clean_leftovers(t.path(), "  "), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn otro_producto_con_prefijo() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("Limpyrid-Pro-1.0-updater-x").join("Limpyrid-Pro-1.0-installer.exe");
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn instalador_de_otra_app_dentro() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join(DIR).join("Otra-1.0-installer.exe");
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn instalador_sin_version() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join(DIR).join("Limpyrid-installer.exe");
        file(&f, 10);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(f.is_file());
    }

    #[test]
    fn enlace_dentro_deja_todo() {
        let (t, u) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let destino = u.path().join("destino");
        file(&destino.join("dato.txt"), 10);
        let d = t.path().join(DIR);
        fs::create_dir(&d).unwrap();
        junction(&d.join(EXE), &destino);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (0, 0));
        assert!(fs::symlink_metadata(d.join(EXE)).is_ok(), "la junction sigue");
        assert!(destino.join("dato.txt").is_file(), "el destino intacto");
    }

    #[test]
    fn sufijo_con_updater() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("Limpyrid-1.0.0-updater-x-updater-y");
        file(&d.join("Limpyrid-1.0.0-installer.exe"), 70);
        assert_eq!(clean_leftovers(t.path(), "Limpyrid"), (1, 70));
        assert!(!d.exists());
    }

    #[test]
    fn portable_por_marcador() {
        let t = tempfile::tempdir().unwrap();
        assert!(!is_portable(t.path()));
        file(&t.path().join("limpyrid.portable"), 0);
        assert!(is_portable(t.path()));
        assert!(t.path().join("limpyrid.portable").is_file());
    }
}
