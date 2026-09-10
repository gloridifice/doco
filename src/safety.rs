//! Conservative filesystem operations. Locks serialize doco writers, not arbitrary editors.
use anyhow::{Context, Result, bail};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;

fn inspect_node(path: &Path, meta: &Metadata) -> Result<()> {
    if meta.file_type().is_symlink() {
        bail!("refusing symbolic link: {}", path.display());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            bail!("refusing reparse point: {}", path.display());
        }
    }
    if meta.is_file() && link_count(path, meta)? != 1 {
        bail!("refusing hard-linked file: {}", path.display());
    }
    if !meta.is_file() && !meta.is_dir() {
        bail!("unsupported filesystem object: {}", path.display());
    }
    Ok(())
}
#[cfg(unix)]
fn link_count(_: &Path, meta: &Metadata) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(meta.nlink())
}
#[cfg(windows)]
fn link_count(path: &Path, _: &Metadata) -> Result<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let file = File::open(path)?;
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the file handle is live and the output buffer has the required layout.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { info.assume_init() }.nNumberOfLinks as u64)
}
#[cfg(not(any(unix, windows)))]
fn link_count(_: &Path, _: &Metadata) -> Result<u64> {
    bail!("cannot verify file ownership on this platform")
}

pub fn inspect_absolute(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, Component::ParentDir) {
            bail!("parent traversal is not allowed: {}", path.display());
        }
        current.push(part);
        // A Windows volume prefix (e.g. \\?\C:) is not a filesystem node
        // until the following RootDir component has been appended.
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) => inspect_node(&current, &meta)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("inspect {}", current.display())),
        }
    }
    Ok(())
}
pub fn inspect(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        bail!("path outside project: {}", path.display());
    }
    inspect_absolute(path)
}
pub fn mkdir(root: &Path, path: &Path) -> Result<()> {
    inspect(root, path)?;
    let relative = path.strip_prefix(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                inspect(root, &current)?;
                if !current.is_dir() {
                    bail!("not a directory: {}", current.display());
                }
            }
            Err(e) => {
                return Err(e).with_context(|| format!("create directory {}", current.display()));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub bytes: Vec<u8>,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    readonly: bool,
}
pub fn snapshot(root: &Path, path: &Path) -> Result<Option<Snapshot>> {
    inspect(root, path)?;
    match fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_file() {
                bail!("expected regular file: {}", path.display());
            }
            let bytes = fs::read(path)?;
            let after = fs::metadata(path)?;
            if meta.len() != after.len() || meta.modified().ok() != after.modified().ok() {
                bail!("file changed while reading: {}", path.display());
            }
            Ok(Some(Snapshot {
                bytes,
                modified: meta.modified().ok(),
                created: meta.created().ok(),
                readonly: meta.permissions().readonly(),
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn text(root: &Path, path: &Path) -> Result<String> {
    let snap = snapshot(root, path)?.with_context(|| format!("missing {}", path.display()))?;
    decode(&snap.bytes)
        .with_context(|| format!("{}: only UTF-8 (optional BOM) is supported", path.display()))
}
pub fn decode(bytes: &[u8]) -> Result<String> {
    Ok(std::str::from_utf8(bytes)?.to_string())
}
pub fn verify(root: &Path, path: &Path, expected: &Option<Snapshot>) -> Result<()> {
    if &snapshot(root, path)? != expected {
        bail!(
            "concurrent modification: {}; refusing overwrite",
            path.display()
        );
    }
    Ok(())
}
pub fn atomic_write(
    root: &Path,
    path: &Path,
    expected: &Option<Snapshot>,
    bytes: &[u8],
) -> Result<()> {
    verify(root, path, expected)?;
    let parent = path.parent().context("missing parent")?;
    mkdir(root, parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    if expected.is_some() {
        temp.as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
    }
    temp.as_file().sync_all()?;
    verify(root, path, expected)?;
    if expected.is_some() {
        temp.persist(path)
            .map_err(|e| e.error)
            .with_context(|| format!("atomic replace {}", path.display()))?;
    } else {
        temp.persist_noclobber(path)
            .map_err(|e| e.error)
            .with_context(|| format!("atomic create {}", path.display()))?;
    }
    Ok(())
}

pub struct Lock {
    _file: File,
}
impl Lock {
    pub fn acquire(root: &Path) -> Result<Self> {
        let directory = root.join("doco/tmp");
        mkdir(root, &directory)?;
        let path = directory.join(".doco.lock");
        inspect(root, &path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        fs2::FileExt::try_lock_exclusive(&file)
            .context("another doco writer is running; retry later")?;
        inspect(root, &path)?;
        // Never unlink a lock file: removing a live inode would allow a second lock.
        Ok(Self { _file: file })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub directory: bool,
    len: u64,
    modified: Option<std::time::SystemTime>,
}
impl TreeEntry {
    pub fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }
}
pub fn tree(root: &Path, path: &Path) -> Result<Vec<TreeEntry>> {
    fn walk(root: &Path, path: &Path, out: &mut Vec<TreeEntry>) -> Result<()> {
        inspect(root, path)?;
        let meta = fs::metadata(path)?;
        out.push(TreeEntry {
            path: path.to_path_buf(),
            directory: meta.is_dir(),
            len: meta.len(),
            modified: meta.modified().ok(),
        });
        if meta.is_dir() {
            let mut entries = fs::read_dir(path)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            entries.sort();
            for child in entries {
                walk(root, &child, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, path, &mut out)?;
    Ok(out)
}
pub fn move_directory(root: &Path, from: &Path, to: &Path) -> Result<()> {
    tree(root, from)?;
    inspect(root, to)?;
    if to.try_exists()? {
        bail!("destination already exists: {}", to.display());
    }
    fs::rename(from, to).with_context(|| {
        format!(
            "move failed; source remains at {}, destination {} was not confirmed",
            from.display(),
            to.display()
        )
    })
}
