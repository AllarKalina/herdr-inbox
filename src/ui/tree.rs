use crate::settings::SpecSource;
use crate::store::Record;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub(super) const FOLDER_OPEN: &str = "\u{f115}";
pub(super) const FOLDER_CLOSED: &str = "\u{f114}";
pub(super) const MARKDOWN: &str = "\u{e73e}";
pub(super) const HTML: &str = "\u{e736}";
pub(super) const FILE: &str = "\u{f016}";

pub(super) fn file_icon(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown" | "mdown" | "mdx") => MARKDOWN,
        Some("html" | "htm") => HTML,
        _ => FILE,
    }
}

#[derive(Clone, Debug)]
pub(super) struct TreeRow {
    pub key: String,
    pub label: String,
    pub path: PathBuf,
    pub record_index: Option<usize>,
    pub depth: usize,
    pub prefix: String,
    pub is_folder: bool,
    pub expanded: bool,
}

#[derive(Default)]
pub(super) struct Tree {
    pub rows: Vec<TreeRow>,
    pub focused: usize,
    collapsed: HashSet<String>,
    all_rows: Vec<TreeRow>,
}

struct Node {
    key: String,
    label: String,
    path: PathBuf,
    record_index: Option<usize>,
    children: Vec<Node>,
}

impl Tree {
    pub fn rebuild(&mut self, records: &[Record], sources: &[SpecSource], managed_root: &Path) {
        let focused_key = self.rows.get(self.focused).map(|row| row.key.clone());
        let mut roots: Vec<Node> = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let (key, root_path, label, relative) = location(record, sources, managed_root);
            let root_index = roots
                .iter()
                .position(|node| node.key == key)
                .unwrap_or_else(|| {
                    roots.push(Node {
                        key,
                        path: root_path,
                        label,
                        record_index: None,
                        children: Vec::new(),
                    });
                    roots.len() - 1
                });
            roots[root_index].insert(&relative, record, index);
        }
        let labels: Vec<_> = roots.iter().map(|root| root.label.clone()).collect();
        for root in &mut roots {
            if labels.iter().filter(|label| **label == root.label).count() > 1 {
                root.label = root.path.display().to_string();
            }
            root.sort();
        }
        roots.sort_by(|a, b| natural_cmp(&a.label, &b.label).then_with(|| a.key.cmp(&b.key)));
        self.all_rows.clear();
        for root in &roots {
            root.flatten(0, "", "", &mut self.all_rows);
        }
        self.collapsed.retain(|key| {
            self.all_rows
                .iter()
                .any(|row| row.is_folder && row.key == *key)
        });
        self.refresh(focused_key.as_deref());
    }

    pub fn move_focus(&mut self, forward: bool) {
        self.focused = if forward {
            (self.focused + 1).min(self.rows.len().saturating_sub(1))
        } else {
            self.focused.saturating_sub(1)
        };
    }

    pub fn selected_record(&self) -> Option<usize> {
        self.rows.get(self.focused).and_then(|row| row.record_index)
    }

    pub fn focus_record(&mut self, index: usize) -> bool {
        let Some(position) = self
            .all_rows
            .iter()
            .position(|row| row.record_index == Some(index))
        else {
            return false;
        };
        let key = self.all_rows[position].key.clone();
        let mut depth = self.all_rows[position].depth;
        for row in self.all_rows[..position].iter().rev() {
            if row.depth < depth {
                self.collapsed.remove(&row.key);
                depth = row.depth;
            }
        }
        self.refresh(Some(&key));
        true
    }

    pub fn toggle_focused(&mut self) {
        let Some(row) = self.rows.get(self.focused).filter(|row| row.is_folder) else {
            return;
        };
        let key = row.key.clone();
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key.clone());
        }
        self.refresh(Some(&key));
    }

    pub fn collapse_or_parent(&mut self) {
        let Some(row) = self.rows.get(self.focused) else {
            return;
        };
        if row.is_folder && row.expanded {
            self.toggle_focused();
        } else if row.depth > 0 {
            let depth = row.depth;
            if let Some(parent) = self.rows[..self.focused]
                .iter()
                .rposition(|row| row.depth < depth)
            {
                self.focused = parent;
            }
        }
    }

    pub fn expand_or_child(&mut self) {
        let Some(row) = self.rows.get(self.focused) else {
            return;
        };
        if row.is_folder {
            if !row.expanded {
                self.toggle_focused();
            } else if self
                .rows
                .get(self.focused + 1)
                .is_some_and(|child| child.depth > row.depth)
            {
                self.focused += 1;
            }
        }
    }

    fn refresh(&mut self, key: Option<&str>) {
        self.rows.clear();
        let mut hidden_below = None;
        for row in &self.all_rows {
            if hidden_below.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            hidden_below = None;
            let mut row = row.clone();
            row.expanded = row.is_folder && !self.collapsed.contains(&row.key);
            if row.is_folder && !row.expanded {
                hidden_below = Some(row.depth);
            }
            self.rows.push(row);
        }
        self.focused = key
            .and_then(|key| self.rows.iter().position(|row| row.key == key))
            .or_else(|| self.rows.iter().position(|row| row.record_index == Some(0)))
            .unwrap_or(0);
    }
}

impl Node {
    fn insert(&mut self, relative: &Path, record: &Record, index: usize) {
        let parts: Vec<_> = relative
            .components()
            .filter_map(|part| match part {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        let mut parent = self;
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            let path = parent.path.join(part);
            let key = format!("{}/folder:{}", parent.key, part.to_string_lossy());
            let child_index = parent
                .children
                .iter()
                .position(|child| child.key == key)
                .unwrap_or_else(|| {
                    parent.children.push(Node {
                        key,
                        label: part.to_string_lossy().into_owned(),
                        path,
                        record_index: None,
                        children: Vec::new(),
                    });
                    parent.children.len() - 1
                });
            parent = &mut parent.children[child_index];
        }
        let label = record.display_title().to_owned();
        parent.children.push(Node {
            key: format!("record:{}", record.id),
            label,
            path: record.spec_path.clone(),
            record_index: Some(index),
            children: Vec::new(),
        });
    }

    fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            a.record_index
                .is_some()
                .cmp(&b.record_index.is_some())
                .then_with(|| {
                    if a.record_index.is_some() && b.record_index.is_some() {
                        natural_cmp(
                            &a.path.file_name().unwrap_or_default().to_string_lossy(),
                            &b.path.file_name().unwrap_or_default().to_string_lossy(),
                        )
                    } else {
                        natural_cmp(&a.label, &b.label)
                    }
                })
                .then_with(|| a.key.cmp(&b.key))
        });
        for child in &mut self.children {
            child.sort();
        }
    }

    fn flatten(&self, depth: usize, prefix: &str, guides: &str, output: &mut Vec<TreeRow>) {
        output.push(TreeRow {
            key: self.key.clone(),
            label: self.label.clone(),
            path: self.path.clone(),
            record_index: self.record_index,
            depth,
            prefix: prefix.into(),
            is_folder: self.record_index.is_none(),
            expanded: self.record_index.is_none(),
        });
        for (index, child) in self.children.iter().enumerate() {
            let last = index + 1 == self.children.len();
            let prefix = format!("{guides}{}", if last { "└─ " } else { "├─ " });
            let next_guides = format!("{guides}{}", if last { "   " } else { "│  " });
            child.flatten(depth + 1, &prefix, &next_guides, output);
        }
    }
}

fn location(
    record: &Record,
    sources: &[SpecSource],
    managed_root: &Path,
) -> (String, PathBuf, String, PathBuf) {
    if let Some(source) = sources
        .iter()
        .find(|source| record.source_id.as_deref() == Some(&source.id))
    {
        let relative = record
            .source_relative_path
            .as_ref()
            .filter(|path| safe_relative(path))
            .cloned()
            .or_else(|| relative_to(&record.spec_path, &source.path))
            .unwrap_or_else(|| filename(&record.spec_path));
        return (
            format!("source:{}", source.id),
            source.path.clone(),
            basename(&source.path),
            relative,
        );
    }
    if let Some((source, relative)) = sources
        .iter()
        .filter_map(|source| {
            relative_to(&record.spec_path, &source.path).map(|relative| (source, relative))
        })
        .max_by_key(|(source, _)| source.path.components().count())
    {
        return (
            format!("source:{}", source.id),
            source.path.clone(),
            basename(&source.path),
            relative,
        );
    }
    if record.ownership == "managed" || record.spec_path.starts_with(managed_root) {
        let relative = record
            .spec_path
            .strip_prefix(managed_root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| filename(&record.spec_path));
        return (
            format!("managed:{}", managed_root.display()),
            managed_root.into(),
            "Inbox specs".into(),
            relative,
        );
    }
    let parent = record.spec_path.parent().unwrap_or_else(|| Path::new("."));
    (
        format!("external:{}", parent.display()),
        parent.into(),
        basename(parent),
        filename(&record.spec_path),
    )
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn relative_to(path: &Path, root: &Path) -> Option<PathBuf> {
    let relative =
        path.strip_prefix(root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                let canonical_root = root.canonicalize().ok()?;
                // Canonicalizing only the parent keeps a missing spec visible under its existing alias.
                let canonical_path = path.canonicalize().ok().or_else(|| {
                    Some(path.parent()?.canonicalize().ok()?.join(path.file_name()?))
                })?;
                canonical_path
                    .strip_prefix(canonical_root)
                    .ok()
                    .map(Path::to_path_buf)
            })?;
    safe_relative(&relative).then_some(relative)
}

fn filename(path: &Path) -> PathBuf {
    path.file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("spec.md"))
}
fn basename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let lower_a = a.to_lowercase();
    let lower_b = b.to_lowercase();
    let (a_bytes, b_bytes) = (lower_a.as_bytes(), lower_b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a_bytes.len() && j < b_bytes.len() {
        if a_bytes[i].is_ascii_digit() && b_bytes[j].is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a_bytes.len() && a_bytes[i].is_ascii_digit() {
                i += 1;
            }
            while j < b_bytes.len() && b_bytes[j].is_ascii_digit() {
                j += 1;
            }
            let number_a = lower_a[start_a..i].trim_start_matches('0');
            let number_b = lower_b[start_b..j].trim_start_matches('0');
            let order = number_a
                .len()
                .cmp(&number_b.len())
                .then_with(|| number_a.cmp(number_b));
            if order != Ordering::Equal {
                return order;
            }
        } else {
            let order = a_bytes[i].cmp(&b_bytes[j]);
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    (a_bytes.len() - i)
        .cmp(&(b_bytes.len() - j))
        .then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests;
