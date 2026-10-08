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
    pub fn rebuild(&mut self, records: &[Record], sources: &[SpecSource]) {
        let focused_key = self.rows.get(self.focused).map(|row| row.key.clone());
        let mut roots: Vec<Node> = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let Some((source, relative)) = source_location(record, sources) else {
                continue;
            };
            let key = format!("source:{}", source.id);
            let root_path = source.path.clone();
            let label = basename(&source.path);
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
        for root in &mut roots {
            root.sort();
        }
        // Source directories define the Inbox boundary. Only their contents are rows.
        let mut top_level: Vec<_> = roots.iter().flat_map(|root| &root.children).collect();
        top_level.sort_by(|a, b| a.compare(b));
        self.all_rows.clear();
        for node in top_level {
            node.flatten(0, "", "", &mut self.all_rows);
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
        self.children.sort_by(Self::compare);
        for child in &mut self.children {
            child.sort();
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        self.record_index
            .is_some()
            .cmp(&other.record_index.is_some())
            .then_with(|| {
                if self.record_index.is_some() && other.record_index.is_some() {
                    natural_cmp(
                        &self.path.file_name().unwrap_or_default().to_string_lossy(),
                        &other.path.file_name().unwrap_or_default().to_string_lossy(),
                    )
                } else {
                    natural_cmp(&self.label, &other.label)
                }
            })
            .then_with(|| self.key.cmp(&other.key))
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

pub(super) fn includes(record: &Record, sources: &[SpecSource]) -> bool {
    source_location(record, sources).is_some()
}

fn source_location<'a>(
    record: &Record,
    sources: &'a [SpecSource],
) -> Option<(&'a SpecSource, PathBuf)> {
    // Metadata references never establish membership: the actual regular file must
    // physically belong to a configured directory, including after symlink resolution.
    let path = record.spec_path.canonicalize().ok()?;
    if !path.is_file() {
        return None;
    }
    crate::settings::source_location(sources, &path)
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
