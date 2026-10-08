use super::*;
use crate::store::{Implementation, Link};

fn source(id: &str, path: &str) -> SpecSource {
    SpecSource {
        id: id.into(),
        path: path.into(),
        recursive: true,
        include: vec!["**/*.md".into()],
        exclude: Vec::new(),
    }
}

fn record(id: &str, path: &str, source_id: Option<&str>, relative: Option<&str>) -> Record {
    Record {
        schema_version: 1,
        ownership: "user".into(),
        source_id: source_id.map(str::to_string),
        source_relative_path: relative.map(PathBuf::from),
        content_fingerprint: None,
        id: id.into(),
        title: Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        repo: None,
        spec_path: path.into(),
        created_at: 1,
        updated_at: 1,
        spec: "done".into(),
        jira: Link {
            status: "ready".into(),
            key: None,
            url: None,
        },
        implementation: Implementation {
            status: "waiting".into(),
            agent: None,
            branch: None,
        },
        pr: Link {
            status: "waiting".into(),
            key: None,
            url: None,
        },
        launch: None,
        previous_launches: Vec::new(),
    }
}

#[test]
fn nested_domains_have_stepped_guides_folders_first_and_natural_filenames() {
    let records = vec![
        record(
            "ten",
            "/missing/context/designs/spec10.md",
            Some("context"),
            Some("designs/spec10.md"),
        ),
        record(
            "two",
            "/missing/context/designs/spec2.md",
            Some("context"),
            Some("designs/spec2.md"),
        ),
        record(
            "mission",
            "/missing/context/missions/zeller/spec.html",
            Some("context"),
            Some("missions/zeller/spec.html"),
        ),
        record(
            "root",
            "/missing/context/a.md",
            Some("context"),
            Some("a.md"),
        ),
    ];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[source("context", "/missing/context")],
        Path::new("/managed/specs"),
    );
    let rows: Vec<_> = tree
        .rows
        .iter()
        .map(|row| (row.label.as_str(), row.depth, row.prefix.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("context", 0, ""),
            ("designs", 1, "├─ "),
            ("spec2.md", 2, "│  ├─ "),
            ("spec10.md", 2, "│  └─ "),
            ("missions", 1, "├─ "),
            ("zeller", 2, "│  └─ "),
            ("spec.html", 3, "│     └─ "),
            ("a.md", 1, "└─ "),
        ]
    );
    assert_eq!(tree.selected_record(), Some(0));
    assert_eq!(tree.rows[1].path, Path::new("/missing/context/designs"));
    assert!(
        tree.rows
            .iter()
            .filter(|row| row.is_folder)
            .all(|row| row.expanded)
    );
}

#[test]
fn duplicate_roots_stay_distinct_and_show_parent_context() {
    let records = vec![
        record(
            "one",
            "/work/specs/design.md",
            Some("work"),
            Some("design.md"),
        ),
        record(
            "two",
            "/personal/specs/design.md",
            Some("personal"),
            Some("design.md"),
        ),
    ];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[
            source("work", "/work/specs"),
            source("personal", "/personal/specs"),
        ],
        Path::new("/managed/specs"),
    );
    let roots: Vec<_> = tree.rows.iter().filter(|row| row.depth == 0).collect();
    assert_eq!(roots.len(), 2);
    assert_ne!(roots[0].key, roots[1].key);
    assert!(roots[0].label.contains("personal"));
    assert!(roots[1].label.contains("work"));
    assert_eq!(
        tree.rows
            .iter()
            .filter(|row| row.label == "design.md")
            .count(),
        2
    );
}

#[test]
fn focus_uses_record_uuid_when_updates_reorder_metadata() {
    let mut records = vec![
        record("one", "/specs/a.md", Some("specs"), Some("a.md")),
        record("two", "/specs/b.md", Some("specs"), Some("b.md")),
    ];
    let sources = [source("specs", "/specs")];
    let mut tree = Tree::default();
    tree.rebuild(&records, &sources, Path::new("/managed/specs"));
    assert!(tree.focus_record(1));
    records.swap(0, 1);
    records[0].title = "Updated title".into();
    records[0].updated_at = 2;
    tree.rebuild(&records, &sources, Path::new("/managed/specs"));
    assert_eq!(tree.selected_record(), Some(0));
    assert_eq!(tree.rows[tree.focused].key, "record:two");
    assert_eq!(tree.rows[tree.focused].label, "Updated title");
}

#[test]
fn folding_navigation_and_focus_record_respect_boundaries() {
    let records = [
        record(
            "one",
            "/specs/domain/nested/a.md",
            Some("specs"),
            Some("domain/nested/a.md"),
        ),
        record("two", "/specs/b.md", Some("specs"), Some("b.md")),
    ];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[source("specs", "/specs")],
        Path::new("/managed/specs"),
    );
    tree.collapse_or_parent();
    assert_eq!(tree.rows[tree.focused].label, "nested");
    tree.collapse_or_parent();
    assert!(!tree.rows[tree.focused].expanded);
    assert_eq!(tree.rows.len(), 4);
    tree.collapse_or_parent();
    assert_eq!(tree.rows[tree.focused].label, "domain");
    tree.collapse_or_parent();
    assert_eq!(tree.rows.len(), 3);
    assert!(tree.focus_record(0));
    assert_eq!(tree.rows.len(), 5);
    tree.focused = 0;
    tree.collapse_or_parent();
    assert_eq!(tree.rows.len(), 1);
    tree.move_focus(false);
    tree.move_focus(true);
    assert_eq!(tree.focused, 0);
    tree.expand_or_child();
    assert_eq!(tree.rows.len(), 5);
    tree.expand_or_child();
    assert_eq!(tree.rows[tree.focused].label, "domain");
    tree.toggle_focused();
    let focused = tree.rows[tree.focused].key.clone();
    tree.rebuild(
        &records,
        &[source("specs", "/specs")],
        Path::new("/managed/specs"),
    );
    assert_eq!(tree.rows[tree.focused].key, focused);
    assert!(!tree.rows[tree.focused].expanded);
    assert!(!tree.focus_record(99));
}

#[test]
fn unassociated_records_match_nearest_source_without_existing_files() {
    let records = [record(
        "one",
        "/root/context/missions/zeller/a.md",
        None,
        None,
    )];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[
            source("outer", "/root/context"),
            source("inner", "/root/context/missions"),
        ],
        Path::new("/managed/specs"),
    );
    assert_eq!(tree.rows[0].key, "source:inner");
    assert_eq!(tree.rows[0].label, "missions");
    assert_eq!(tree.rows[1].label, "zeller");
}

#[test]
fn legacy_and_external_roots_never_expose_managed_uuid_directory_as_domain() {
    let mut managed = record("managed", "/app/items/specs/uuid.md", None, None);
    managed.ownership = "managed".into();
    let records = [
        managed,
        record("one", "/personal/specs/a.md", None, None),
        record("two", "/work/specs/a.md", None, None),
    ];
    let mut tree = Tree::default();
    tree.rebuild(&records, &[], Path::new("/app/items/specs"));
    let roots: Vec<_> = tree
        .rows
        .iter()
        .filter(|row| row.depth == 0)
        .map(|row| row.label.as_str())
        .collect();
    assert!(roots.contains(&"Inbox specs"));
    assert!(roots.contains(&"/personal/specs"));
    assert!(roots.contains(&"/work/specs"));
    assert_eq!(
        tree.rows
            .iter()
            .filter(|row| row.record_index.is_some())
            .count(),
        3
    );
}

#[test]
fn icons_distinguish_markdown_html_and_other_files_case_insensitively() {
    assert_eq!(file_icon(Path::new("spec.MD")), MARKDOWN);
    assert_eq!(file_icon(Path::new("spec.markdown")), MARKDOWN);
    assert_eq!(file_icon(Path::new("spec.HTML")), HTML);
    assert_eq!(file_icon(Path::new("spec.htm")), HTML);
    assert_eq!(file_icon(Path::new("spec.txt")), FILE);
    assert_ne!(MARKDOWN, HTML);
    assert_ne!(FOLDER_OPEN, FOLDER_CLOSED);
}

#[test]
fn empty_tree_navigation_is_safe() {
    let mut tree = Tree::default();
    tree.rebuild(&[], &[], Path::new("/managed/specs"));
    tree.move_focus(true);
    tree.move_focus(false);
    tree.toggle_focused();
    tree.expand_or_child();
    tree.collapse_or_parent();
    assert!(tree.rows.is_empty());
    assert_eq!(tree.selected_record(), None);
    assert!(!tree.focus_record(0));
}

#[cfg(unix)]
#[test]
fn alias_source_matches_physical_paths_and_missing_files() -> crate::store::Result<()> {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("herdr-inbox-tree-{}", uuid::Uuid::new_v4()));
    let content = root.join("content");
    std::fs::create_dir_all(content.join("nested"))?;
    let alias = root.join("alias");
    symlink(&content, &alias)?;
    let missing = content.join("nested/missing.md");
    let records = [record("missing", &missing.to_string_lossy(), None, None)];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[source("alias", &alias.to_string_lossy())],
        Path::new("/managed/specs"),
    );
    assert_eq!(tree.rows[0].key, "source:alias");
    assert_eq!(tree.rows[1].label, "nested");
    assert_eq!(tree.rows[2].path, missing);
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn recorded_source_identity_wins_over_overlapping_roots() {
    let records = [record(
        "one",
        "/context/missions/zeller/a.md",
        Some("outer"),
        Some("missions/zeller/a.md"),
    )];
    let mut tree = Tree::default();
    tree.rebuild(
        &records,
        &[
            source("outer", "/context"),
            source("inner", "/context/missions"),
        ],
        Path::new("/managed/specs"),
    );
    assert_eq!(tree.rows[0].key, "source:outer");
    assert_eq!(tree.rows[1].label, "missions");
    assert_eq!(tree.rows[2].label, "zeller");
}

#[test]
fn semantic_titles_do_not_change_filename_order() {
    let mut first = record("ten", "/specs/spec10.md", Some("specs"), Some("spec10.md"));
    first.title = "A first alphabetically".into();
    let mut second = record("two", "/specs/spec2.md", Some("specs"), Some("spec2.md"));
    second.title = "Z last alphabetically".into();
    let mut managed = record("managed", "/managed/specs/uuid.md", None, None);
    managed.ownership = "managed".into();
    managed.title = "Human readable spec".into();
    let mut tree = Tree::default();
    tree.rebuild(
        &[first, second, managed],
        &[source("specs", "/specs")],
        Path::new("/managed/specs"),
    );
    let leaves: Vec<_> = tree
        .rows
        .iter()
        .filter(|row| row.record_index.is_some())
        .collect();
    let two = leaves
        .iter()
        .position(|row| row.key == "record:two")
        .unwrap();
    let ten = leaves
        .iter()
        .position(|row| row.key == "record:ten")
        .unwrap();
    assert!(two < ten);
    assert_eq!(leaves[two].label, "Z last alphabetically");
    assert!(leaves.iter().any(|row| row.label == "Human readable spec"));
    assert!(!leaves.iter().any(|row| row.label == "uuid.md"));
}
