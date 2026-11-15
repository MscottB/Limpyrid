use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    RamDisk,
    Unknown,
}

#[derive(Serialize)]
pub struct Drive {
    pub root: String,
    pub label: String,
    pub kind: DriveKind,
    pub usb: bool,
    pub system: bool,
    pub filesystem: String,
    pub total: u64,
    pub free: u64,
}

use std::ffi::c_void;

const SEM_FAILCRITICALERRORS: u32 = 1;
const OPEN_EXISTING: u32 = 3;
const SHARE_READ_WRITE: u32 = 3;
const INVALID_HANDLE: isize = -1;
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
const BUS_TYPE_USB: u32 = 7;
const BUS_TYPE_OFFSET: usize = 28; // STORAGE_DEVICE_DESCRIPTOR.BusType

#[repr(C)]
struct StoragePropertyQuery {
    property_id: u32, // StorageDeviceProperty = 0
    query_type: u32,  // PropertyStandardQuery = 0
    extra: [u8; 4],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLogicalDrives() -> u32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetVolumeInformationW(
        root: *const u16,
        label: *mut u16,
        label_len: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        fs: *mut u16,
        fs_len: u32,
    ) -> i32;
    fn GetDiskFreeSpaceExW(dir: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
    fn SetThreadErrorMode(new_mode: u32, old_mode: *mut u32) -> i32;
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *const c_void, disposition: u32, flags: u32, template: isize) -> isize;
    fn DeviceIoControl(
        handle: isize,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

pub fn kind_from(code: u32) -> DriveKind {
    match code {
        2 => DriveKind::Removable,
        3 => DriveKind::Fixed,
        4 => DriveKind::Network,
        5 => DriveKind::Optical,
        6 => DriveKind::RamDisk,
        _ => DriveKind::Unknown,
    }
}

pub fn roots_from(mask: u32) -> Vec<String> {
    (0..26u8).filter(|i| mask >> i & 1 == 1).map(|i| format!("{}:\\", (b'A' + i) as char)).collect()
}

pub fn is_system(root: &str, system_drive: &str) -> bool {
    root.trim_end_matches('\\').eq_ignore_ascii_case(system_drive.trim_end_matches('\\'))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn until_nul(buf: &[u16]) -> String {
    String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())])
}

// Etiqueta y sistema de archivos; vacios si la unidad no responde.
fn volume_info(root: &[u16]) -> (String, String) {
    let (mut label, mut fs) = ([0u16; 261], [0u16; 261]);
    // SAFETY: `root` termina en nul; los buffers miden 261 u16 y se pasan esas longitudes; los punteros opcionales son nulos.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    if ok == 0 { (String::new(), String::new()) } else { (until_nul(&label), until_nul(&fs)) }
}

// (total, libre para el usuario); (0, 0) si la unidad no responde.
fn space(root: &[u16]) -> (u64, u64) {
    let (mut free, mut total, mut any) = (0u64, 0u64, 0u64);
    // SAFETY: `root` termina en nul y los tres punteros apuntan a u64 vivos en esta pila.
    let ok = unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut free, &mut total, &mut any) };
    if ok == 0 { (0, 0) } else { (total, free.min(total)) }
}

// Tipo de bus del dispositivo; el asa se abre con acceso 0 (solo metadatos) y se cierra siempre.
fn is_usb(letter: char) -> bool {
    let name = wide(&format!(r"\\.\{letter}:"));
    // SAFETY: `name` termina en nul; acceso 0 no lee ni escribe el dispositivo; sin seguridad ni plantilla.
    let h = unsafe { CreateFileW(name.as_ptr(), 0, SHARE_READ_WRITE, std::ptr::null(), OPEN_EXISTING, 0, 0) };
    if h == INVALID_HANDLE || h == 0 {
        return false;
    }
    let query = StoragePropertyQuery { property_id: 0, query_type: 0, extra: [0; 4] };
    let mut out = [0u8; 1024];
    let mut returned = 0u32;
    // SAFETY: `h` es un asa valida; entrada y salida son buffers vivos con sus longitudes reales; sin solapamiento.
    let ok = unsafe {
        DeviceIoControl(
            h,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const StoragePropertyQuery).cast(),
            size_of::<StoragePropertyQuery>() as u32,
            out.as_mut_ptr().cast(),
            out.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: `h` es el asa abierta arriba y se cierra una sola vez, en toda ruta.
    unsafe { CloseHandle(h) };
    ok != 0 && returned as usize >= BUS_TYPE_OFFSET + 4 && {
        let bus = u32::from_le_bytes(out[BUS_TYPE_OFFSET..BUS_TYPE_OFFSET + 4].try_into().unwrap());
        bus == BUS_TYPE_USB
    }
}

// Solo lectura. Con SEM_FAILCRITICALERRORS un lector vacio falla en silencio en vez de mostrar
// "No hay disco en la unidad" (documentado en SetThreadErrorMode); el modo anterior se restaura.
pub fn drives() -> Vec<Drive> {
    let mut old = 0u32;
    // SAFETY: `old` es un u32 vivo; el modo es por hilo y se restaura abajo.
    unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, &mut old) };
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    // SAFETY: sin parametros ni punteros.
    let mask = unsafe { GetLogicalDrives() };
    let list = roots_from(mask)
        .into_iter()
        .map(|root| {
            let w = wide(&root);
            // SAFETY: `w` termina en nul.
            let kind = kind_from(unsafe { GetDriveTypeW(w.as_ptr()) });
            let (label, filesystem) = volume_info(&w);
            let (total, free) = space(&w);
            let usb = kind != DriveKind::Network && is_usb(root.chars().next().unwrap_or('C'));
            Drive { system: is_system(&root, &system_drive), root, label, kind, usb, filesystem, total, free }
        })
        .collect();
    // SAFETY: restaura el modo guardado; el puntero anterior es nulo.
    unsafe { SetThreadErrorMode(old, std::ptr::null_mut()) };
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos() {
        assert_eq!(kind_from(2), DriveKind::Removable);
        assert_eq!(kind_from(3), DriveKind::Fixed);
        assert_eq!(kind_from(4), DriveKind::Network);
        assert_eq!(kind_from(5), DriveKind::Optical);
        assert_eq!(kind_from(6), DriveKind::RamDisk);
        assert_eq!(kind_from(0), DriveKind::Unknown);
        assert_eq!(kind_from(1), DriveKind::Unknown);
        assert_eq!(kind_from(99), DriveKind::Unknown);
    }

    #[test]
    fn raices() {
        assert_eq!(roots_from(0b101), vec!["A:\\", "C:\\"]);
        assert!(roots_from(0).is_empty());
    }

    #[test]
    fn sistema() {
        assert!(is_system("c:\\", "C:"));
        assert!(!is_system("D:\\", "C:"));
    }

    #[test]
    fn unidades_reales() {
        let all = drives();
        let sys: Vec<_> = all.iter().filter(|d| d.system).collect();
        assert_eq!(sys.len(), 1, "{:?}", all.iter().map(|d| &d.root).collect::<Vec<_>>());
        assert_eq!(sys[0].kind, DriveKind::Fixed);
        assert!(sys[0].total > 0);
        for d in &all {
            assert!(d.free <= d.total, "{}", d.root);
            let b = d.root.as_bytes();
            assert!(b.len() == 3 && b[0].is_ascii_uppercase() && &d.root[1..] == ":\\", "{}", d.root);
        }
    }
}
