use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: PathBuf,
    pub version: Option<String>,
    pub build: Option<String>,
    pub executable_path: Option<PathBuf>,
    pub running_pids: Vec<u32>,
}

#[derive(Debug)]
pub enum TargetError<'a> {
    NotFound,
    Ambiguous(Vec<&'a Application>),
}

pub fn normalize_applications(mut discovered: Vec<Application>) -> Vec<Application> {
    discovered.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    discovered.dedup_by(|next, previous| {
        if next.path != previous.path {
            return false;
        }
        previous.running_pids.append(&mut next.running_pids);
        true
    });

    for application in &mut discovered {
        application.running_pids.sort_unstable();
        application.running_pids.dedup();
    }
    discovered.sort_unstable_by(|left, right| {
        left.name.cmp(&right.name).then(left.path.cmp(&right.path))
    });
    discovered
}

pub fn resolve_target<'a>(
    applications: &'a [Application],
    selector: &str,
) -> Result<&'a Application, TargetError<'a>> {
    let mut first = None;
    let mut ambiguous = Vec::new();
    for application in applications {
        let matches = if selector.starts_with('/') {
            application.path == Path::new(selector)
        } else {
            application.name == selector
                || application
                    .bundle_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(selector))
        };
        if matches {
            if let Some(previous) = first {
                if ambiguous.is_empty() {
                    ambiguous.push(previous);
                }
                ambiguous.push(application);
            } else {
                first = Some(application);
            }
        }
    }
    if !ambiguous.is_empty() {
        return Err(TargetError::Ambiguous(ambiguous));
    }
    first.ok_or(TargetError::NotFound)
}
