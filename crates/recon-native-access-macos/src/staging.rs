//! The research helper staged where sandboxed targets can read it.
//!
//! An App Sandboxed target loads the helper only from a path its sandbox reads, and the packaged
//! helper beside `recon` is not one. `prepare-host` has root create a directory App Sandbox reads,
//! owned by the invoking user, and Recon copies the helper into it itself, so no root process ever
//! opens a path the user controls. Research sessions use the copy only while it is byte for byte
//! the packaged helper, so an upgraded Recon never loads a helper an earlier one staged.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::access::{CommandRunner, Fact};
use crate::artifact::{Artifacts, HELPER_FILE};

pub(crate) const STAGED_DIRECTORY: &str = "/Library/Application Support/Recon";

/// The helper staged in `directory`, when it is a regular file identical to the packaged helper.
pub(crate) fn staged_helper(packaged: &Artifacts, directory: &Path) -> Option<PathBuf> {
    let staged = directory.join(HELPER_FILE);
    if !staged.symlink_metadata().ok()?.is_file() {
        return None;
    }
    (fs::read(&staged).ok()? == fs::read(&packaged.helper).ok()?).then_some(staged)
}

pub(crate) fn staged_fact(packaged: Option<&Artifacts>, directory: &Path) -> Fact<bool> {
    let staged = directory.join(HELPER_FILE);
    match packaged {
        Some(packaged) => Fact::known(
            staged_helper(packaged, directory).is_some(),
            &format!("{} compared with {}", staged.display(), packaged.helper.display()),
        ),
        None => Fact::unknown(
            &staged.display().to_string(),
            "no packaged helper to compare the staged one with",
        ),
    }
}

/// Stage the packaged helper in `directory`. Root only creates the directory, owned by the invoking
/// user. The copy is written beside the staged helper and renamed over it, so a target that has the
/// old helper mapped keeps it intact.
pub(crate) fn install<R: CommandRunner>(
    runner: &mut R,
    packaged: &Artifacts,
    directory: &Path,
) -> Result<PathBuf, String> {
    // SAFETY: geteuid and getegid read the caller's identity without pointer arguments.
    let (user, group) = unsafe { (libc::geteuid(), libc::getegid()) };
    let (owner, owner_group) = (user.to_string(), group.to_string());
    as_root(
        runner,
        &["/usr/bin/install", "-d", "-m", "0755", "-o", &owner, "-g", &owner_group],
        &[directory],
    )?;
    let metadata = directory
        .symlink_metadata()
        .map_err(|error| format!("{} could not be inspected: {error}", directory.display()))?;
    if !metadata.is_dir() || metadata.uid() != user || metadata.mode() & 0o022 != 0 {
        return Err(format!("{} is not a directory only you can write", directory.display()));
    }
    let staged = directory.join(HELPER_FILE);
    let partial = directory.join(format!(".{HELPER_FILE}.{}", std::process::id()));
    copy_to_new(&packaged.helper, &partial).and_then(|()| fs::rename(&partial, &staged)).map_err(
        |error| {
            let _ = fs::remove_file(&partial);
            format!("the helper could not be copied to {}: {error}", staged.display())
        },
    )?;
    Ok(staged)
}

/// Remove the staged helper, and the directory once nothing else is in it. Answers whether anything
/// was removed; with nothing staged, sudo is never asked.
pub(crate) fn remove<R: CommandRunner>(runner: &mut R, directory: &Path) -> Result<bool, String> {
    let staged = directory.join(HELPER_FILE);
    match fs::remove_file(&staged) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("{} could not be removed: {error}", staged.display())),
    }
    if fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_none()) {
        as_root(runner, &["/bin/rmdir"], &[directory])?;
    }
    Ok(true)
}

fn copy_to_new(source: &Path, destination: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let mut output =
        OpenOptions::new().write(true).create_new(true).mode(0o755).open(destination)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()
}

fn as_root<R: CommandRunner>(
    runner: &mut R,
    command: &[&str],
    paths: &[&Path],
) -> Result<(), String> {
    let paths: Vec<String> = paths.iter().map(|path| path.display().to_string()).collect();
    let mut args: Vec<&str> = command.to_vec();
    args.extend(paths.iter().map(String::as_str));
    let result = runner.run("/usr/bin/sudo", &args)?;
    if result.success {
        Ok(())
    } else {
        Err(format!("`sudo {}` failed: {}", args.join(" "), result.stderr.trim()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;
    use crate::access::CommandResult;
    use crate::artifact::{BUILD_FILE, HELPER_FILE};

    /// Stands in for sudo by applying each command's effect to the test's own files, ignoring the
    /// ownership it asks for.
    #[derive(Default)]
    struct Root {
        commands: Vec<Vec<String>>,
        refuse: bool,
    }

    impl CommandRunner for Root {
        fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
            assert_eq!(program, "/usr/bin/sudo", "staging acts as root only through sudo");
            self.commands.push(args.iter().map(|arg| (*arg).to_owned()).collect());
            if self.refuse {
                return Ok(CommandResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "sudo: a password is required\n".into(),
                });
            }
            let last = Path::new(args[args.len() - 1]);
            match args[..2] {
                ["/usr/bin/install", "-d"] => fs::create_dir_all(last).unwrap(),
                ["/bin/rmdir", _] => fs::remove_dir(last).unwrap(),
                _ => panic!("unexpected root command {args:?}"),
            }
            Ok(CommandResult { success: true, stdout: String::new(), stderr: String::new() })
        }
    }

    /// A packaged helper, its build file and loader, as Recon ships them next to its executable.
    fn packaged(root: &Path, contents: &str) -> Artifacts {
        let directory = root.join("packaged");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(HELPER_FILE), contents).unwrap();
        fs::write(directory.join(BUILD_FILE), "build\n").unwrap();
        Artifacts {
            helper: directory.join(HELPER_FILE),
            helper_build: "build".to_owned(),
            loader: directory.join("recon-loader"),
        }
    }

    fn stage(directory: &Path, contents: &str) {
        fs::create_dir_all(directory).unwrap();
        fs::write(directory.join(HELPER_FILE), contents).unwrap();
    }

    #[test]
    fn a_staged_copy_of_the_packaged_helper_is_used() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        stage(&staged, "helper new");

        assert_eq!(staged_helper(&packaged, &staged), Some(staged.join(HELPER_FILE)));
        assert_eq!(staged_fact(Some(&packaged), &staged).value, Some(true));
    }

    #[test]
    fn a_staged_helper_that_differs_from_the_packaged_one_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        stage(&staged, "helper old");

        assert_eq!(staged_helper(&packaged, &staged), None);
        assert_eq!(staged_fact(Some(&packaged), &staged).value, Some(false));
    }

    #[test]
    fn a_staged_link_to_the_packaged_helper_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        symlink(&packaged.helper, staged.join(HELPER_FILE)).unwrap();

        assert_eq!(staged_helper(&packaged, &staged), None);
    }

    #[test]
    fn nothing_staged_is_reported_as_not_staged() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");

        let fact = staged_fact(Some(&packaged), &root.path().join("staged"));

        assert_eq!(fact.value, Some(false));
        assert_eq!(staged_fact(None, &root.path().join("staged")).value, None);
    }

    #[test]
    fn installing_asks_root_only_for_a_directory_the_user_owns() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        let mut sudo = Root::default();

        let helper = install(&mut sudo, &packaged, &staged).expect("install");

        assert_eq!(staged_helper(&packaged, &staged), Some(helper));
        // SAFETY: geteuid and getegid read the caller's identity without pointer arguments.
        let (user, group) = unsafe { (libc::geteuid(), libc::getegid()) };
        let directory: Vec<String> = [
            "/usr/bin/install".to_owned(),
            "-d".into(),
            "-m".into(),
            "0755".into(),
            "-o".into(),
            user.to_string(),
            "-g".into(),
            group.to_string(),
            staged.display().to_string(),
        ]
        .into();
        assert_eq!(sudo.commands, [directory], "root never reads the packaged helper");
    }

    #[test]
    fn installing_replaces_a_helper_an_earlier_recon_staged() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        stage(&staged, "helper old");

        install(&mut Root::default(), &packaged, &staged).expect("install");

        assert_eq!(fs::read_to_string(staged.join(HELPER_FILE)).unwrap(), "helper new");
        assert_eq!(fs::read_dir(&staged).unwrap().count(), 1, "no temporary copy is left behind");
    }

    #[test]
    fn installing_refuses_a_directory_others_can_write() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let staged = root.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o777)).unwrap();

        let failure = install(&mut Root::default(), &packaged, &staged)
            .expect_err("a shared directory is not a place to stage the helper");

        assert!(failure.contains(&staged.display().to_string()), "{failure}");
        assert!(!staged.join(HELPER_FILE).exists());
    }

    #[test]
    fn a_refused_install_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "helper new");
        let mut sudo = Root { refuse: true, ..Default::default() };

        let failure = install(&mut sudo, &packaged, &root.path().join("staged"))
            .expect_err("a refused sudo stages nothing");

        assert!(failure.contains("password is required"), "{failure}");
    }

    #[test]
    fn removing_deletes_the_staged_helper_and_its_directory() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        stage(&staged, "helper new");

        assert!(remove(&mut Root::default(), &staged).expect("remove"));

        assert!(!staged.exists());
    }

    #[test]
    fn removing_keeps_a_directory_holding_anything_else() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        stage(&staged, "helper new");
        fs::write(staged.join("operator-notes.txt"), "mine").unwrap();
        let mut sudo = Root::default();

        assert!(remove(&mut sudo, &staged).expect("remove"));

        assert!(staged.join("operator-notes.txt").exists());
        assert!(!staged.join(HELPER_FILE).exists());
        assert!(sudo.commands.is_empty());
    }

    #[test]
    fn removing_nothing_never_asks_for_sudo() {
        let root = tempfile::tempdir().unwrap();
        let mut sudo = Root::default();

        assert!(!remove(&mut sudo, &root.path().join("staged")).expect("remove"));

        assert!(sudo.commands.is_empty());
    }
}
