//! Opened directory handles anchor every association operation to one checked store.
use herdr_agent_kit::plugin_env::PluginEnv;
use rustix::{
    fs::{self, AtFlags, FileType, Mode, OFlags},
    io::Errno,
};
use std::{fs::File, io::Write};

pub(super) struct Directory {
    fd: File,
    uid: u32,
}
fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}
fn check_directory(fd: &File, uid: u32, private: bool) -> Result<(), Errno> {
    let st = fs::fstat(fd)?;
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory
        || (st.st_uid != uid && (private || st.st_uid != 0))
        || st.st_mode & if private { 0o077 } else { 0o022 } != 0
    {
        return Err(Errno::ACCESS);
    }
    Ok(())
}
impl Directory {
    pub fn open(env: &PluginEnv, create: bool) -> Result<Self, Errno> {
        let base = env.xdg_state_home();
        if create {
            std::fs::create_dir_all(&base).map_err(|_| Errno::ACCESS)?;
        }
        let mut fd: File = fs::open(&base, directory_flags(), Mode::empty())?.into();
        check_directory(&fd, env.uid(), false)?;
        for (name, private) in [("herdr-panorama", false), ("codex-bindings", true)] {
            if create {
                match fs::mkdirat(&fd, name, Mode::RWXU) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(e) => return Err(e),
                }
            }
            let next: File = fs::openat(&fd, name, directory_flags(), Mode::empty())?.into();
            check_directory(&next, env.uid(), private)?;
            fd = next;
        }
        Ok(Self { fd, uid: env.uid() })
    }
    pub fn file(&self, name: &str, write: bool) -> Result<File, Errno> {
        let access = if write {
            OFlags::WRONLY | OFlags::CREATE
        } else {
            OFlags::RDONLY
        };
        let file: File = fs::openat(
            &self.fd,
            name,
            access | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?
        .into();
        let st = fs::fstat(&file)?;
        if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile
            || st.st_uid != self.uid
            || st.st_mode & 0o077 != 0
            || st.st_nlink != 1
        {
            return Err(Errno::ACCESS);
        }
        Ok(file)
    }
    pub fn replace(&self, name: &str, bytes: &[u8]) -> Result<(), &'static str> {
        let pending = format!("{name}.pending");
        let result = (|| {
            let mut file = self
                .file(&pending, true)
                .map_err(|_| "cannot open private pending binding")?;
            file.set_len(0)
                .map_err(|_| "cannot truncate pending binding")?;
            file.write_all(bytes).map_err(|_| "cannot write binding")?;
            file.sync_all().map_err(|_| "cannot persist binding")?;
            fs::renameat(&self.fd, pending.as_str(), &self.fd, name)
                .map_err(|_| "cannot replace binding")
        })();
        if result.is_err() {
            let _ = fs::unlinkat(&self.fd, pending.as_str(), AtFlags::empty());
        }
        result
    }
}
