use std::fs::{self, File};
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
    let file = File::create_new(path).map_err(|error| match error.kind() {
        ErrorKind::AlreadyExists => already_exists(path),
        _ => cannot_write(path, &error),
    })?;
    write_or_remove(path, file, bytes)
}

/// A partly written file is not a result, so a failed write leaves nothing behind.
fn write_or_remove(path: &Path, mut sink: impl Write, bytes: &[u8]) -> Result<(), String> {
    match sink.write_all(bytes).and_then(|()| sink.flush()) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(path);
            Err(cannot_write(path, &error))
        }
    }
}

fn already_exists(path: &Path) -> String {
    format!("cannot write {}: the file already exists; recon never overwrites", path.display())
}

fn cannot_write(path: &Path, error: &std::io::Error) -> String {
    format!("cannot write {}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    /// A sink that cannot take any bytes, standing in for a full disk.
    struct Full;

    impl Write for Full {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(ErrorKind::StorageFull, "no space left on device"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_write_that_fails_leaves_no_file_behind() {
        let path =
            std::env::temp_dir().join(format!("recon-output-{}-partial.png", std::process::id()));
        File::create(&path).expect("create the file");

        let error = write_or_remove(&path, Full, b"a partial capture").unwrap_err();

        assert!(error.contains("no space left on device"), "{error}");
        assert!(!path.exists(), "{} was left behind", path.display());
    }
}
