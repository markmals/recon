use proptest::prelude::*;
use recon::request::parse_arguments;

fn accepts(words: &[&str]) -> bool {
    parse_arguments(&words.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>()).is_ok()
}

#[test]
fn proposal_0004_running_is_a_single_boolean_list_option() {
    assert!(accepts(&["app", "list", "--running"]));
    for words in [
        vec!["app", "list", "--running=yes"],
        vec!["app", "list", "--running", "--running"],
        vec!["app", "list", "--running", "yes"],
        vec!["app", "list", "--running", "Notes"],
        vec!["app", "inspect", "Notes", "--running"],
        vec!["app", "launch", "Notes", "--running"],
        vec!["ui", "tree", "Notes", "--running"],
    ] {
        assert!(!accepts(&words), "{words:?}");
    }
}

#[test]
fn proposal_0004_tree_options_combine_before_or_after_the_target() {
    for words in [
        vec!["ui", "tree", "Notes", "--depth", "0"],
        vec!["ui", "tree", "--depth", "64", "Notes"],
        vec!["ui", "tree", "Notes", "--no-menu-bar"],
        vec!["ui", "tree", "--no-menu-bar", "Notes", "--depth", "1"],
        vec!["ui", "tree", "--depth", "1", "--pid", "12", "Notes", "--no-menu-bar"],
        vec!["ui", "tree", "Notes", "--window", "9", "--depth", "3"],
    ] {
        assert!(accepts(&words), "{words:?}");
    }
}

#[test]
fn proposal_0004_rejects_malformed_or_inapplicable_narrowing() {
    for words in [
        vec!["ui", "tree", "Notes", "--depth"],
        vec!["ui", "tree", "Notes", "--depth", "1", "--depth", "2"],
        vec!["ui", "tree", "Notes", "--no-menu-bar", "--no-menu-bar"],
        vec!["ui", "tree", "Notes", "--no-menu-bar=yes"],
        vec!["ui", "tree", "Notes", "--window", "0", "--no-menu-bar"],
        vec!["ui", "tree", "--no-menu-bar", "Notes", "--window", "9"],
        vec!["ui", "windows", "Notes", "--depth", "1"],
        vec!["ui", "windows", "Notes", "--no-menu-bar"],
        vec!["ui", "screenshot", "Notes", "--out", "x.png", "--depth", "1"],
        vec!["ui", "screenshot", "Notes", "--out", "x.png", "--no-menu-bar"],
        vec!["app", "list", "--depth", "1"],
        vec!["app", "list", "--no-menu-bar"],
        vec!["ui", "tree", "Notes", "--compact"],
    ] {
        assert!(!accepts(&words), "{words:?}");
    }
    for depth in ["", "65", "4294967296", "-1", "+1", " 1", "1 ", "1.0", "0x1", "１", "١", "1x"] {
        assert!(!accepts(&["ui", "tree", "Notes", "--depth", depth]), "{depth:?}");
    }
}

proptest! {
    #[test]
    fn proposal_0004_all_supported_depths_are_accepted(depth in 0usize..=64, zeros in 0usize..8) {
        let depth = format!("{}{depth}", "0".repeat(zeros));
        prop_assert!(accepts(&["ui", "tree", "Notes", "--depth", &depth]));
    }

    #[test]
    fn proposal_0004_depths_beyond_the_safety_cap_are_rejected(depth in 65u64..=u64::MAX) {
        prop_assert!(!accepts(&["ui", "tree", "Notes", "--depth", &depth.to_string()]));
    }
}
