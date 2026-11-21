use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

const MAX_FILE: u64 = 16 * 1024 * 1024;

#[derive(Default)]
pub struct Playlist {
    pub current: Option<usize>,
    pub entries: Vec<(PathBuf, bool)>,
}

pub fn directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join("UltraMP3Windows"))
}

pub fn read_playlist(path: &Path) -> io::Result<Playlist> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE || bytes.len() % 2 != 0 || !bytes.starts_with(&[0xff, 0xfe]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Playlist harus UTF-16LE dengan BOM.",
        ));
    }
    let words: Vec<_> = bytes[2..]
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect();
    let mut lines = words
        .split(|&c| c == b'\n' as u16)
        .map(|line| line.strip_suffix(&[13]).unwrap_or(line));
    let current = lines
        .next()
        .and_then(|line| String::from_utf16(line).ok())
        .and_then(|value| value.trim().parse::<usize>().ok());
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut entries = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let marked = line.first() == Some(&(b'*' as u16));
        let name = if marked { &line[1..] } else { line };
        if name.is_empty() || name.contains(&0) {
            continue;
        }
        let entry = PathBuf::from(OsString::from_wide(name));
        let entry = if entry.has_root() || entry.is_absolute() {
            entry
        } else {
            parent.join(entry)
        };
        entries.push((entry, marked));
    }
    let current = current.filter(|&index| index < entries.len());
    Ok(Playlist { current, entries })
}

pub fn write_playlist<'a>(
    path: &Path,
    current: Option<usize>,
    entries: impl IntoIterator<Item = (&'a Path, bool)>,
) -> io::Result<()> {
    let mut contents = vec![0xff, 0xfe];
    let index = current.unwrap_or(0);
    write_words(&mut contents, format!("{index}\r\n").encode_utf16());
    for (entry, marked) in entries {
        if marked {
            write_words(&mut contents, [b'*' as u16]);
        }
        write_words(&mut contents, entry.as_os_str().encode_wide());
        write_words(&mut contents, [13, 10]);
        if contents.len() as u64 > MAX_FILE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Playlist melebihi 16 MiB.",
            ));
        }
    }
    atomic_write(path, &contents)
}

fn write_words(bytes: &mut Vec<u8>, words: impl IntoIterator<Item = u16>) {
    bytes.extend(words.into_iter().flat_map(u16::to_le_bytes));
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_playlist_wire_format_and_unicode_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("playlist.ini");
        let entries = [
            (Path::new(r"C:\Music\Lagu é.mp3"), true),
            (Path::new(r"E:\MOD\Demo.s3m"), false),
        ];
        write_playlist(&file, Some(1), entries).unwrap();
        let bytes = fs::read(&file).unwrap();
        let mut expected = vec![0xff, 0xfe];
        write_words(
            &mut expected,
            "1\r\n*C:\\Music\\Lagu é.mp3\r\nE:\\MOD\\Demo.s3m\r\n".encode_utf16(),
        );
        assert_eq!(bytes, expected);
        let restored = read_playlist(&file).unwrap();
        assert_eq!(restored.current, Some(1));
        assert_eq!(
            restored.entries,
            entries.map(|(path, mark)| (path.to_path_buf(), mark))
        );
        write_playlist(&file, None, []).unwrap();
        assert!(read_playlist(&file).unwrap().entries.is_empty());
    }
}

pub fn scan_audio(root: &Path) -> io::Result<Vec<PathBuf>> {
    use std::os::windows::fs::MetadataExt;
    let mut paths = Vec::new();
    let root_entries = fs::read_dir(root)?;
    let mut pending = Vec::new();
    let mut directory = Some(root_entries);
    while let Some(entries) = directory.take() {
        for entry in entries.flatten() {
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if metadata.file_attributes() & 0x400 != 0 {
                continue; // Skip junctions, symlinks and other reparse points.
            }
            let path = entry.path();
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file()
                && path.extension().is_some_and(|ext| {
                    ["mp3", "wav", "s3m", "mod", "xm", "it"]
                        .iter()
                        .any(|kind| ext.eq_ignore_ascii_case(kind))
                })
            {
                paths.push(path);
            }
        }
        directory = pending.pop().and_then(|mut path| loop {
            if let Ok(next) = fs::read_dir(path) {
                break Some(next);
            }
            let Some(next) = pending.pop() else {
                break None;
            };
            path = next;
        });
    }
    paths.sort_unstable();
    Ok(paths)
}

#[link(name = "kernel32")]
extern "system" {
    fn MoveFileW(source: *const u16, destination: *const u16) -> i32;
}

pub fn move_audio(source: &Path, destination: &Path) -> io::Result<()> {
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let source = wide(source);
    let destination = wide(destination);
    if unsafe { MoveFileW(source.as_ptr(), destination.as_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn copy_audio(source: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut source = File::open(source)?;
    io::copy(&mut source, &mut temp)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn copy_move_and_scan_do_not_overwrite_existing_tracks() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.MP3");
        let child = root.path().join("album");
        fs::create_dir(&child).unwrap();
        let copy = child.join("copy.mp3");
        let moved = child.join("moved.s3m");
        fs::write(&source, b"audio").unwrap();
        fs::write(root.path().join("ignore.txt"), b"other").unwrap();
        copy_audio(&source, &copy).unwrap();
        assert!(copy_audio(&source, &copy).is_err());
        assert!(move_audio(&source, &copy).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"audio");
        move_audio(&source, &moved).unwrap();
        assert_eq!(scan_audio(root.path()).unwrap(), vec![copy, moved]);
    }
}
