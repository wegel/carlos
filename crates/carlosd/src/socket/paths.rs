//! Resolve private leaf directories through trusted ancestors before creating a daemon endpoint.

use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(super) fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub(super) fn private_directory(path: &Path) -> io::Result<PathBuf> {
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let metadata = path.symlink_metadata()?;
    let uid = unsafe { libc::geteuid() };
    if !metadata.file_type().is_dir()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket and state directories must be private, owned by this user and not symlinks",
        ));
    }
    let path = path.canonicalize()?;
    for ancestor in path.ancestors() {
        let metadata = ancestor.metadata()?;
        let mode = metadata.permissions().mode();
        let trusted_sticky_root = metadata.uid() == 0 && mode & 0o1000 != 0;
        if !matches!(metadata.uid(), 0) && metadata.uid() != uid
            || mode & 0o022 != 0 && !trusted_sticky_root
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "daemon path has an untrusted or writable ancestor",
            ));
        }
    }
    Ok(path)
}
