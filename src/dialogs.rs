use std::{
    ffi::c_void,
    path::{Path, PathBuf},
};
#[repr(C)]

struct OpenFileName {
    size: u32,
    owner: isize,
    instance: isize,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_ext: u16,
    default_ext: *const u16,
    custom_data: isize,
    hook: *const c_void,
    template: *const u16,
    reserved: *const c_void,
    reserved2: u32,
    flags_ex: u32,
}
#[link(name = "comdlg32")]
extern "system" {

    fn GetOpenFileNameW(p: *mut OpenFileName) -> i32;
    fn GetSaveFileNameW(p: *mut OpenFileName) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
use std::os::windows::ffi::{OsStrExt, OsStringExt};

pub fn choose_files() -> Vec<PathBuf> {
    file_dialog(
        "Audio\0*.mp3;*.wav;*.s3m;*.mod;*.xm;*.it\0Semua file\0*.*\0",
        true,
        None,
    )
}

pub fn playlist_dialog(save: bool) -> Option<PathBuf> {
    file_dialog(
        "UltraMP3 Playlist\0*.ini\0",
        false,
        save.then(|| Path::new("playlist.ini")),
    )
    .into_iter()
    .next()
}

pub fn file_dialog(filter: &str, multiple: bool, save: Option<&Path>) -> Vec<PathBuf> {
    let filter = wide(filter);
    let mut files = vec![0u16; 65_536];
    let mut dialog: OpenFileName = unsafe { std::mem::zeroed() };
    dialog.size = std::mem::size_of::<OpenFileName>() as u32;
    dialog.filter = filter.as_ptr();
    dialog.filter_index = 1;
    dialog.file = files.as_mut_ptr();
    dialog.max_file = files.len() as u32;
    dialog.flags = 0x00000800 | 0x00080000 | 0x00000008;
    if multiple {
        dialog.flags |= 0x00000200;
    }
    let extension: Vec<u16> = save
        .and_then(Path::extension)
        .map(|extension| extension.encode_wide().chain(Some(0)).collect())
        .unwrap_or_default();
    let success = if let Some(path) = save {
        dialog.flags |= 0x00000002;
        if !extension.is_empty() {
            dialog.default_ext = extension.as_ptr();
        }
        let name: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if name.len() >= files.len() {
            return Vec::new();
        }
        files[..name.len()].copy_from_slice(&name);
        unsafe { GetSaveFileNameW(&mut dialog) }
    } else {
        dialog.flags |= 0x00001000;
        unsafe { GetOpenFileNameW(&mut dialog) }
    };
    if success == 0 {
        return Vec::new();
    }
    let mut entries = files
        .split(|&unit| unit == 0)
        .take_while(|entry| !entry.is_empty())
        .map(|entry| PathBuf::from(std::ffi::OsString::from_wide(entry)));
    let Some(first) = entries.next() else {
        return Vec::new();
    };
    let mut selected: Vec<_> = entries.map(|entry| first.join(entry)).collect();
    if selected.is_empty() {
        selected.push(first);
    }
    selected
}

#[repr(C)]
struct BrowseInfo {
    owner: isize,
    root: *const c_void,
    display_name: *mut u16,
    title: *const u16,
    flags: u32,
    callback: usize,
    param: isize,
    image: i32,
}

#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(info: *mut BrowseInfo) -> *mut c_void;
    fn SHGetPathFromIDListW(id: *const c_void, path: *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(memory: *mut c_void);
}

pub fn choose_folder() -> Option<PathBuf> {
    let title = wide("Pilih folder musik");
    let mut display = [0; 260];
    let mut info: BrowseInfo = unsafe { std::mem::zeroed() };
    info.display_name = display.as_mut_ptr();
    info.title = title.as_ptr();
    info.flags = 1; // BIF_RETURNONLYFSDIRS
    let id = unsafe { SHBrowseForFolderW(&mut info) };
    if id.is_null() {
        return None;
    }
    let mut path = [0; 260];
    let valid = unsafe { SHGetPathFromIDListW(id, path.as_mut_ptr()) != 0 };
    unsafe { CoTaskMemFree(id) };
    let end = path.iter().position(|&unit| unit == 0)?;
    valid.then(|| PathBuf::from(std::ffi::OsString::from_wide(&path[..end])))
}
