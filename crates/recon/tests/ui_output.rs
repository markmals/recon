use std::fs;
use std::path::Path;

use recon::ui::output::{prepare_output, write_new};
use tempfile::TempDir;

fn scratch() -> TempDir {
    tempfile::tempdir().expect("create the scratch directory")
}

#[test]
fn a_relative_output_path_is_resolved_against_the_working_directory() {
    let scratch = scratch();
    let directory = scratch.path();
    let prepared = prepare_output(Path::new("shot.png"), directory).unwrap();
    assert_eq!(prepared, directory.join("shot.png"));
    assert!(prepared.is_absolute());
}

#[test]
fn an_absolute_output_path_is_kept() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    assert_eq!(prepare_output(&target, Path::new("/")).unwrap(), target);
}

#[test]
fn a_parent_that_is_not_an_existing_directory_is_refused() {
    let scratch = scratch();
    let directory = scratch.path();
    let missing = directory.join("nested").join("shot.png");
    let error = prepare_output(&missing, directory).unwrap_err();
    assert!(error.contains(&directory.join("nested").display().to_string()), "{error}");
    assert!(error.contains("is not an existing directory"), "{error}");

    let file = directory.join("file");
    fs::write(&file, b"x").unwrap();
    let under_file = file.join("shot.png");
    let error = prepare_output(&under_file, directory).unwrap_err();
    assert!(error.contains("is not an existing directory"), "{error}");
}

#[test]
fn an_existing_output_file_is_never_overwritten() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    fs::write(&target, b"old").unwrap();

    let error = prepare_output(&target, directory).unwrap_err();
    let expected = "the file already exists; recon never overwrites";
    assert_eq!(error, format!("cannot write {}: {expected}", target.display()));
    assert_eq!(fs::read(&target).unwrap(), b"old");
}

#[test]
fn an_existing_directory_at_the_output_path_is_refused() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    fs::create_dir(&target).unwrap();
    assert!(prepare_output(&target, directory).is_err());
}

#[test]
fn write_new_creates_the_file_with_its_bytes() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");

    write_new(&target, b"\x89PNG").unwrap();

    assert_eq!(fs::read(&target).unwrap(), b"\x89PNG");
}

#[test]
fn write_new_refuses_to_replace_an_existing_file() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    fs::write(&target, b"old").unwrap();

    let error = write_new(&target, b"new").unwrap_err();

    assert!(error.contains("recon never overwrites"), "{error}");
    assert_eq!(fs::read(&target).unwrap(), b"old");
}
