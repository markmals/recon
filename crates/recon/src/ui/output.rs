//! Writing a capture to `--out`: never over an existing file, and removing a partial one.

use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::{ErrorCode, Failure};

/// Resolves `--out` to an absolute path Recon may create, without creating it.
///
/// # Errors
///
/// [`ErrorCode::IoFailed`] when the path names no file or its parent directory does not exist,
/// and [`ErrorCode::AlreadyExists`] when the file already does.
pub fn prepare_output(out: &Path, current_dir: &Path) -> Result<PathBuf, Failure> {
    let path = if out.is_absolute() { out.to_path_buf() } else { current_dir.join(out) };
    let Some(parent) = path.parent() else {
        return Err(unresolved_output(&path, "names-no-file", "it names no file".to_owned()));
    };
    if !parent.is_dir() {
        let detail = format!("{} is not an existing directory", parent.display());
        return Err(unresolved_output(&path, "parent-not-a-directory", detail));
    }
    if path.exists() {
        return Err(already_exists(&path));
    }
    Ok(path)
}

/// Writes `bytes` to a file created at `path`.
///
/// # Errors
///
/// [`ErrorCode::AlreadyExists`] when the file already exists, which Recon never overwrites, and
/// [`ErrorCode::IoFailed`] when writing fails. A failed write removes the partial file, and the
/// details name the file when macOS refuses to remove it.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let file = File::create_new(path).map_err(|error| match error.kind() {
        ErrorKind::AlreadyExists => already_exists(path),
        _ => write_failure(path, &error, None),
    })?;
    write_or_remove(path, file, bytes)
}

/// A partly written file is not a result, so a failed write leaves nothing behind.
fn write_or_remove(path: &Path, mut sink: impl Write, bytes: &[u8]) -> Result<(), Failure> {
    let Err(error) = sink.write_all(bytes).and_then(|()| sink.flush()) else {
        return Ok(());
    };
    // A file that is already gone was never left behind, so only a real refusal is reported.
    let refused =
        fs::remove_file(path).err().filter(|removal| removal.kind() != ErrorKind::NotFound);
    Err(write_failure(path, &error, refused.as_ref()))
}

/// Names the partial file only when macOS refused to remove it, because only then does it remain.
fn write_failure(path: &Path, error: &std::io::Error, refused: Option<&std::io::Error>) -> Failure {
    let cannot_write = format!("cannot write {}: {error}", path.display());
    let message = match refused {
        None => cannot_write,
        Some(refused) => {
            format!("{cannot_write}; the partial file could not be removed: {refused}")
        }
    };
    Failure::new(
        ErrorCode::IoFailed,
        message,
        json!({
            "path": path.display().to_string(),
            "operation": "write",
            // The operating system's own text is the whole explanation of a failed write.
            "reason": null,
            "detail": error.to_string(),
            "partialFile": refused.map(|_| path.display().to_string()),
        }),
    )
}

fn unresolved_output(path: &Path, reason: &str, detail: String) -> Failure {
    Failure::new(
        ErrorCode::IoFailed,
        format!("cannot write {}: {detail}", path.display()),
        json!({
            "path": path.display().to_string(),
            "operation": "resolve-output",
            "reason": reason,
            "detail": detail,
            "partialFile": null,
        }),
    )
}

fn already_exists(path: &Path) -> Failure {
    Failure::new(
        ErrorCode::AlreadyExists,
        format!("cannot write {}: the file already exists; recon never overwrites", path.display()),
        json!({ "path": path.display().to_string() }),
    )
}

#[cfg(test)]
mod tests {
    use std::io;

    use serde_json::Value;

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

    fn details(failure: &Failure) -> Value {
        failure.details.clone().expect("the data needed to recover")
    }

    #[test]
    fn a_write_that_fails_leaves_no_file_behind() {
        let path =
            std::env::temp_dir().join(format!("recon-output-{}-partial.png", std::process::id()));
        File::create(&path).expect("create the file");

        let failure = write_or_remove(&path, Full, b"a partial capture").unwrap_err();

        assert_eq!(failure.code, ErrorCode::IoFailed);
        let reported = details(&failure);
        assert_eq!(reported["operation"], "write");
        assert_eq!(reported["partialFile"], Value::Null);
        assert!(reported["detail"].as_str().expect("a detail").contains("no space left on device"));
        assert!(!path.exists(), "{} was left behind", path.display());
    }

    #[test]
    fn a_partial_file_that_could_not_be_removed_is_named_in_the_details() {
        let path = Path::new("/recon/capture.png");
        let write = io::Error::new(ErrorKind::StorageFull, "no space left on device");
        let refused = io::Error::new(ErrorKind::PermissionDenied, "operation not permitted");

        let failure = write_failure(path, &write, Some(&refused));

        assert_eq!(failure.code, ErrorCode::IoFailed);
        assert_eq!(details(&failure)["partialFile"], "/recon/capture.png");
        assert_eq!(details(&failure)["path"], "/recon/capture.png");
        assert_eq!(details(&write_failure(path, &write, None))["partialFile"], Value::Null);
    }
}
