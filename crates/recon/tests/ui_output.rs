use std::fs;
use std::path::Path;

use recon::error::{ErrorCode, Failure};
use recon::ui::output::{prepare_output, write_new};
use serde_json::{Value, json};
use tempfile::TempDir;

mod support;

use support::validate_failure;

fn scratch() -> TempDir {
    tempfile::tempdir().expect("create the scratch directory")
}

fn details(failure: &Failure) -> Value {
    validate_failure(&failure.report(), &["ui", "screenshot"]);
    failure.details.clone().expect("the data needed to recover")
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

    let failure = prepare_output(&missing, directory).unwrap_err();

    assert_eq!(failure.code, ErrorCode::IoFailed);
    assert!(!failure.retryable);
    let reported = details(&failure);
    assert_eq!(reported["path"], missing.to_str().unwrap());
    assert_eq!(reported["operation"], "resolve-output");
    assert_eq!(reported["reason"], "parent-not-a-directory");
    assert_eq!(reported["partialFile"], Value::Null);
    let detail = reported["detail"].as_str().expect("what macOS could not do");
    assert!(detail.contains(&directory.join("nested").display().to_string()), "{detail}");

    let file = directory.join("file");
    fs::write(&file, b"x").unwrap();
    let failure = prepare_output(&file.join("shot.png"), directory).unwrap_err();
    assert_eq!(details(&failure)["reason"], "parent-not-a-directory");
}

#[test]
fn an_output_path_that_names_no_file_is_refused() {
    let failure = prepare_output(Path::new("/"), Path::new("/")).unwrap_err();

    assert_eq!(failure.code, ErrorCode::IoFailed);
    let reported = details(&failure);
    assert_eq!(reported["path"], "/");
    assert_eq!(reported["operation"], "resolve-output");
    assert_eq!(reported["reason"], "names-no-file");
}

#[test]
fn an_existing_output_file_is_never_overwritten() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    fs::write(&target, b"old").unwrap();

    let failure = prepare_output(&target, directory).unwrap_err();

    assert_eq!(failure.code, ErrorCode::AlreadyExists);
    assert!(!failure.retryable);
    assert_eq!(details(&failure), json!({ "path": target.to_str().unwrap() }));
    assert_eq!(fs::read(&target).unwrap(), b"old");
}

#[test]
fn an_existing_directory_at_the_output_path_is_refused() {
    let scratch = scratch();
    let directory = scratch.path();
    let target = directory.join("shot.png");
    fs::create_dir(&target).unwrap();
    let failure = prepare_output(&target, directory).unwrap_err();
    validate_failure(&failure.report(), &["ui", "screenshot"]);
    assert_eq!(failure.code, ErrorCode::AlreadyExists);
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

    let failure = write_new(&target, b"new").unwrap_err();

    assert_eq!(failure.code, ErrorCode::AlreadyExists);
    assert_eq!(details(&failure), json!({ "path": target.to_str().unwrap() }));
    assert_eq!(fs::read(&target).unwrap(), b"old");
}
