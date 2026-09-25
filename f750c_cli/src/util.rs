use std::path::{Path, PathBuf};

pub fn get_relative_path(base: &Path, path: &Path) -> Option<PathBuf> {
    let base = base.canonicalize().ok()?;
    let path = path.canonicalize().ok()?;

    path.strip_prefix(base).ok().map(|p| p.to_path_buf())
}