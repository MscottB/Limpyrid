use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Docker,
    Podman,
}
#[derive(Serialize)]
pub struct UsageRow {
    pub kind: String,
    pub total: String,
    pub reclaimable: String,
}
#[derive(Serialize)]
pub struct DiskFile {
    pub path: String,
    pub size: u64,
}
#[derive(Serialize)]
pub struct Usage {
    pub installed: bool,
    pub running: bool,
    pub rows: Vec<UsageRow>,
    pub disks: Vec<DiskFile>,
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl Engine {
    fn exe(self) -> &'static str {
        match self {
            Engine::Docker => "docker",
            Engine::Podman => "podman",
        }
    }
}

pub fn parse_df(engine: Engine, stdout: &str) -> Vec<UsageRow> {
    match engine {
        Engine::Docker => stdout
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| {
                let f = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
                Some(UsageRow {
                    kind: f("Type")?,
                    total: f("Size")?,
                    reclaimable: f("Reclaimable")?,
                })
            })
            .collect(),
        // shortcut: formato de podman sin verificar, completar con una instalación real
        Engine::Podman => vec![],
    }
}

// Nunca incluye --volumes.
pub fn prune_args(all_images: bool) -> Vec<&'static str> {
    let mut a = vec!["system", "prune", "-f"];
    if all_images {
        a.push("-a");
    }
    a
}

fn command(exe: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(exe);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

// Ok = stdout. Err: engine_missing (no está en PATH) o engine_stopped (el comando falla).
fn run(engine: Engine, args: &[&str]) -> Result<String, &'static str> {
    match command(engine.exe()).args(args).output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err("engine_missing"),
        Err(_) => Err("command_failed"),
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(_) => Err("engine_stopped"),
    }
}

fn vhdx_in(dir: &Path, depth: u8, out: &mut Vec<DiskFile>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = e.metadata() else { continue };
        if m.is_dir() && depth > 0 {
            vhdx_in(&p, depth - 1, out);
        } else if m.is_file()
            && p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("vhdx"))
        {
            out.push(DiskFile {
                path: p.to_string_lossy().into_owned(),
                size: m.len(),
            });
        }
    }
}

// Carpetas del motor tomadas del catálogo (ids "docker" / "podman").
fn engine_dirs(engine: Engine) -> Vec<PathBuf> {
    crate::catalog::CATALOG
        .iter()
        .filter(|c| c.id == engine.exe())
        .flat_map(|c| c.paths.iter())
        .flat_map(|p| crate::catalog::expand(p))
        .collect()
}

fn disks(engine: Engine) -> Vec<DiskFile> {
    let mut out = vec![];
    for d in engine_dirs(engine) {
        vhdx_in(&d, 4, &mut out);
    }
    out
}

pub fn usage(engine: Engine) -> Usage {
    let disks = disks(engine);
    match run(engine, &["system", "df", "--format", "{{json .}}"]) {
        Ok(out) => Usage {
            installed: true,
            running: true,
            rows: parse_df(engine, &out),
            disks,
        },
        Err("engine_missing") => Usage {
            installed: false,
            running: false,
            rows: vec![],
            disks,
        },
        Err(_) => Usage {
            installed: true,
            running: false,
            rows: vec![],
            disks,
        },
    }
}

// Docker local en Windows = named pipe. Con DOCKER_HOST o un contexto remoto el prune iria a otro equipo
// mientras la tarjeta muestra el disco local: solo se limpia si ambos apuntan a npipe://.
fn local_engine(docker_host: Option<&str>, context_host: &str) -> bool {
    docker_host.is_none_or(|h| h.trim().is_empty() || h.trim().starts_with("npipe://")) && context_host.trim().starts_with("npipe://")
}

// Prune fallido: si el motor responde, el fallo es del comando, no un motor detenido.
fn prune_failure(reachable: bool) -> &'static str {
    if reachable { "command_failed" } else { "engine_stopped" }
}

// diskpart lee el guion con la pagina de codigos OEM: una ruta no ASCII se leeria mal con el motor ya detenido.
fn ascii_only(paths: &[&Path]) -> bool {
    paths.iter().all(|p| p.to_string_lossy().is_ascii())
}

// El motor puede tardar en soltar el disco tras apagarse WSL: se reintenta antes de rendirse.
fn wait_free(path: &Path, tries: u32, pause: std::time::Duration) -> bool {
    (0..tries).any(|i| {
        if i > 0 {
            std::thread::sleep(pause);
        }
        is_free(path)
    })
}

pub fn prune(engine: Engine, all_images: bool) -> Result<String, &'static str> {
    match engine {
        Engine::Docker => {
            let host = std::env::var("DOCKER_HOST").ok();
            let ctx = run(engine, &["context", "inspect", "--format", "{{.Endpoints.docker.Host}}"])?;
            if !local_engine(host.as_deref(), &ctx) {
                return Err("engine_remote");
            }
        }
        // shortcut: Podman no esta instalado y no se verifico su forma de apuntar a un motor remoto
        // (CONTAINER_HOST, `podman system connection`); comprobarlo con una instalacion real.
        Engine::Podman => {}
    }
    run(engine, &prune_args(all_images)).map_err(|e| match e {
        "engine_stopped" => prune_failure(run(engine, &["version"]).is_ok()),
        other => other,
    })
}

const UAC_CANCELLED: i32 = 1223; // ERROR_CANCELLED; lo devuelve el PowerShell externo si Start-Process lanza
const FILE_SHARE_READ: u32 = 1;

fn largest(v: Vec<DiskFile>) -> Option<DiskFile> {
    v.into_iter().max_by_key(|d| d.size)
}

// Script A de diskpart (sin `noerr`: si attach o compact fallan, diskpart se detiene y sale con código != 0).
fn diskpart_script(vhdx: &str) -> String {
    format!("select vdisk file=\"{vhdx}\"\r\nattach vdisk readonly\r\ncompact vdisk\r\nexit\r\n")
}

// Script B: siempre se ejecuta después de A; `noerr` porque puede no haber nada que desconectar.
fn diskpart_detach_script(vhdx: &str) -> String {
    format!("select vdisk file=\"{vhdx}\"\r\ndetach vdisk noerr\r\nexit\r\n")
}

// Sin otro proceso con el archivo abierto (apertura exclusiva).
fn is_free(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .is_ok()
}

fn elevated_result(code: Option<i32>) -> Result<(), &'static str> {
    match code {
        Some(0) => Ok(()),
        Some(UAC_CANCELLED) => Err("uac_denied"),
        _ => Err("command_failed"),
    }
}

// Archivo nuevo y único (create_new): se escribe, se cierra y se reabre SOLO lectura con FILE_SHARE_READ; se
// relee y compara con lo escrito. Mientras el `File` siga vivo nadie puede reescribirlo ni borrarlo, y cualquier
// lector con share de lectura (diskpart elevado) puede abrirlo.
fn locked_script(dir: &Path, text: &str, tag: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    use std::io::{Read, Write};
    use std::os::windows::fs::OpenOptionsExt;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let p = dir.join(format!(
        "limpyrid-diskpart-{}-{nanos}-{tag}.txt",
        std::process::id()
    ));
    let mut w = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)?;
    w.write_all(text.as_bytes())?;
    drop(w);
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&p)?;
    let mut back = String::new();
    f.read_to_string(&mut back)?;
    if back != text {
        drop(f);
        let _ = std::fs::remove_file(&p);
        return Err(std::io::Error::other("script modificado"));
    }
    Ok((p, f))
}

// Comando que corre elevado: script A, guarda su código (1 si diskpart ni arrancó), SIEMPRE script B, sale con el de A.
fn elevated_inner(sys32: &str, a: &Path, b: &Path) -> String {
    let q = |p: &Path| p.to_string_lossy().replace('\'', "''");
    format!(
        "$d='{sys32}\\diskpart.exe'; & $d /s '{}'; $c=$LASTEXITCODE; & $d /s '{}' | Out-Null; if ($null -eq $c) {{ $c = 1 }}; exit $c",
        q(a),
        q(b)
    )
}

// PowerShell no elevado: lanza el elevado (cuerpo en $env:LIMPYRID_ELEVATED). Nunca sale 0 sin código real.
fn elevated_outer(ps_exe: &str) -> String {
    format!(
        "try {{ $p = Start-Process -FilePath '{ps_exe}' -ArgumentList ('-NoProfile -Command \"' + $env:LIMPYRID_ELEVATED + '\"')          -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ErrorAction Stop; if ($null -eq $p -or $null -eq $p.ExitCode) {{ exit 1 }}; exit $p.ExitCode }}          catch {{ exit {UAC_CANCELLED} }}"
    )
}

// Método: parar el motor + `wsl --shutdown` + comprobar que el .vhdx está libre + UN solo PowerShell elevado que
// corre diskpart con el script A (select, attach readonly, compact), guarda su código, corre SIEMPRE el script B
// (select, detach noerr) y sale con el código de A. Así un compact fallido no deja el vdisk adjunto y el fallo
// llega como código != 0 (con `noerr` en compact el fallo quedaría oculto: por eso no se usa ahí).
// Fuentes: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/compact-vdisk
// (solo VHD dinámico, desconectado o adjunto de solo lectura) y .../diskpart-scripts-and-examples (por defecto el
// script se detiene en el primer error salvo `noerr`). SIN CONFIRMAR en documentación oficial: que diskpart
// salga con código != 0 ante un compact fallido (solo una guía de terceros lista 0 = sin error, 5 = sintaxis).
// Ni docs.docker.com/desktop/features/wsl/ ni learn.microsoft.com/windows/wsl/disk-space documentan compactar
// el .vhdx; la secuencia completa NO está verificada contra la documentación de Docker 29.9.3.
pub fn compact(engine: Engine) -> Result<(u64, u64), &'static str> {
    let big = largest(disks(engine)).ok_or("engine_missing")?;
    let vhdx = PathBuf::from(&big.path);
    let under_catalog = engine_dirs(engine).iter().any(|d| vhdx.starts_with(d));
    if !under_catalog
        || !vhdx.is_file()
        || !vhdx
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("vhdx"))
    {
        return Err("command_failed");
    }
    let before = big.size;
    let tmp = std::env::temp_dir();
    // Se comprueba antes de detener nada.
    if !ascii_only(&[&vhdx, &tmp]) {
        return Err("command_failed");
    }

    // Solo la parada propia del motor: si falla no se mata nada ni se apaga WSL (eso cortaria contenedores en marcha).
    let stop: &[&str] = match engine {
        Engine::Docker => &["desktop", "stop"],
        Engine::Podman => &["machine", "stop"],
    };
    if !command(engine.exe()).args(stop).output().is_ok_and(|o| o.status.success()) {
        return Err("command_failed");
    }
    if !command("wsl")
        .arg("--shutdown")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Err("command_failed");
    }

    // El motor (o su backend) puede seguir con el archivo abierto: no se eleva si no está libre.
    if !wait_free(&vhdx, 10, std::time::Duration::from_secs(1)) {
        return Err("command_failed");
    }

    let v = vhdx.to_string_lossy();
    let Ok((path_a, hold_a)) = locked_script(&tmp, &diskpart_script(&v), "a") else {
        return Err("command_failed");
    };
    let Ok((path_b, hold_b)) = locked_script(&tmp, &diskpart_detach_script(&v), "b") else {
        drop(hold_a);
        let _ = std::fs::remove_file(&path_a);
        return Err("command_failed");
    };
    // Rutas absolutas bajo %SystemRoot%: un PATH editable por el usuario no puede poner el binario que se eleva.
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let sys32 = format!(r"{root}\System32");
    let ps_exe = format!(r"{sys32}\WindowsPowerShell\v1.0\powershell.exe");
    // El cuerpo elevado viaja por entorno para no anidar comillas.
    let res = command(&ps_exe)
        .env(
            "LIMPYRID_ELEVATED",
            elevated_inner(&sys32, &path_a, &path_b),
        )
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &elevated_outer(&ps_exe),
        ])
        .output();
    // Los File siguen abiertos hasta que Start-Process -Wait volvió; recién ahora se sueltan y se borran.
    drop((hold_a, hold_b));
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
    elevated_result(res.map_err(|_| "command_failed")?.status.code())?;
    // El código de salida del script B se descarta: si el detach falló, el vdisk sigue adjunto y el archivo ocupado.
    if !wait_free(&vhdx, 10, std::time::Duration::from_secs(1)) {
        return Err("command_failed");
    }

    let after = std::fs::metadata(&vhdx)
        .map_err(|_| "command_failed")?
        .len();
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Forma real de `docker system df --format "{{json .}}"` (Docker 29.9.3), tamanos sinteticos.
    const DOCKER_DF: &str = r#"{"Active":"1","Reclaimable":"1GB (50%)","Size":"2GB","TotalCount":"3","Type":"Images"}
{"Active":"0","Reclaimable":"0B (0%)","Size":"10MB","TotalCount":"1","Type":"Containers"}
{"Active":"2","Reclaimable":"5GB (10%)","Size":"50GB","TotalCount":"4","Type":"Local Volumes"}
{"Active":"0","Reclaimable":"1.5GB","Size":"2.5GB","TotalCount":"9","Type":"Build Cache"}
"#;

    #[test]
    fn parse_docker() {
        let rows = parse_df(Engine::Docker, DOCKER_DF);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].kind, "Images");
        assert_eq!(rows[0].total, "2GB");
        assert_eq!(rows[0].reclaimable, "1GB (50%)");
        assert_eq!(rows[2].kind, "Local Volumes");
        assert_eq!(rows[3].reclaimable, "1.5GB");
    }

    #[test]
    fn parse_basura() {
        assert!(parse_df(Engine::Docker, "").is_empty());
        assert!(parse_df(Engine::Docker, "no es json\n{roto").is_empty());
    }

    #[test]
    fn prune_sin_volumenes() {
        assert_eq!(prune_args(false), ["system", "prune", "-f"]);
        assert_eq!(prune_args(true), ["system", "prune", "-f", "-a"]);
        assert!(
            !prune_args(true).contains(&"--volumes") && !prune_args(false).contains(&"--volumes")
        );
    }

    #[test]
    fn script_con_comillas() {
        let s = diskpart_script(r"C:\a b\x.vhdx");
        assert!(s.starts_with(
            "select vdisk file=\"C:\\a b\\x.vhdx\"\r\nattach vdisk readonly\r\ncompact vdisk\r\n"
        ));
        assert!(!s.contains("detach"), "el detach va en su propio script");
        let d = diskpart_detach_script(r"C:\a b\x.vhdx");
        assert!(d.starts_with("select vdisk file=\"C:\\a b\\x.vhdx\"\r\n"));
        assert!(d.contains("detach vdisk noerr"));
    }

    #[test]
    fn codigos_elevados() {
        assert_eq!(elevated_result(Some(0)), Ok(()));
        assert_eq!(elevated_result(Some(UAC_CANCELLED)), Err("uac_denied"));
        assert_eq!(elevated_result(Some(1)), Err("command_failed"));
        assert_eq!(elevated_result(Some(5)), Err("command_failed"));
        assert_eq!(elevated_result(None), Err("command_failed"));
    }

    #[test]
    fn archivo_libre() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.vhdx");
        std::fs::write(&p, b"x").unwrap();
        assert!(is_free(&p));
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&p)
            .unwrap();
        assert!(!is_free(&p));
        drop(held);
        assert!(is_free(&p));
        assert!(!is_free(&dir.path().join("no-existe.vhdx")));
    }

    #[test]
    fn descubre_vhdx() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("disk").join("deep");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.path().join("small.vhdx"), vec![0u8; 10]).unwrap();
        std::fs::write(sub.join("BIG.VHDX"), vec![0u8; 300]).unwrap();
        std::fs::write(dir.path().join("mid.vhdx"), vec![0u8; 50]).unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![0u8; 9999]).unwrap();
        let mut found = vec![];
        vhdx_in(dir.path(), 4, &mut found);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|d| !d.path.ends_with("big.txt")));
        let top = largest(found).unwrap();
        assert!(top.path.ends_with("BIG.VHDX"));
        assert_eq!(top.size, 300);
        assert!(largest(vec![]).is_none());
    }

    #[test]
    fn guion_bloqueado() {
        use std::io::Read;
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (p, held) = locked_script(dir.path(), "select vdisk file=\"x\"\r\n", "t").unwrap();
        let mut got = String::new();
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&p)
            .unwrap()
            .read_to_string(&mut got)
            .unwrap();
        assert_eq!(got, "select vdisk file=\"x\"\r\n");
        assert!(std::fs::OpenOptions::new().write(true).open(&p).is_err());
        assert!(std::fs::remove_file(&p).is_err());
        drop(held);
        std::fs::remove_file(&p).unwrap();
    }

    #[test]
    fn motor_local() {
        let pipe = "npipe:////./pipe/dockerDesktopLinuxEngine"; // forma real de `docker context inspect`
        assert!(local_engine(None, pipe));
        assert!(local_engine(Some(""), pipe));
        assert!(local_engine(Some("npipe:////./pipe/docker_engine"), pipe));
        assert!(!local_engine(Some("tcp://10.0.0.5:2376"), pipe));
        assert!(!local_engine(Some("ssh://u@host"), pipe));
        assert!(!local_engine(None, "tcp://remote:2376"));
        assert!(!local_engine(None, ""));
        assert!(local_engine(None, "  npipe:////./pipe/x\r\n"));
    }

    #[test]
    fn fallo_de_prune() {
        assert_eq!(prune_failure(true), "command_failed");
        assert_eq!(prune_failure(false), "engine_stopped");
    }

    #[test]
    fn rutas_ascii() {
        assert!(ascii_only(&[Path::new(r"C:\Users\x\AppData\Local\Docker\wsl\disk\docker_data.vhdx"), Path::new(r"C:\T")]));
        assert!(!ascii_only(&[Path::new(r"C:\Users\José\x.vhdx")]));
        assert!(!ascii_only(&[Path::new(r"C:\a.vhdx"), Path::new(r"C:\Temp\ñ")]));
    }

    #[test]
    fn espera_archivo_libre() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.vhdx");
        std::fs::write(&p, b"x").unwrap();
        assert!(wait_free(&p, 3, std::time::Duration::ZERO));
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&p).unwrap();
        assert!(!wait_free(&p, 3, std::time::Duration::ZERO));
        drop(held);
    }

    #[test]
    fn consola_elevada_oculta() {
        assert!(elevated_outer(r"C:\ps.exe").contains("-WindowStyle Hidden"));
    }

    #[test]
    fn comandos_powershell() {
        let a = Path::new(r"C:\Temp x\a'1.txt");
        let b = Path::new(r"C:\Temp x\b.txt");
        let inner = elevated_inner(r"C:\Windows\System32", a, b);
        assert!(inner.contains("if ($null -eq $c) { $c = 1 }"));
        assert!(inner.contains(r"'C:\Temp x\a''1.txt'") && inner.contains(r"'C:\Temp x\b.txt'"));
        assert!(inner.contains(r"'C:\Windows\System32\diskpart.exe'"));
        let (ia, ib, ic, ie) = (
            inner.find("a''1").unwrap(),
            inner.find("b.txt").unwrap(),
            inner.find("$c=$LASTEXITCODE").unwrap(),
            inner.find("exit $c").unwrap(),
        );
        assert!(
            ia < ic && ic < ib && ib < ie,
            "A, luego guardar código, luego B, luego exit con el de A"
        );
        let outer = elevated_outer(r"C:\Windows\System32\WindowsPowerShellv1.0\powershell.exe");
        assert!(outer.contains("if ($null -eq $p -or $null -eq $p.ExitCode) { exit 1 }"));
        assert!(
            outer.contains("-PassThru")
                && outer.contains("-Wait")
                && outer.contains("catch { exit 1223 }")
        );
        assert!(!inner.contains("--volumes") && !outer.contains("--volumes"));
    }
}
