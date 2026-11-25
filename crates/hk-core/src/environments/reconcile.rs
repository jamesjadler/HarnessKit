//! Pure reconciliation. Inaccessible observations are not evidence of removal.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub available: bool,
    pub excluded: bool,
    pub paused: bool,
    pub current: Option<String>,
    pub previous: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Decision {
    IdentityDecision,
    Synced,
    Conflict,
    RemovalDecision,
    Change {
        source: String,
        targets: Vec<String>,
    },
    Off,
}
pub fn reconcile(copies: &[Observation], ask_removal: bool, host: Option<&str>) -> Decision {
    let participating: Vec<_> = copies.iter().filter(|c| !c.excluded).collect();
    if participating.is_empty() {
        return Decision::Off;
    }
    let available: Vec<_> = participating
        .iter()
        .copied()
        .filter(|c| c.available && !c.paused)
        .collect();
    let changed: Vec<_> = available
        .iter()
        .copied()
        .filter(|c| c.current != c.previous)
        .collect();
    let changes: BTreeSet<_> = changed.iter().map(|c| &c.current).collect();
    if changes.len() > 1 {
        return Decision::Conflict;
    }
    if let Some(host) = host {
        if changed.iter().any(|c| c.id != host) {
            return Decision::Conflict;
        }
    }
    let source = if let Some(c) = changed.first() {
        Some(*c)
    } else {
        let versions: BTreeSet<_> = available
            .iter()
            .filter_map(|c| c.current.as_ref())
            .collect();
        if versions.len() > 1 {
            return Decision::Conflict;
        }
        available.iter().copied().find(|c| c.current.is_some())
    };
    let Some(source) = source else {
        return Decision::Synced;
    };
    if source.current.is_none() && ask_removal {
        return Decision::RemovalDecision;
    }
    let targets = participating
        .iter()
        .filter(|c| c.id != source.id && !c.paused && c.current != source.current)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    if targets.is_empty() {
        Decision::Synced
    } else {
        Decision::Change {
            source: source.id.clone(),
            targets,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn copy(id: &str, current: Option<&str>, previous: Option<&str>) -> Observation {
        Observation {
            id: id.into(),
            current: current.map(Into::into),
            previous: previous.map(Into::into),
            available: true,
            excluded: false,
            paused: false,
        }
    }
    #[test]
    fn independent_edits_in_three_environments_conflict() {
        assert_eq!(
            reconcile(
                &[
                    copy("a", Some("8"), Some("7")),
                    copy("b", Some("9"), Some("7")),
                    copy("c", Some("7"), Some("7"))
                ],
                false,
                None
            ),
            Decision::Conflict
        );
    }
    #[test]
    fn offline_is_not_deletion() {
        let mut b = copy("b", None, Some("7"));
        b.available = false;
        assert_eq!(
            reconcile(&[copy("a", Some("7"), Some("7")), b], false, None),
            Decision::Change {
                source: "a".into(),
                targets: vec!["b".into()]
            }
        );
    }
    #[test]
    fn ignore_and_pause_exclude_writes() {
        let mut b = copy("b", Some("9"), Some("7"));
        b.excluded = true;
        let mut c = copy("c", Some("7"), Some("7"));
        c.paused = true;
        assert_eq!(
            reconcile(&[copy("a", Some("8"), Some("7")), b, c], false, None),
            Decision::Synced
        );
    }
    #[test]
    fn removal_against_edit_conflicts() {
        assert_eq!(
            reconcile(
                &[copy("a", None, Some("7")), copy("b", Some("8"), Some("7"))],
                false,
                None
            ),
            Decision::Conflict
        );
    }
    #[test]
    fn host_mode_does_not_overwrite_remote_edit() {
        assert_eq!(
            reconcile(
                &[
                    copy("a", Some("7"), Some("7")),
                    copy("b", Some("8"), Some("7"))
                ],
                false,
                Some("a")
            ),
            Decision::Conflict
        );
    }
    #[test]
    fn removal_prompt_holds_propagation() {
        assert_eq!(
            reconcile(
                &[copy("a", None, Some("7")), copy("b", Some("7"), Some("7"))],
                true,
                None
            ),
            Decision::RemovalDecision
        );
    }
}
