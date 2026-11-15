// Catalogo de cachés conocidas y deteccion de carpetas de proyecto a partir de un escaneo.
use crate::clean::{canon, is_blocked, long_name};
use crate::scan::{Node, ScanResult, plain, verbatim};
use serde::Serialize;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const MIN_AGE_CHOICES_HOURS: [u64; 5] = [24, 72, 168, 336, 720];
pub const DEFAULT_MIN_AGE_HOURS: u64 = 168;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Contents { min_age_hours: u64 },
    Subfolders { min_age_hours: u64 },
    Project,
    Containers,
    InfoOnly,
    Worktrees { min_age_hours: u64 },
}

pub struct Category {
    pub id: &'static str,
    pub ecosystem: &'static str,
    pub paths: &'static [&'static str],
    pub kind: Kind,
    pub exclude: &'static [&'static str], // nombres de primer nivel que no se tocan ni se cuentan
}

const fn cat(id: &'static str, ecosystem: &'static str, paths: &'static [&'static str], kind: Kind) -> Category {
    Category { id, ecosystem, paths, kind, exclude: &[] }
}

const C0: Kind = Kind::Contents { min_age_hours: 0 };
const C24: Kind = Kind::Contents { min_age_hours: 24 };

// Rutas por defecto en Windows. Fuente de cada fila en el comentario; "equipo" = medido o consultado
// con el comando propio de la herramienta el 2026-11-13. Quitadas por no confirmarse: Yarn clasico
// (`Yarn\Cache`), node-gyp (`node-gyp\Cache`) y Ruby (`.gem`, `.bundle\cache`).
pub const CATALOG: &[Category] = &[
    // variable de entorno TEMP del usuario; %TEMP%\claude se lista aparte (claude-sessions)
    Category { id: "temp-user", ecosystem: "system", paths: &[r"%TEMP%"], kind: C24, exclude: &["claude"] },
    cat("temp-windows", "system", &[r"%SystemRoot%\Temp"], C24), // carpeta estandar de Windows, existe en el equipo
    cat("npm", "node", &[r"%LOCALAPPDATA%\npm-cache"], C0), // `npm config get cache`
    // https://pnpm.io/settings/store (storeDir) y https://pnpm.io/settings/other (cacheDir)
    cat("pnpm", "node", &[r"%LOCALAPPDATA%\pnpm\store", r"%LOCALAPPDATA%\pnpm-cache"], C0),
    // getDefaultGlobalFolder + `${globalFolder}/cache`: github.com/yarnpkg/berry (folderUtils.ts, Configuration.ts)
    cat("yarn", "node", &[r"%LOCALAPPDATA%\Yarn\Berry\cache"], C0),
    cat("bun", "node", &[r"%USERPROFILE%\.bun\install\cache"], C0), // `bun pm cache`
    // https://docs.deno.com/runtime/fundamentals/installation/ (DENO_DIR); location_data guarda localStorage y Deno KV
    Category { id: "deno", ecosystem: "node", paths: &[r"%LOCALAPPDATA%\deno"], kind: C0, exclude: &["location_data"] },
    cat("playwright", "node", &[r"%LOCALAPPDATA%\ms-playwright"], C0), // https://playwright.dev/docs/browsers
    cat("node-projects", "node", &[], Kind::Project),
    cat("pip", "python", &[r"%LOCALAPPDATA%\pip\Cache"], C0), // `pip cache dir`
    cat("uv", "python", &[r"%LOCALAPPDATA%\uv\cache"], C0), // `uv cache dir`
    cat("uv-installed", "python", &[r"%APPDATA%\uv\python", r"%APPDATA%\uv\tools"], Kind::InfoOnly), // `uv python dir`, `uv tool dir`
    // https://python-poetry.org/docs/configuration/ (cache-dir); virtualenvs = los entornos que Poetry gestiona
    Category { id: "poetry", ecosystem: "python", paths: &[r"%LOCALAPPDATA%\pypoetry\Cache"], kind: C0, exclude: &["virtualenvs"] },
    cat("python-projects", "python", &[], Kind::Project),
    cat("go-build", "go", &[r"%LOCALAPPDATA%\go-build"], C0), // `go env GOCACHE`
    cat("go-mod", "go", &[r"%USERPROFILE%\go\pkg\mod"], C0), // `go env GOMODCACHE`
    // https://doc.rust-lang.org/cargo/guide/cargo-home.html (registry/ y git/ bajo CARGO_HOME)
    cat("cargo", "rust", &[r"%USERPROFILE%\.cargo\registry", r"%USERPROFILE%\.cargo\git"], C0),
    cat("rust-target", "rust", &[], Kind::Project),
    // https://docs.gradle.org/current/userguide/directory_layout.html (caches/ bajo el Gradle User Home)
    cat("gradle", "java", &[r"%USERPROFILE%\.gradle\caches"], C0),
    cat("maven", "java", &[r"%USERPROFILE%\.m2\repository"], C0), // https://maven.apache.org/settings.html (localRepository)
    // `dotnet nuget locals all --list`: global-packages y http-cache
    cat("nuget", "dotnet", &[r"%USERPROFILE%\.nuget\packages", r"%LOCALAPPDATA%\NuGet\v3-cache"], C0),
    cat("composer", "php", &[r"%LOCALAPPDATA%\Composer"], C0), // https://getcomposer.org/doc/03-cli.md (COMPOSER_CACHE_DIR)
    // https://dart.dev/tools/pub/environment-variables (PUB_CACHE); solo hosted y git: global_packages, bin y
    // credentials.json no se recuperan con pub get
    cat("dart-pub", "dart", &[r"%LOCALAPPDATA%\Pub\Cache\hosted", r"%LOCALAPPDATA%\Pub\Cache\git"], C0),
    // vcpkg: https://learn.microsoft.com/en-us/vcpkg/users/binarycaching; Conan 2: https://docs.conan.io/2/reference/environment.html
    // (.conan2 = CONAN_HOME) y https://docs.conan.io/2/reference/commands/cache.html (carpeta `p`)
    cat("cpp", "cpp", &[r"%LOCALAPPDATA%\vcpkg\archives", r"%USERPROFILE%\.conan2\p"], C0),
    // github.com/ziglang/zig (src/introspect.zig + lib/std/fs/get_app_data_dir.zig: %LOCALAPPDATA%\zig)
    cat("zig", "zig", &[r"%LOCALAPPDATA%\zig"], C0),
    cat("terraform-projects", "terraform", &[], Kind::Project),
    // worktrees de git en proyectos del escaneo (worktrees.rs); sin rutas fijas
    cat("git-worktrees", "git", &[], Kind::Worktrees { min_age_hours: 24 }),
    // https://docs.cypress.io/app/references/advanced-installation; https://pptr.dev/guides/configuration
    cat("web-testing", "web-testing", &[r"%LOCALAPPDATA%\Cypress\Cache", r"%USERPROFILE%\.cache\puppeteer"], C0),
    cat("native-build", "native-build", &[r"%LOCALAPPDATA%\electron\Cache"], C0), // github.com/electron/get
    // Existen en el equipo (VS Code instalado: `code`)
    cat("vscode", "editors", &[r"%APPDATA%\Code\Cache", r"%APPDATA%\Code\CachedData", r"%APPDATA%\Code\CachedExtensionVSIXs"], C0),
    // https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html
    cat("jetbrains", "editors", &[r"%LOCALAPPDATA%\JetBrains\*\caches"], C0),
    // https://huggingface.co/docs/huggingface_hub/guides/manage-cache (~/.cache/huggingface/hub); el token vive un nivel arriba
    cat("huggingface", "local-ai", &[r"%USERPROFILE%\.cache\huggingface\hub"], C0),
    cat("ollama", "local-ai", &[r"%USERPROFILE%\.ollama\models"], Kind::InfoOnly),
    cat("android", "android", &[r"%USERPROFILE%\.android\avd", r"%LOCALAPPDATA%\Android\Sdk"], Kind::InfoOnly),
    // github.com/kubernetes/minikube (pkg/minikube/localpath: ~/.minikube, cache)
    cat("minikube", "kubernetes", &[r"%USERPROFILE%\.minikube\cache"], C0),
    cat("docker", "containers", &[r"%LOCALAPPDATA%\Docker\wsl"], Kind::Containers),
    cat("podman", "containers", &[r"%USERPROFILE%\.local\share\containers\podman\machine"], Kind::Containers),
    cat("wsl", "wsl", &[r"%LOCALAPPDATA%\wsl\*\ext4.vhdx", r"%LOCALAPPDATA%\Packages\*\LocalState\ext4.vhdx"], Kind::InfoOnly),
    // Agentes de IA: rutas medidas en el equipo el 2026-11-13
    // Solo informativa: Claude-Data guarda sesiones, ajustes e inicio de sesion, y la propia app limpia su cache
    // (Configuracion > Este equipo > Almacenamiento > Borrar cache); Limpyrid no borra nada dentro.
    cat(
        "claude-desktop-cache",
        "ai-agents",
        &[
            r"%LOCALAPPDATA%\Claude-Data\*\Cache",
            r"%LOCALAPPDATA%\Claude-Data\*\Code Cache",
            r"%LOCALAPPDATA%\Claude-Data\*\GPUCache",
            r"%LOCALAPPDATA%\Claude\logs",
        ],
        Kind::InfoOnly,
    ),
    cat("claude-desktop-data", "ai-agents", &[r"%LOCALAPPDATA%\Claude-Data"], Kind::InfoOnly),
    cat("claude-code", "ai-agents", &[r"%USERPROFILE%\.claude"], Kind::InfoOnly),
    cat("codex", "ai-agents", &[r"%USERPROFILE%\.codex", r"%LOCALAPPDATA%\OpenAI\Codex"], Kind::InfoOnly),
    cat("antigravity-cache", "ai-agents", &[r"%APPDATA%\Antigravity\Cache", r"%APPDATA%\Antigravity\GPUCache"], C0),
    cat("gemini", "ai-agents", &[r"%USERPROFILE%\.gemini"], Kind::InfoOnly),
    // medido en este equipo el 2026-11-13: carpetas temporales de sesion de Claude, una por proyecto
    cat("claude-sessions", "ai-agents", &[r"%TEMP%\claude"], Kind::Subfolders { min_age_hours: 24 }),
];

#[derive(Clone, Copy)]
pub enum Witness {
    None,
    Sibling(&'static str),
    SiblingExt(&'static str),
    Inside(&'static str),
}

pub struct ProjectRule {
    pub category: &'static str,
    pub dir_names: &'static [&'static str],
    pub witness: Witness,
}

pub const PROJECT_RULES: &[ProjectRule] = &[
    ProjectRule { category: "node-projects", dir_names: &["node_modules", ".next", ".nuxt"], witness: Witness::Sibling("package.json") },
    ProjectRule { category: "rust-target", dir_names: &["target"], witness: Witness::Sibling("Cargo.toml") },
    ProjectRule { category: "python-projects", dir_names: &[".venv", "venv", "env"], witness: Witness::Inside("pyvenv.cfg") },
    ProjectRule { category: "python-projects", dir_names: &["__pycache__", ".pytest_cache"], witness: Witness::None },
    ProjectRule { category: "terraform-projects", dir_names: &[".terraform"], witness: Witness::SiblingExt("tf") },
];

#[derive(Serialize)]
pub struct Found {
    pub category: &'static str,
    pub path: String,
    pub size: u64,
    pub modified: Option<u64>, // segundos Unix; en las de Kind::Project, project_mtime (la fecha la pone categories())
}

const REPARSE_POINT: u32 = 0x400;
const DIRECTORY: u32 = 0x10;

// Sustituye cada %VAR%; None si falta la variable o un `%` queda sin cerrar.
fn substitute(pattern: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let j = after.find('%')?;
        out.push_str(&std::env::var(&after[..j]).ok()?);
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    Some(out)
}

// shortcut: un solo `*` (un segmento completo); ni el comodin ni el resultado final pasan por carpetas reparse.
// Mejora si el catalogo necesita mas.
pub fn expand(pattern: &str) -> Vec<PathBuf> {
    let Some(s) = substitute(pattern) else { return vec![] };
    let mut segs = s.split(['\\', '/']).filter(|c| !c.is_empty());
    let Some(first) = segs.next() else { return vec![] };
    let mut cur = vec![PathBuf::from(if first.ends_with(':') { format!("{first}\\") } else { first.to_string() })];
    for seg in segs {
        cur = if seg == "*" {
            cur.iter()
                .filter_map(|p| fs::read_dir(verbatim(p)).ok())
                .flat_map(|rd| rd.flatten())
                // Atributos directos: std reporta junctions y symlinks de carpeta como symlink, no como dir.
                .filter(|e| e.metadata().is_ok_and(|m| m.file_attributes() & (REPARSE_POINT | DIRECTORY) != REPARSE_POINT | DIRECTORY))
                .map(|e| PathBuf::from(plain(&e.path())))
                .collect()
        } else {
            cur.into_iter().map(|p| p.join(seg)).collect()
        };
    }
    // GetLongPathNameW solo resuelve rutas que existen: filtra y normaliza a forma larga a la vez. Una carpeta
    // final que es un enlace se descarta: limpiar a traves de ella tocaria un destino que nadie comprobo.
    cur.iter()
        .filter_map(|p| long_name(&p.to_string_lossy()))
        .map(|l| PathBuf::from(plain(Path::new(&l))))
        .filter(|p| fs::symlink_metadata(verbatim(p)).is_ok_and(|m| m.file_attributes() & (REPARSE_POINT | DIRECTORY) != REPARSE_POINT | DIRECTORY))
        .collect()
}

fn has_witness(w: Witness, parent: &Path, dir: &Path) -> bool {
    match w {
        Witness::None => true,
        Witness::Sibling(name) => verbatim(parent).join(name).exists(),
        Witness::Inside(name) => verbatim(dir).join(name).exists(),
        Witness::SiblingExt(ext) => fs::read_dir(verbatim(parent))
            .is_ok_and(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))),
    }
}

// Zonas donde no se buscan carpetas de proyecto: lo que hay ahi es software instalado (npm global, plugins,
// Pythons de uv...) que no se regenera con `npm install` en un proyecto. Componentes canonicos (clean::canon).
pub struct NoProjects {
    subtrees: Vec<Vec<String>>,
    profile: Option<Vec<String>>, // sus carpetas de primer nivel que empiezan por `.` tambien se saltan
}

impl NoProjects {
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var_os(k).map(|v| canon(Path::new(&v))).filter(|v| !v.is_empty());
        let profile = get("USERPROFILE");
        // Las zonas de sistema que clean::is_blocked protege con todo su contenido, mas AppData entera (incluye LocalLow).
        let mut subtrees: Vec<_> =
            ["APPDATA", "LOCALAPPDATA", "SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"].into_iter().filter_map(get).collect();
        subtrees.extend(profile.iter().map(|p| [p.as_slice(), &["appdata".to_string()]].concat()));
        NoProjects { subtrees, profile }
    }

    #[cfg(test)]
    pub(crate) fn none() -> Self {
        NoProjects { subtrees: vec![], profile: None }
    }

    pub(crate) fn skips(&self, parts: &[String]) -> bool {
        self.subtrees.iter().any(|r| parts.starts_with(r))
            || self.profile.as_ref().is_some_and(|p| parts.len() > p.len() && parts.starts_with(p) && parts[p.len()].starts_with('.'))
    }
}

impl Default for NoProjects {
    fn default() -> Self {
        Self::from_env()
    }
}

// `parts`: componentes canonicos de `path`; los hijos del arbol ya traen su nombre largo.
fn walk(node: &Node, path: &Path, parts: &mut Vec<String>, skip: &NoProjects, out: &mut Vec<Found>) {
    for c in node.children.iter().filter(|c| c.is_dir) {
        parts.push(c.name.to_lowercase());
        let p = path.join(&c.name);
        if !skip.skips(parts) {
            let rule = PROJECT_RULES.iter().find(|r| r.dir_names.iter().any(|n| n.eq_ignore_ascii_case(&c.name)));
            match rule {
                // Una coincidencia no se recorre: evita listar node_modules anidados.
                Some(r) if has_witness(r.witness, path, &p) => {
                    if !is_blocked(&p) {
                        out.push(Found { category: r.category, path: plain(&p), size: c.size, modified: None });
                    }
                }
                // Lo que hay dentro de un node_modules son paquetes instalados, nunca proyectos del usuario.
                _ if c.name.eq_ignore_ascii_case("node_modules") => {}
                _ => walk(c, &p, parts, skip, out),
            }
        }
        parts.pop();
    }
}

// Los testigos pesan menos de 1 MB y no estan en el arbol: se comprueban en disco.
pub fn detect_projects(result: &ScanResult, skip: &NoProjects) -> Vec<Found> {
    let mut out = vec![];
    let mut parts = canon(&result.root);
    if !skip.skips(&parts) {
        walk(&result.tree, &result.root, &mut parts, skip, &mut out);
    }
    out
}

// Antiguedad efectiva: None si la categoria no la usa; si no, max(minimo del catalogo, pedido o por defecto).
pub fn effective_min_age(cat: &Category, requested_hours: Option<u64>) -> Option<Duration> {
    let (Kind::Contents { min_age_hours } | Kind::Subfolders { min_age_hours } | Kind::Worktrees { min_age_hours }) = cat.kind else { return None };
    (min_age_hours > 0).then(|| Duration::from_secs(min_age_hours.max(requested_hours.unwrap_or(DEFAULT_MIN_AGE_HOURS).min(MIN_AGE_CHOICES_HOURS[4])) * 3600))
}

// Subcarpetas de primer nivel (sin enlaces reparse), con prefijo \\?\; los archivos sueltos no cuentan.
pub fn subfolder_paths(cat: &Category) -> Vec<PathBuf> {
    let mut out = vec![];
    for base in cat.paths.iter().flat_map(|p| expand(p)) {
        let Ok(rd) = fs::read_dir(verbatim(&base)) else { continue };
        out.extend(
            rd.flatten()
                .filter(|e| e.metadata().is_ok_and(|m| m.file_attributes() & (REPARSE_POINT | DIRECTORY) == DIRECTORY))
                .map(|e| e.path()),
        );
    }
    out
}

// Fecha de una carpeta de proyecto, en segundos Unix: el mtime mas reciente entre la carpeta y sus hijos directos.
// Un hijo reparse cuenta por su propia fecha y no se abre; una carpeta reparse no se lista. None si no se puede leer.
// shortcut: aproxima la ultima compilacion o instalacion sin recorrer todo el arbol; recorrerlo si hiciera falta precision.
pub fn project_mtime(path: &Path) -> Option<u64> {
    let meta = fs::symlink_metadata(verbatim(path)).ok()?;
    let mut newest = meta.modified().ok()?;
    if meta.file_attributes() & (REPARSE_POINT | DIRECTORY) == DIRECTORY
        && let Ok(rd) = fs::read_dir(verbatim(path))
    {
        // DirEntry::metadata no sigue enlaces en Windows.
        newest = rd.flatten().filter_map(|e| e.metadata().ok()?.modified().ok()).fold(newest, SystemTime::max);
    }
    newest.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

// Una entrada por subcarpeta; `size` mide cada una y `modified` da su fecha mas reciente (el llamador toma el arbol
// del escaneo si la contiene).
pub fn subfolders(cat: &Category, size: &dyn Fn(&Path) -> u64, modified: &dyn Fn(&Path) -> Option<u64>) -> Vec<Found> {
    subfolder_paths(cat).into_iter().map(|path| Found { category: cat.id, path: plain(&path), size: size(&path), modified: modified(&path) }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{find, scan};
    use std::process::Command;
    use std::sync::atomic::AtomicBool;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    fn measure(p: &Path) -> u64 {
        scan(p, &AtomicBool::new(false), &|_| {}).tree.size
    }

    fn scanned(root: &Path) -> ScanResult {
        scan(root, &AtomicBool::new(false), &|_| {})
    }

    // Los arboles de prueba viven bajo %LOCALAPPDATA%\Temp: sin zonas excluidas para no saltarlos.
    fn found(root: &Path) -> Vec<Found> {
        detect_projects(&scanned(root), &NoProjects::none())
    }

    #[test]
    fn no_entra_en_node_modules() {
        // Forma de un npm global: node_modules sin package.json al lado, paquetes con el suyo dentro.
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join(r"npm\node_modules\pkg\package.json"));
        touch(&t.path().join(r"npm\node_modules\pkg\node_modules\x.bin"));
        assert!(found(t.path()).is_empty());
    }

    #[test]
    fn salta_zonas_instaladas() {
        let t = tempfile::tempdir().unwrap();
        let (appdata, home) = (t.path().join("appdata"), t.path().join("home"));
        for base in [appdata.join("proj"), home.join(r".claude\plugins\p"), home.join(r"work\p")] {
            touch(&base.join("package.json"));
            touch(&base.join(r"node_modules\x.bin"));
        }
        touch(&home.join(r".venv\pyvenv.cfg"));
        let skip = NoProjects { subtrees: vec![canon(&appdata)], profile: Some(canon(&home)) };
        let f = detect_projects(&scanned(t.path()), &skip);
        assert_eq!(f.len(), 1, "solo el proyecto normal");
        assert!(f[0].path.to_lowercase().ends_with(r"home\work\p\node_modules"));
    }

    #[test]
    fn zonas_reales_excluidas() {
        let skip = NoProjects::from_env();
        let env = |k: &str| PathBuf::from(std::env::var_os(k).unwrap());
        for p in [
            env("APPDATA").join(r"npm\node_modules\npm"),
            env("LOCALAPPDATA").join(r"Programs\Python"),
            env("USERPROFILE").join(r".claude\plugins\cache"),
            env("USERPROFILE").join(r"AppData\LocalLow\x"),
            env("ProgramFiles").join("nodejs"),
        ] {
            assert!(skip.skips(&canon(&p)), "{p:?}");
        }
        assert!(!skip.skips(&canon(&env("USERPROFILE").join(r"Proyectos\app"))));
        assert!(!skip.skips(&canon(Path::new(r"D:\src\app"))));
    }

    fn long(p: &Path) -> String {
        plain(Path::new(&long_name(&p.to_string_lossy()).unwrap())).to_lowercase()
    }

    fn lower(v: Vec<PathBuf>) -> Vec<String> {
        let mut o: Vec<String> = v.iter().map(|p| p.to_string_lossy().to_lowercase()).collect();
        o.sort();
        o
    }

    #[test]
    fn expand_variable() {
        let t = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LIMPYRID_TEST_VAR", t.path()) };
        assert_eq!(lower(expand("%LIMPYRID_TEST_VAR%")), vec![long(t.path())]);
    }

    #[test]
    fn expand_variable_ausente() {
        assert!(expand(r"%NO_EXISTE_LIMPYRID%\x").is_empty());
    }

    #[test]
    fn expand_comodin() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("a").join("caches").join("f"));
        touch(&t.path().join("b").join("caches").join("f"));
        touch(&t.path().join("c").join("f"));
        let pat = format!(r"{}\*\caches", t.path().display());
        let want = vec![long(&t.path().join("a").join("caches")), long(&t.path().join("b").join("caches"))];
        assert_eq!(lower(expand(&pat)), want);
    }

    #[test]
    fn expand_comodin_ignora_junction() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("real").join("caches").join("f"));
        let out = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(t.path().join("link"))
            .arg(t.path().join("real"))
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink fallo");
        let pat = format!(r"{}\*\caches", t.path().display());
        assert_eq!(lower(expand(&pat)), vec![long(&t.path().join("real").join("caches"))]);
    }

    #[test]
    fn expand_rechaza_junction_final() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("real").join("f"));
        let out = Command::new("cmd").args(["/c", "mklink", "/J"]).arg(t.path().join("link")).arg(t.path().join("real")).output().unwrap();
        assert!(out.status.success(), "mklink fallo");
        assert!(expand(&format!(r"{}\link", t.path().display())).is_empty());
        assert_eq!(expand(&format!(r"{}\real", t.path().display())).len(), 1);
    }

    #[test]
    fn expand_comodin_archivo_con_espacios() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("a").join("Code Cache").join("f"));
        touch(&t.path().join("b").join("disk.vhdx"));
        let dirs = format!(r"{}\*\Code Cache", t.path().display());
        let files = format!(r"{}\*\disk.vhdx", t.path().display());
        assert_eq!(lower(expand(&dirs)), vec![long(&t.path().join("a").join("Code Cache"))]);
        assert_eq!(lower(expand(&files)), vec![long(&t.path().join("b").join("disk.vhdx"))]);
    }

    #[test]
    fn expand_inexistente() {
        let t = tempfile::tempdir().unwrap();
        assert!(expand(&format!(r"{}\no\existe", t.path().display())).is_empty());
    }

    #[test]
    fn expand_forma_larga() {
        let short = std::env::var("TEMP").unwrap();
        let long_form = plain(Path::new(&long_name(&short).unwrap()));
        let got = expand("%TEMP%");
        assert_eq!(got.len(), 1);
        if long_form.to_lowercase() != short.to_lowercase() {
            assert_eq!(got[0].to_string_lossy().to_lowercase(), long_form.to_lowercase());
            assert!(got[0].components().all(|c| !c.as_os_str().to_string_lossy().contains('~')));
        } else {
            assert!(got[0].exists());
        }
    }

    #[test]
    fn ids_unicos() {
        let mut ids: Vec<_> = CATALOG.iter().map(|c| c.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
        for r in PROJECT_RULES {
            let c = CATALOG.iter().find(|c| c.id == r.category).expect("categoria de regla en CATALOG");
            assert!(matches!(c.kind, Kind::Project));
        }
        assert!(CATALOG.iter().filter(|c| matches!(c.kind, Kind::Project)).all(|c| c.paths.is_empty()));
    }

    fn by_id(id: &str) -> &'static Category {
        CATALOG.iter().find(|c| c.id == id).unwrap()
    }

    // Filas que compartian carpeta con datos que no son cache (credenciales, entornos, bases de datos).
    #[test]
    fn filas_sin_datos_ajenos() {
        assert_eq!(by_id("huggingface").paths, [r"%USERPROFILE%\.cache\huggingface\hub"]);
        assert_eq!(by_id("poetry").exclude, ["virtualenvs"]);
        assert_eq!(by_id("deno").exclude, ["location_data"]);
        assert_eq!(by_id("dart-pub").paths, [r"%LOCALAPPDATA%\Pub\Cache\hosted", r"%LOCALAPPDATA%\Pub\Cache\git"]);
        // Los datos de Claude Desktop y de Claude Code se gestionan desde esas apps: nada borrable dentro.
        for c in CATALOG.iter().filter(|c| matches!(c.kind, Kind::Contents { .. } | Kind::Project | Kind::Subfolders { .. })) {
            for p in c.paths {
                let l = p.to_lowercase();
                assert!(!l.starts_with(r"%localappdata%\claude-data") && !l.starts_with(r"%userprofile%\.claude"), "{} {p}", c.id);
            }
        }
    }

    #[test]
    fn edad_minima_piso() {
        let c = by_id("temp-user");
        let h = |r| effective_min_age(c, r).unwrap().as_secs() / 3600;
        assert_eq!(h(Some(1)), 24);
        assert_eq!(h(None), 168);
        assert_eq!(h(Some(720)), 720);
    }

    #[test]
    fn edad_minima_no_aplica() {
        assert!(effective_min_age(by_id("npm"), Some(720)).is_none());
    }

    #[test]
    fn catalogo_sesiones_claude() {
        let c = by_id("claude-sessions");
        assert!(matches!(c.kind, Kind::Subfolders { min_age_hours: 24 }));
        assert_eq!(c.ecosystem, "ai-agents");
        assert!(by_id("temp-user").exclude.contains(&"claude"));
        assert!(MIN_AGE_CHOICES_HOURS.iter().all(|&h| h >= 24));
        assert!(MIN_AGE_CHOICES_HOURS.contains(&DEFAULT_MIN_AGE_HOURS));
    }

    #[test]
    fn subcarpetas_lista() {
        let t = tempfile::tempdir().unwrap();
        let mb = |p: &str, n: usize| {
            fs::create_dir_all(t.path().join(p).parent().unwrap()).unwrap();
            fs::write(t.path().join(p), vec![0u8; n << 20]).unwrap();
        };
        mb("a/x.bin", 2);
        mb("b/y.bin", 1);
        mb("suelto.bin", 1);
        // Variable propia de esta prueba; ninguna otra la lee.
        unsafe { std::env::set_var("LIMPYRID_TEST_SUBCARPETAS", t.path()) };
        let test_cat = cat("t", "t", &[r"%LIMPYRID_TEST_SUBCARPETAS%"], Kind::Subfolders { min_age_hours: 24 });
        let mut f = subfolders(&test_cat, &measure, &project_mtime);
        f.sort_by_key(|x| x.size);
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].size, f[1].size), (1 << 20, 2 << 20));
        assert!(f.iter().all(|x| x.modified.is_some() && x.category == "t"));
    }

    #[test]
    fn subcarpetas_actividad_profunda() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::SystemTime;
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("s/x/y/nuevo.bin"));
        let set = |p: &str, at: SystemTime| {
            fs::File::options().write(true).custom_flags(0x0200_0000).open(t.path().join(p)).unwrap().set_modified(at).unwrap()
        };
        let fresh = UNIX_EPOCH + Duration::from_secs(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 3600);
        let old = SystemTime::now() - Duration::from_secs(10 * 24 * 3600);
        set("s/x/y/nuevo.bin", fresh);
        for p in ["s/x/y", "s/x", "s"] {
            set(p, old);
        }
        // Variable propia de esta prueba; ninguna otra la lee.
        unsafe { std::env::set_var("LIMPYRID_TEST_SUBCARPETAS_EDAD", t.path()) };
        let c = cat("t", "t", &[r"%LIMPYRID_TEST_SUBCARPETAS_EDAD%"], Kind::Subfolders { min_age_hours: 24 });
        // La actividad profunda llega por el arbol del escaneo (raiz en forma larga, como las rutas del catalogo).
        let r = scanned(&fs::canonicalize(t.path()).unwrap());
        let f = subfolders(&c, &measure, &|p| find(&r, p).map(|n| n.newest));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].modified, Some(fresh.duration_since(UNIX_EPOCH).unwrap().as_secs()));
    }

    #[test]
    fn edad_minima_desbordamiento() {
        let d = effective_min_age(by_id("temp-user"), Some(u64::MAX)).unwrap();
        assert_eq!(d.as_secs(), 720 * 3600);
    }

    #[test]
    fn subcarpetas_ignora_junction() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("real").join("x.bin"));
        let out = Command::new("cmd").args(["/c", "mklink", "/J"]).arg(t.path().join("link")).arg(t.path().join("real")).output().unwrap();
        assert!(out.status.success(), "mklink fallo");
        // Variable propia de esta prueba; ninguna otra la lee.
        unsafe { std::env::set_var("LIMPYRID_TEST_SUBCARPETAS_JUNCTION", t.path()) };
        let c = cat("t", "t", &[r"%LIMPYRID_TEST_SUBCARPETAS_JUNCTION%"], Kind::Subfolders { min_age_hours: 24 });
        let f = subfolders(&c, &measure, &|_| None);
        assert_eq!(f.len(), 1);
        assert!(f[0].path.to_lowercase().ends_with("real"));
    }

    #[test]
    fn detecta_node_modules() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("p").join("package.json"));
        touch(&t.path().join("p").join("node_modules").join("x.bin"));
        let f = found(t.path());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "node-projects");
        assert_eq!(f[0].size, 1);
        assert!(f[0].path.to_lowercase().ends_with(r"p\node_modules"));
    }

    #[test]
    fn ignora_sin_testigo() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("q").join("node_modules").join("x.bin"));
        touch(&t.path().join("r").join("target").join("x.bin"));
        assert!(found(t.path()).is_empty());
    }

    #[test]
    fn no_lista_anidados() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("p").join("package.json"));
        touch(&t.path().join("p").join("node_modules").join("dep").join("package.json"));
        touch(&t.path().join("p").join("node_modules").join("dep").join("node_modules").join("x.bin"));
        assert_eq!(found(t.path()).len(), 1);
    }

    #[test]
    fn detecta_venv() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("p").join(".venv").join("pyvenv.cfg"));
        touch(&t.path().join("p").join("env").join("x.bin"));
        let f = found(t.path());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "python-projects");
    }

    #[test]
    fn detecta_terraform() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("p").join("main.tf"));
        touch(&t.path().join("p").join(".terraform").join("x.bin"));
        let f = found(t.path());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "terraform-projects");
    }

    #[test]
    fn detecta_rust_y_pycache_por_nombre() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("c").join("Cargo.toml"));
        touch(&t.path().join("c").join("target").join("x.bin"));
        touch(&t.path().join("py").join("__pycache__").join("m.pyc"));
        let mut cats: Vec<_> = found(t.path()).iter().map(|f| f.category).collect();
        cats.sort();
        assert_eq!(cats, vec!["python-projects", "rust-target"]);
    }
}
