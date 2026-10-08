use super::*;
use crate::store::SpecStatus;
use std::fs;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("herdr-inbox-tree-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn source(&self, id: &str, path: &str) -> SpecSource {
        let path = self.0.join(path);
        fs::create_dir_all(&path).unwrap();
        SpecSource {
            id: id.into(),
            path,
            recursive: true,
            include: vec!["**/*.md".into(), "**/*.html".into()],
            exclude: Vec::new(),
        }
    }
    fn record(&self, id: &str, path: &str) -> Record {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# Mock spec\n").unwrap();
        Record {
            title: path.file_name().unwrap().to_string_lossy().into_owned(),
            ..Record::blank(id.into(), path, SpecStatus::Done, 1)
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nested_domains_have_stepped_guides_folders_first_and_natural_filenames() {
    let fixture = Fixture::new();
    let source = fixture.source("context", "context");
    let records = [
        fixture.record("ten", "context/designs/spec10.md"),
        fixture.record("two", "context/designs/spec2.md"),
        fixture.record("mission", "context/missions/zeller/spec.html"),
        fixture.record("root", "context/a.md"),
    ];
    let mut tree = Tree::default();
    tree.rebuild(&records, &[source]);
    let rows: Vec<_> = tree
        .rows
        .iter()
        .map(|row| (row.label.as_str(), row.depth, row.prefix.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("designs", 0, ""),
            ("spec2.md", 1, "├─ "),
            ("spec10.md", 1, "└─ "),
            ("missions", 0, ""),
            ("zeller", 1, "└─ "),
            ("spec.html", 2, "   └─ "),
            ("a.md", 0, "")
        ]
    );
    assert_eq!(tree.selected_record(), Some(0));
    assert_eq!(tree.rows[0].path, fixture.0.join("context/designs"));
    assert!(
        tree.rows
            .iter()
            .filter(|row| row.is_folder)
            .all(|row| row.expanded)
    );
}

#[test]
fn no_configured_sources_means_empty_tree_even_for_managed_and_external_records() {
    let fixture = Fixture::new();
    let managed = fixture.record("managed", "appdata/specs/uuid.md");
    let records = [managed, fixture.record("external", "external/spec.md")];
    let mut tree = Tree::default();
    tree.rebuild(&records, &[]);
    assert!(tree.rows.is_empty());
    assert!(records.iter().all(|record| !includes(record, &[])));
    tree.move_focus(true);
    tree.move_focus(false);
    tree.toggle_focused();
    tree.expand_or_child();
    tree.collapse_or_parent();
    assert_eq!(tree.selected_record(), None);
    assert!(!tree.focus_record(0));
}

#[test]
fn cached_source_metadata_cannot_import_outside_records_or_invent_domains() {
    let fixture = Fixture::new();
    let sources = [fixture.source("context", "context")];
    let mut outside = fixture.record("outside", "elsewhere/outside.md");
    outside.source_id = Some("context".into());
    outside.source_relative_path = Some("invented/outside.md".into());
    let mut inside = fixture.record("inside", "context/designs/inside.md");
    inside.source_id = Some("obsolete".into());
    inside.source_relative_path = Some("invented/inside.md".into());
    let records = [outside, inside];
    let mut tree = Tree::default();
    tree.rebuild(&records, &sources);
    assert!(!includes(&records[0], &sources));
    assert!(includes(&records[1], &sources));
    assert_eq!(tree.rows[0].label, "designs");
    assert_eq!(tree.rows[1].record_index, Some(1));
    assert!(!tree.rows.iter().any(|row| row.label == "invented"));
    assert!(!tree.focus_record(0));
}

#[test]
fn missing_files_and_directory_records_are_excluded() {
    let fixture = Fixture::new();
    let source = fixture.source("context", "context");
    let missing = fixture.record("missing", "context/missing.md");
    fs::remove_file(&missing.spec_path).unwrap();
    let mut directory = fixture.record("directory", "context/directory.md");
    directory.spec_path = source.path.clone();
    let mut tree = Tree::default();
    tree.rebuild(
        &[missing.clone(), directory.clone()],
        std::slice::from_ref(&source),
    );
    assert!(tree.rows.is_empty());
    assert!(!includes(&missing, std::slice::from_ref(&source)));
    assert!(!includes(&directory, &[source]));
}

#[test]
fn deepest_actual_source_wins_over_cached_identity() {
    let fixture = Fixture::new();
    let sources = [
        fixture.source("outer", "context"),
        fixture.source("inner", "context/missions"),
    ];
    let mut record = fixture.record("one", "context/missions/zeller/a.md");
    record.source_id = Some("outer".into());
    record.source_relative_path = Some("missions/zeller/a.md".into());
    let mut tree = Tree::default();
    tree.rebuild(&[record], &sources);
    assert_eq!(tree.rows[0].key, "source:inner/folder:zeller");
    assert_eq!(tree.rows[0].label, "zeller");
    assert_eq!(tree.rows[0].depth, 0);
}

#[test]
fn focus_uses_record_uuid_when_updates_reorder_metadata() {
    let fixture = Fixture::new();
    let sources = [fixture.source("specs", "specs")];
    let mut records = [
        fixture.record("one", "specs/a.md"),
        fixture.record("two", "specs/b.md"),
    ];
    let mut tree = Tree::default();
    tree.rebuild(&records, &sources);
    assert!(tree.focus_record(1));
    records.swap(0, 1);
    records[0].title = "Updated title".into();
    tree.rebuild(&records, &sources);
    assert_eq!(tree.selected_record(), Some(0));
    assert_eq!(tree.rows[tree.focused].key, "record:two");
    assert_eq!(tree.rows[tree.focused].label, "Updated title");
}

#[test]
fn folding_navigation_and_focus_record_respect_boundaries() {
    let fixture = Fixture::new();
    let sources = [fixture.source("specs", "specs")];
    let records = [
        fixture.record("one", "specs/domain/nested/a.md"),
        fixture.record("two", "specs/b.md"),
    ];
    let mut tree = Tree::default();
    tree.rebuild(&records, &sources);
    tree.collapse_or_parent();
    assert_eq!(tree.rows[tree.focused].label, "nested");
    tree.collapse_or_parent();
    assert!(!tree.rows[tree.focused].expanded);
    assert_eq!(tree.rows.len(), 3);
    tree.collapse_or_parent();
    assert_eq!(tree.rows[tree.focused].label, "domain");
    tree.collapse_or_parent();
    assert_eq!(tree.rows.len(), 2);
    assert!(tree.focus_record(0));
    assert_eq!(tree.rows.len(), 4);
    tree.focused = 0;
    tree.collapse_or_parent();
    assert_eq!(tree.rows.len(), 2);
    tree.move_focus(false);
    assert_eq!(tree.focused, 0);
    tree.expand_or_child();
    tree.expand_or_child();
    assert_eq!(tree.rows[tree.focused].label, "nested");
    tree.toggle_focused();
    let key = tree.rows[tree.focused].key.clone();
    tree.rebuild(&records, &sources);
    assert_eq!(tree.rows[tree.focused].key, key);
    assert!(!tree.rows[tree.focused].expanded);
    assert!(!tree.focus_record(99));
}

#[test]
fn immediate_children_sort_globally_without_merging_same_named_domains() {
    let fixture = Fixture::new();
    let sources = [fixture.source("one", "one"), fixture.source("two", "two")];
    let records = [
        fixture.record("z", "one/zeta/a.md"),
        fixture.record("a", "two/alpha/a.md"),
        fixture.record("d1", "one/designs/one.md"),
        fixture.record("d2", "two/designs/two.md"),
        fixture.record("ten", "one/spec10.md"),
        fixture.record("two", "two/spec2.md"),
    ];
    let mut tree = Tree::default();
    tree.rebuild(&records, &sources);
    let top: Vec<_> = tree.rows.iter().filter(|row| row.depth == 0).collect();
    assert_eq!(
        top.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
        vec![
            "alpha",
            "designs",
            "designs",
            "zeta",
            "spec2.md",
            "spec10.md"
        ]
    );
    assert!(top.iter().all(|row| row.prefix.is_empty()));
    assert_ne!(top[1].key, top[2].key);
    tree.focused = tree
        .rows
        .iter()
        .position(|row| row.key == "source:one/folder:designs")
        .unwrap();
    tree.toggle_focused();
    assert!(!tree.rows.iter().any(|row| row.key == "record:d1"));
    assert!(tree.rows.iter().any(|row| row.key == "record:d2"));
    assert!(tree.focus_record(2));
    assert_eq!(tree.selected_record(), Some(2));
}

#[test]
fn semantic_titles_do_not_change_filename_order() {
    let fixture = Fixture::new();
    let sources = [fixture.source("specs", "specs")];
    let mut first = fixture.record("ten", "specs/spec10.md");
    first.title = "A first alphabetically".into();
    let mut second = fixture.record("two", "specs/spec2.md");
    second.title = "Z last alphabetically".into();
    let mut tree = Tree::default();
    tree.rebuild(&[first, second], &sources);
    assert_eq!(tree.rows[0].label, "Z last alphabetically");
    assert_eq!(tree.rows[1].label, "A first alphabetically");
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

#[cfg(unix)]
#[test]
fn canonical_aliases_work_but_symlink_and_parent_escapes_do_not() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let source = fixture.source("context", "context");
    let inside = fixture.record("inside", "context/designs/inside.md");
    let outside = fixture.record("outside", "outside/spec.md");
    let alias = fixture.0.join("alias");
    symlink(&source.path, &alias).unwrap();
    let mut aliased_source = source.clone();
    aliased_source.path = alias;
    assert!(includes(&inside, std::slice::from_ref(&aliased_source)));
    let mut escaped_file = outside.clone();
    escaped_file.spec_path = source.path.join("linked.md");
    symlink(&outside.spec_path, &escaped_file.spec_path).unwrap();
    let mut escaped_directory = outside.clone();
    symlink(fixture.0.join("outside"), source.path.join("linked-dir")).unwrap();
    escaped_directory.spec_path = source.path.join("linked-dir/spec.md");
    let mut escaped_parent = outside.clone();
    escaped_parent.spec_path = source.path.join("../outside/spec.md");
    let records = [inside, escaped_file, escaped_directory, escaped_parent];
    let mut tree = Tree::default();
    tree.rebuild(&records, std::slice::from_ref(&aliased_source));
    assert_eq!(tree.rows.len(), 2);
    assert_eq!(tree.rows[0].key, "source:context/folder:designs");
    for record in &records[1..] {
        assert!(!includes(record, std::slice::from_ref(&aliased_source)));
    }
}

#[test]
fn current_filters_and_recursion_hide_stale_metadata() {
    let fixture = Fixture::new();
    let mut source = fixture.source("specs", "specs");
    let records = [
        fixture.record("top", "specs/top.md"),
        fixture.record("nested", "specs/domain/nested.md"),
        fixture.record("hidden", "specs/hidden/private.md"),
        fixture.record("html", "specs/page.html"),
    ];
    source.include = vec!["**/*.md".into()];
    source.exclude = vec!["hidden".into()];
    let mut tree = Tree::default();
    tree.rebuild(&records, std::slice::from_ref(&source));
    assert_eq!(
        tree.rows
            .iter()
            .filter(|row| row.record_index.is_some())
            .count(),
        2
    );
    assert!(!includes(&records[2], std::slice::from_ref(&source)));
    assert!(!includes(&records[3], std::slice::from_ref(&source)));
    source.recursive = false;
    tree.rebuild(&records, std::slice::from_ref(&source));
    assert_eq!(tree.rows.len(), 1);
    assert_eq!(tree.rows[0].key, "record:top");
    assert!(!includes(&records[1], std::slice::from_ref(&source)));
    source.include = vec!["**/*.html".into()];
    tree.rebuild(&records, &[source]);
    assert_eq!(tree.rows.len(), 1);
    assert_eq!(tree.rows[0].key, "record:html");
}
