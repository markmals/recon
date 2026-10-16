use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Resolves `--out` to an absolute path Recon may create, without creating it.
pub fn prepare_output(out: &Path, current_dir: &Path) -> Result<PathBuf, String> {
    let path = if out.is_absolute() { out.to_path_buf() } else { current_dir.join(out) };
    let Some(parent) = path.parent() else {
        return Err(format!("cannot write {}: it names no file", path.display()));
    };
    if !parent.is_dir() {
        return Err(format!(
            "cannot write {}: {} is not an existing directory",
            path.display(),
            parent.display()
        ));
    }
    if path.exists() {
        return Err(already_exists(&path));
    }
    Ok(path)
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = File::create_new(path).map_err(|error| match error.kind() {
        ErrorKind::AlreadyExists => already_exists(path),
        _ => cannot_write(path, &error),
    })?;
    file.write_all(bytes).map_err(|error| cannot_write(path, &error))
}

fn already_exists(path: &Path) -> String {
    format!("cannot write {}: the file already exists; recon never overwrites", path.display())
}

fn cannot_write(path: &Path, error: &std::io::Error) -> String {
    format!("cannot write {}: {error}", path.display())
}
