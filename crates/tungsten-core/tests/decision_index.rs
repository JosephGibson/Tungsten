use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn extract_decision_ids(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut ids = BTreeSet::new();

    for index in 0..bytes.len().saturating_sub(4) {
        if bytes[index] != b'D' || bytes[index + 1] != b'-' {
            continue;
        }

        let digits = &bytes[index + 2..index + 5];
        if digits.iter().all(|byte| byte.is_ascii_digit()) {
            ids.insert(text[index..index + 5].to_string());
        }
    }

    ids
}

fn decision_heading_ids(decisions: &str) -> BTreeSet<String> {
    decisions
        .lines()
        .filter(|line| line.starts_with("## D-"))
        .flat_map(extract_decision_ids)
        .collect()
}

#[test]
fn decision_index_mentions_every_decision_heading() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let decisions =
        fs::read_to_string(repo_root.join("DECISIONS.md")).expect("failed to read DECISIONS.md");
    let index = fs::read_to_string(repo_root.join("docs/DECISION_INDEX.md"))
        .expect("failed to read docs/DECISION_INDEX.md");

    let decision_ids = decision_heading_ids(&decisions);
    let index_ids = extract_decision_ids(&index);
    let missing: Vec<_> = decision_ids.difference(&index_ids).cloned().collect();

    assert!(
        missing.is_empty(),
        "docs/DECISION_INDEX.md is missing decision IDs: {}",
        missing.join(", ")
    );
}

#[test]
fn decision_index_does_not_reference_unknown_ids() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let decisions =
        fs::read_to_string(repo_root.join("DECISIONS.md")).expect("failed to read DECISIONS.md");
    let index = fs::read_to_string(repo_root.join("docs/DECISION_INDEX.md"))
        .expect("failed to read docs/DECISION_INDEX.md");

    let decision_ids = decision_heading_ids(&decisions);
    let index_ids = extract_decision_ids(&index);
    let unknown: Vec<_> = index_ids.difference(&decision_ids).cloned().collect();

    assert!(
        unknown.is_empty(),
        "docs/DECISION_INDEX.md references unknown decision IDs: {}",
        unknown.join(", ")
    );
}
