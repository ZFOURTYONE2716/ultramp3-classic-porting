use libloading::os::windows::{
    Library, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
};
use std::{
    ffi::{c_char, c_void},
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    ptr,
};
use tempfile::TempPath;

const RATE: u32 = 44_100;
const MAX_FRAMES: u32 = RATE * 60 * 20;
const MAX_INPUT: u64 = 64 * 1024 * 1024;
type Create = unsafe extern "C" fn(
    *const u8,
    usize,
    Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    *mut c_void,
    Option<unsafe extern "C" fn(i32, *mut c_void)>,
    *mut c_void,
    *mut i32,
    *mut *const c_char,
    *const c_void,
) -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
type Render = unsafe extern "C" fn(*mut c_void, i32, usize, *mut i16) -> usize;
type LastError = unsafe extern "C" fn(*mut c_void) -> i32;

struct Module {
    handle: *mut c_void,
    destroy: Destroy,
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.handle) };
    }
}

pub fn is_tracker(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["s3m", "mod", "xm", "it"]
                .iter()
                .any(|format| extension.eq_ignore_ascii_case(format))
        })
}

pub fn decode(path: &Path) -> Result<TempPath, String> {
    let library_path = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .with_file_name("libopenmpt.dll");
    let library = unsafe {
        Library::load_with_flags(
            library_path,
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        )
    }
    .map_err(|error| format!("Gagal membuka libopenmpt.dll di samping aplikasi: {error}"))?;
    let create = unsafe { library.get::<Create>(b"openmpt_module_create_from_memory2") }
        .map_err(|error| error.to_string())?;
    let destroy = unsafe { library.get::<Destroy>(b"openmpt_module_destroy") }
        .map_err(|error| error.to_string())?;
    let render = unsafe { library.get::<Render>(b"openmpt_module_read_interleaved_stereo") }
        .map_err(|error| error.to_string())?;
    let last_error = unsafe { library.get::<LastError>(b"openmpt_module_error_get_last") }
        .map_err(|error| error.to_string())?;
    let mut data = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_INPUT + 1).read_to_end(&mut data))
        .map_err(|error| error.to_string())?;
    if data.len() as u64 > MAX_INPUT {
        return Err("Ukuran modul melebihi 64 MiB.".into());
    }
    let mut error = 0;
    let handle = unsafe {
        create(
            data.as_ptr(),
            data.len(),
            None,
            ptr::null_mut(),
            None,
            ptr::null_mut(),
            &mut error,
            ptr::null_mut(),
            ptr::null(),
        )
    };
    if handle.is_null() {
        return Err(format!("Modul tracker tidak bisa dibuka ({error})."));
    }
    let module = Module {
        handle,
        destroy: *destroy,
    };
    let mut wav = tempfile::Builder::new()
        .suffix(".wav")
        .tempfile()
        .map_err(|error| error.to_string())?;
    write_pcm(wav.as_file_mut(), |samples| {
        let frames = unsafe {
            render(
                module.handle,
                RATE as i32,
                samples.len() / 2,
                samples.as_mut_ptr(),
            )
        };
        let error = unsafe { last_error(module.handle) };
        if error != 0 {
            return Err(std::io::Error::other(format!(
                "Dekode modul gagal ({error})."
            )));
        }
        Ok(frames)
    })
    .map_err(|error| error.to_string())?;
    Ok(wav.into_temp_path())
}

fn write_pcm(
    file: &mut File,
    mut render: impl FnMut(&mut [i16]) -> std::io::Result<usize>,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(file);
    writer.write_all(b"RIFF")?;
    writer.write_all(&0u32.to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&2u16.to_le_bytes())?;
    writer.write_all(&RATE.to_le_bytes())?;
    writer.write_all(&(RATE * 4).to_le_bytes())?;
    writer.write_all(&4u16.to_le_bytes())?;
    writer.write_all(&16u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&0u32.to_le_bytes())?;
    let mut samples = [0i16; 8192];
    let mut pcm = [0u8; 16384];
    let mut total = 0;
    loop {
        let frames = render(&mut samples)?;
        if frames == 0 {
            break;
        }
        total += frames as u32;
        if total > MAX_FRAMES {
            return Err(std::io::Error::other("Durasi modul melebihi 20 menit."));
        }
        for (sample, bytes) in samples[..frames * 2].iter().zip(pcm.chunks_exact_mut(2)) {
            bytes.copy_from_slice(&sample.to_le_bytes());
        }
        writer.write_all(&pcm[..frames * 4])?;
    }
    if total == 0 {
        return Err(std::io::Error::other("Modul tidak menghasilkan audio."));
    }
    let data_size = total * 4;
    writer.seek(SeekFrom::Start(4))?;
    writer.write_all(&(36 + data_size).to_le_bytes())?;
    writer.seek(SeekFrom::Start(40))?;
    writer.write_all(&data_size.to_le_bytes())?;
    writer.flush()
}
