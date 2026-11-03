//! No-follow app-home descriptors and checked atomic replacement of owned files.
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, String>;
pub const FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn file_digest(file: &File) -> Result<String> {
    file_digest_until(file, None)
}
pub fn file_digest_until(file: &File, deadline: Option<Instant>) -> Result<String> {
    let mut input = file.try_clone().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err("private proof timed out".into());
        }
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub dev: u64,
    pub ino: u64,
    pub mode: u32,
    pub len: u64,
    modified: (i64, i64),
}
impl Stamp {
    fn of(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode() & 0o7777,
            len: m.len(),
            modified: (m.mtime(), m.mtime_nsec()),
        }
    }
}
pub struct Known {
    pub bytes: Vec<u8>,
    pub stamp: Stamp,
}
pub struct Home {
    pub fd: File,
    pub path: PathBuf,
}

fn relative(path: &str) -> Result<Vec<&str>> {
    if path.is_empty()
        || path.starts_with('/')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("unsafe relative runtime path".into());
    }
    Ok(path.split('/').collect())
}
fn regular(file: &File) -> Result<Metadata> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.nlink() != 1 {
        return Err("runtime path is not a single-link regular file".into());
    }
    Ok(m)
}
impl Home {
    pub fn open(path: &Path) -> Result<Self> {
        let clean: PathBuf = path.components().collect();
        if !path.is_absolute()
            || path == Path::new("/")
            || clean.as_os_str() != path.as_os_str()
            || path
                .components()
                .any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
        {
            return Err("app home must be an absolute canonical existing directory".into());
        }
        let mut fd: File = fs::open("/", FLAGS, Mode::empty())
            .map_err(|e| e.to_string())?
            .into();
        for component in path.components() {
            if let Component::Normal(name) = component {
                fd = fs::openat(&fd, name, FLAGS, Mode::empty())
                    .map_err(|e| format!("unsafe app home: {e}"))?
                    .into();
            }
        }
        Ok(Self {
            fd,
            path: path.into(),
        })
    }
    pub fn directory(&self, path: &str) -> Result<File> {
        let mut fd = self.fd.try_clone().map_err(|e| e.to_string())?;
        for p in relative(path)? {
            fd = fs::openat(&fd, p, FLAGS, Mode::empty())
                .map_err(|e| format!("unsafe runtime directory: {e}"))?
                .into();
        }
        Ok(fd)
    }
    fn parent(&self, path: &str) -> Result<(File, String)> {
        let mut parts = relative(path)?;
        let name = parts.pop().unwrap().to_owned();
        let fd = if parts.is_empty() {
            self.fd.try_clone().map_err(|e| e.to_string())?
        } else {
            self.directory(&parts.join("/"))?
        };
        Ok((fd, name))
    }
    pub fn open_file(&self, path: &str, write: bool) -> Result<Option<File>> {
        let (fd, name) = self.parent(path)?;
        let flags = if write { OFlags::RDWR } else { OFlags::RDONLY };
        match fs::openat(
            &fd,
            name.as_str(),
            flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(file) => {
                let file: File = file.into();
                regular(&file)?;
                Ok(Some(file))
            }
            Err(rustix::io::Errno::NOENT) => Ok(None),
            Err(e) => Err(format!("unsafe runtime file {path}: {e}")),
        }
    }
    pub fn read(&self, path: &str, limit: usize, mode: u32) -> Result<Option<Known>> {
        let Some(mut file) = self.open_file(path, false)? else {
            return Ok(None);
        };
        let before = regular(&file)?;
        let stamp = Stamp::of(&before);
        if stamp.mode != mode || before.len() > limit as u64 {
            return Err(format!("runtime mode/size mismatch: {path}"));
        }
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > limit || Stamp::of(&regular(&file)?) != stamp {
            return Err(format!("runtime file changed while reading: {path}"));
        }
        Ok(Some(Known { bytes, stamp }))
    }
    pub fn names(&self, path: &str) -> Result<Vec<String>> {
        let fd = self.directory(path)?;
        let mut names = Vec::new();
        let entries = fs::Dir::read_from(&fd).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .to_str()
                .map_err(|_| "non-UTF8 runtime entry")?;
            if name != "." && name != ".." {
                names.push(name.into());
            }
        }
        names.sort();
        Ok(names)
    }
    fn matches(&self, path: &str, expected: Option<&Known>) -> Result<bool> {
        match (self.open_file(path, false)?, expected) {
            (None, None) => Ok(true),
            (Some(file), Some(old)) => Ok(Stamp::of(&regular(&file)?) == old.stamp
                && file_digest(&file)? == digest(&old.bytes)),
            _ => Ok(false),
        }
    }
    pub fn replace(
        &self,
        path: &str,
        temp: &str,
        bytes: &[u8],
        expected: Option<&Known>,
    ) -> Result<()> {
        self.replace_checked(path, temp, bytes, expected, || Ok(()))
    }
    pub fn replace_checked(
        &self,
        path: &str,
        temp: &str,
        bytes: &[u8],
        expected: Option<&Known>,
        check: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let (parent, name) = self.parent(path)?;
        let (temp_parent, temp_name) = self.parent(temp)?;
        let p = parent.metadata().map_err(|e| e.to_string())?;
        let t = temp_parent.metadata().map_err(|e| e.to_string())?;
        if (p.dev(), p.ino()) != (t.dev(), t.ino()) || !self.matches(path, expected)? {
            return Err(format!("changed or occupied runtime destination: {path}"));
        }
        let file: File = fs::openat(
            &temp_parent,
            temp_name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|e| format!("runtime staging collision: {e}"))?
        .into();
        let created = Stamp::of(&regular(&file)?);
        let result = (|| {
            let mut output = &file;
            output.write_all(bytes).map_err(|e| e.to_string())?;
            fs::fchmod(&file, Mode::RUSR | Mode::WUSR).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            if !self.matches(path, expected)? {
                return Err(format!("runtime destination changed: {path}"));
            }
            check()?;
            if expected.is_none() {
                fs::renameat_with(
                    &temp_parent,
                    temp_name.as_str(),
                    &parent,
                    name.as_str(),
                    RenameFlags::NOREPLACE,
                )
            } else {
                fs::renameat(&temp_parent, temp_name.as_str(), &parent, name.as_str())
            }
            .map_err(|e| e.to_string())?;
            parent.sync_all().map_err(|e| e.to_string())
        })();
        // Remove only this still-open staging inode; never an externally replaced path.
        if let Ok(Some(staged)) = self.open_file(temp, false)
            && let Ok(st) = staged.metadata()
            && st.ino() == created.ino
            && st.dev() == created.dev
        {
            let _ = fs::unlinkat(&temp_parent, temp_name.as_str(), AtFlags::empty());
        }
        result
    }
    pub fn remove(&self, path: &str, expected: &Known) -> Result<()> {
        if !self.matches(path, Some(expected))? {
            return Err(format!("runtime cleanup fingerprint changed: {path}"));
        }
        let (parent, name) = self.parent(path)?;
        fs::unlinkat(&parent, name.as_str(), AtFlags::empty()).map_err(|e| e.to_string())?;
        parent.sync_all().map_err(|e| e.to_string())
    }
}

pub fn lock(file: &File, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(e) => return Err(format!("runtime lock unavailable: {e}")),
        }
    }
}
