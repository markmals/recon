use std::path::PathBuf;

use proptest::prelude::*;
use recon::application::{Application, TargetError, normalize_applications, resolve_target};

fn application(name: &str, bundle_id: Option<&str>, path: &str) -> Application {
    Application {
        name: name.into(),
        bundle_id: bundle_id.map(str::to_owned),
        path: PathBuf::from(path),
        version: None,
        build: None,
        executable_path: None,
        running_pids: Vec::new(),
    }
}

#[test]
fn path_selects_the_intended_copy_when_names_and_identifiers_repeat() {
    let apps = vec![
        application("Logic Pro", Some("com.apple.logic10"), "/Applications/Logic Pro.app"),
        application("Logic Pro", Some("com.apple.logic10"), "/Users/test/Logic Pro.app"),
    ];

    let selected = resolve_target(&apps, "/Users/test/Logic Pro.app").unwrap();
    assert_eq!(selected.path, PathBuf::from("/Users/test/Logic Pro.app"));
    assert!(matches!(resolve_target(&apps, "Logic Pro"), Err(TargetError::Ambiguous(_))));
    assert!(matches!(resolve_target(&apps, "COM.APPLE.LOGIC10"), Err(TargetError::Ambiguous(_))));
}

#[test]
fn a_name_colliding_with_another_app_identifier_is_ambiguous() {
    let apps = vec![
        application("com.example.editor", Some("com.example.viewer"), "/A.app"),
        application("Viewer", Some("com.example.editor"), "/B.app"),
    ];
    assert!(matches!(resolve_target(&apps, "com.example.editor"), Err(TargetError::Ambiguous(_))));
}

#[test]
fn names_require_an_exact_match_not_a_substring() {
    let apps = vec![application("Logic Pro", None, "/Applications/Logic Pro.app")];
    assert!(matches!(resolve_target(&apps, "Logic"), Err(TargetError::NotFound)));
}

#[test]
fn running_processes_belong_to_the_exact_bundle_location() {
    let mut system = application("Editor", Some("com.example.editor"), "/Applications/Editor.app");
    system.running_pids.push(91);
    let mut copy = application("Editor", Some("com.example.editor"), "/Users/test/Editor.app");
    copy.running_pids.extend([75, 12, 75]);
    let apps = normalize_applications(vec![copy, system.clone(), system.clone()]);

    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].running_pids, vec![91]);
    assert_eq!(apps[1].running_pids, vec![12, 75]);
    assert_eq!(apps[0].path, system.path);
}

proptest! {
    #[test]
    fn duplicate_names_are_ambiguous_in_any_observation_order(count in 2usize..32) {
        let mut apps = (0..count)
            .map(|index| application("Editor", None, &format!("/{index}/Editor.app")))
            .collect::<Vec<_>>();
        prop_assert!(matches!(resolve_target(&apps, "Editor"), Err(TargetError::Ambiguous(_))));
        apps.reverse();
        prop_assert!(matches!(resolve_target(&apps, "Editor"), Err(TargetError::Ambiguous(_))));
    }
}
