//! The research helper staged where sandboxed targets can read it.
//!
//! An App Sandboxed target loads the helper only from a path its sandbox reads, and the packaged
//! helper beside `recon` is not one. `prepare-host` installs a copy, through sudo, in a root-owned
//! directory App Sandbox reads. Research sessions use that copy only while it is the packaged
//! helper's build, so an upgraded Recon never loads a helper an earlier one staged.

use std::fs;
use std::path::{Path, PathBuf};

use crate::access::{CommandRunner, Fact};
use crate::artifact::{Artifacts, BUILD_FILE, HELPER_FILE};

pub(crate) const STAGED_DIRECTORY: &str = "/Library/Application Support/Recon";

/// The helper staged in `directory`, when it is the packaged helper's build.
pub(crate) fn staged_helper(packaged: &Artifacts, directory: &Path) -> Option<PathBuf> {
    let build = fs::read_to_string(directory.join(BUILD_FILE)).ok()?;
    let helper = directory.join(HELPER_FILE);
    (build.trim() == packaged.helper_build && helper.is_file()).then_some(helper)
}

pub(crate) fn staged_fact(packaged: Option<&Artifacts>, directory: &Path) -> Fact<bool> {
    let source = format!("{} build file", directory.display());
    match packaged {
        Some(packaged) => Fact::known(
            staged_helper(packaged, directory).is_some(),
            &format!("{source}, against packaged build {}", packaged.helper_build),
        ),
        None => Fact::unknown(&source, "no packaged helper to compare the staged one with"),
    }
}

/// Install the packaged helper and its build file in `directory`, owned by root. `install` always
/// replaces a file by renaming a complete copy over it, so a target that has the old helper mapped
/// keeps it intact.
pub(crate) fn install<R: CommandRunner>(
    runner: &mut R,
    packaged: &Artifacts,
    directory: &Path,
) -> Result<PathBuf, String> {
    let staged = directory.join(HELPER_FILE);
    let packaged_build = packaged.helper.with_file_name(BUILD_FILE);
    as_root(
        runner,
        &["/usr/bin/install", "-d", "-m", "0755", "-o", "root", "-g", "wheel"],
        &[directory],
    )?;
    // The helper goes first, so the build file never names a helper that is not yet in place.
    as_root(
        runner,
        &["/usr/bin/install", "-m", "0755", "-o", "root", "-g", "wheel"],
        &[&packaged.helper, &staged],
    )?;
    as_root(
        runner,
        &["/usr/bin/install", "-m", "0644", "-o", "root", "-g", "wheel"],
        &[&packaged_build, &directory.join(BUILD_FILE)],
    )?;
    Ok(staged)
}

/// Remove what `install` put in `directory`, and the directory once nothing else is in it. Answers
/// whether anything was removed; with nothing staged, sudo is never asked.
pub(crate) fn remove<R: CommandRunner>(runner: &mut R, directory: &Path) -> Result<bool, String> {
    let staged: Vec<PathBuf> = [BUILD_FILE, HELPER_FILE]
        .into_iter()
        .map(|name| directory.join(name))
        .filter(|path| path.symlink_metadata().is_ok())
        .collect();
    if staged.is_empty() {
        return Ok(false);
    }
    let staged: Vec<&Path> = staged.iter().map(PathBuf::as_path).collect();
    as_root(runner, &["/bin/rm", "-f"], &staged)?;
    if fs::read_dir(directory).is_ok_and(|mut entries| entries.next().is_none()) {
        as_root(runner, &["/bin/rmdir"], &[directory])?;
    }
    Ok(true)
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

    use super::*;
    use crate::access::CommandResult;
    use crate::artifact::{BUILD_FILE, HELPER_FILE};

    /// Stands in for sudo by applying each command's effect to the test's own files, ignoring the
    /// ownership and mode it asks for.
    #[derive(Default)]
    struct Root {
        commands: Vec<Vec<String>>,
        refuse: bool,
    }

    impl CommandRunner for Root {
        fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
            assert_eq!(program, "/usr/bin/sudo", "staging acts only through sudo");
            self.commands.push(args.iter().map(|arg| (*arg).to_owned()).collect());
            if self.refuse {
                return Ok(CommandResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "sudo: a password is required\n".into(),
                });
            }
            let last = Path::new(args[args.len() - 1]);
            match args[0] {
                "/usr/bin/install" if args[1] == "-d" => fs::create_dir_all(last).unwrap(),
                "/usr/bin/install" => {
                    fs::copy(args[args.len() - 2], last).unwrap();
                }
                "/bin/rm" => args[2..].iter().for_each(|path| fs::remove_file(path).unwrap()),
                "/bin/rmdir" => fs::remove_dir(last).unwrap(),
                other => panic!("unexpected staging command {other}"),
            }
            Ok(CommandResult { success: true, stdout: String::new(), stderr: String::new() })
        }
    }

    /// A packaged helper and its build file, as Recon ships them next to its executable.
    fn packaged(root: &Path, build: &str) -> Artifacts {
        let directory = root.join("packaged");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(HELPER_FILE), format!("helper {build}")).unwrap();
        fs::write(directory.join(BUILD_FILE), format!("{build}\n")).unwrap();
        Artifacts {
            helper: directory.join(HELPER_FILE),
            helper_build: build.to_owned(),
            loader: directory.join("recon-loader"),
        }
    }

    fn stage(directory: &Path, build: &str) {
        fs::create_dir_all(directory).unwrap();
        fs::write(directory.join(HELPER_FILE), format!("helper {build}")).unwrap();
        fs::write(directory.join(BUILD_FILE), format!("{build}\n")).unwrap();
    }

    #[test]
    fn a_staged_copy_of_the_packaged_build_is_used() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "new");
        let staged = root.path().join("staged");
        stage(&staged, "new");

        assert_eq!(staged_helper(&packaged, &staged), Some(staged.join(HELPER_FILE)));
        assert_eq!(staged_fact(Some(&packaged), &staged).value, Some(true));
    }

    #[test]
    fn a_staged_copy_from_another_build_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "new");
        let staged = root.path().join("staged");
        stage(&staged, "old");

        assert_eq!(staged_helper(&packaged, &staged), None);
        assert_eq!(staged_fact(Some(&packaged), &staged).value, Some(false));
    }

    #[test]
    fn nothing_staged_is_reported_as_not_staged() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "new");

        let fact = staged_fact(Some(&packaged), &root.path().join("staged"));

        assert_eq!(fact.value, Some(false));
        assert_eq!(staged_fact(None, &root.path().join("staged")).value, None);
    }

    #[test]
    fn installing_stages_the_packaged_build_through_sudo() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "new");
        let staged = root.path().join("staged");
        stage(&staged, "old");
        let mut sudo = Root::default();

        let helper = install(&mut sudo, &packaged, &staged).expect("install");

        assert_eq!(helper, staged.join(HELPER_FILE));
        assert_eq!(staged_helper(&packaged, &staged), Some(helper));
        // The helper is replaced before the build file that names it.
        let installed: Vec<&str> =
            sudo.commands.iter().map(|command| command.last().unwrap().as_str()).collect();
        let helper_target = staged.join(HELPER_FILE).display().to_string();
        let build_target = staged.join(BUILD_FILE).display().to_string();
        assert_eq!(installed, [staged.display().to_string(), helper_target, build_target]);
        assert!(
            sudo.commands
                .iter()
                .all(|command| command.windows(2).any(|pair| pair == ["-o", "root"]))
        );
    }

    #[test]
    fn a_refused_install_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let packaged = packaged(root.path(), "new");
        let mut sudo = Root { refuse: true, ..Default::default() };

        let failure = install(&mut sudo, &packaged, &root.path().join("staged"))
            .expect_err("a refused sudo stages nothing");

        assert!(failure.contains("password is required"), "{failure}");
    }

    #[test]
    fn removing_deletes_what_prepare_host_staged_and_its_directory() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        stage(&staged, "new");
        let mut sudo = Root::default();

        assert!(remove(&mut sudo, &staged).expect("remove"));

        assert!(!staged.exists());
    }

    #[test]
    fn removing_keeps_a_directory_holding_anything_else() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("staged");
        stage(&staged, "new");
        fs::write(staged.join("operator-notes.txt"), "mine").unwrap();
        let mut sudo = Root::default();

        assert!(remove(&mut sudo, &staged).expect("remove"));

        assert!(staged.join("operator-notes.txt").exists());
        assert!(!staged.join(HELPER_FILE).exists());
    }

    #[test]
    fn removing_nothing_never_asks_for_sudo() {
        let root = tempfile::tempdir().unwrap();
        let mut sudo = Root::default();

        assert!(!remove(&mut sudo, &root.path().join("staged")).expect("remove"));

        assert!(sudo.commands.is_empty());
    }
}
