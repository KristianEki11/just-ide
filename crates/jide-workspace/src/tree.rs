use crate::error::{Result, WorkspaceError};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

fn natural_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    let mut a_chars = a_lower.chars().peekable();
    let mut b_chars = b_lower.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&ac), Some(&bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let a_num = extract_number(&mut a_chars);
                    let b_num = extract_number(&mut b_chars);
                    match a_num.cmp(&b_num) {
                        std::cmp::Ordering::Equal => {}
                        other => return other,
                    }
                } else {
                    a_chars.next();
                    b_chars.next();
                    match ac.cmp(&bc) {
                        std::cmp::Ordering::Equal => {}
                        other => return other,
                    }
                }
            }
        }
    }
}

fn extract_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> u64 {
    let mut num = 0u64;
    while let Some(&c) = chars.peek() {
        if let Some(digit) = c.to_digit(10) {
            num = num.saturating_mul(10).saturating_add(u64::from(digit));
            chars.next();
        } else {
            break;
        }
    }
    num
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Option<Vec<TreeNode>>,
}

#[derive(Debug, Clone)]
pub struct FileTree {
    root: PathBuf,
    show_ignored: bool,
    pub root_node: TreeNode,
}

impl FileTree {
    pub fn open(root: PathBuf, show_ignored: bool) -> Result<Self> {
        if !root.exists() {
            return Err(WorkspaceError::InvalidPath(root));
        }

        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let root_node = TreeNode {
            path: root.clone(),
            name,
            is_dir: true,
            expanded: false,
            children: None,
        };

        let mut tree = Self {
            root,
            show_ignored,
            root_node,
        };

        tree.expand_dir(&tree.root.clone())?;
        Ok(tree)
    }

    pub fn expand_dir(&mut self, dir: &Path) -> Result<()> {
        let node = Self::find_node_mut(&mut self.root_node, dir)
            .ok_or_else(|| WorkspaceError::InvalidPath(dir.to_path_buf()))?;

        if !node.is_dir {
            return Ok(());
        }

        let mut entries = Vec::new();
        let show_ignored = self.show_ignored;
        let walker = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(false)
            .git_ignore(!show_ignored)
            .require_git(false)
            .filter_entry(move |entry| show_ignored || entry.file_name() != ".git")
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path == dir {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let is_dir = path.is_dir();
            entries.push(TreeNode {
                path: path.to_path_buf(),
                name,
                is_dir,
                expanded: false,
                children: None,
            });
        }

        // Sort: folders first, then case-insensitive natural sort
        entries.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                return b.is_dir.cmp(&a.is_dir);
            }
            natural_compare(&a.name, &b.name)
        });

        node.expanded = true;
        node.children = Some(entries);
        Ok(())
    }

    pub fn toggle_dir(&mut self, dir: &Path) -> Result<()> {
        let is_expanded = Self::find_node_mut(&mut self.root_node, dir)
            .ok_or_else(|| WorkspaceError::InvalidPath(dir.to_path_buf()))?
            .expanded;

        if is_expanded {
            if let Some(node) = Self::find_node_mut(&mut self.root_node, dir) {
                node.expanded = false;
            }
            Ok(())
        } else {
            self.expand_dir(dir)
        }
    }

    fn find_node_mut<'a>(node: &'a mut TreeNode, target: &Path) -> Option<&'a mut TreeNode> {
        if node.path == target {
            return Some(node);
        }

        if let Some(ref mut children) = node.children {
            for child in children {
                if let Some(found) = Self::find_node_mut(child, target) {
                    return Some(found);
                }
            }
        }

        None
    }

    pub fn create_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::File::create(path)?;
        Ok(())
    }

    pub fn create_dir(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }

    pub fn rename_path(&self, old: &Path, new: &Path) -> Result<()> {
        std::fs::rename(old, new)?;
        Ok(())
    }

    pub fn delete_to_trash(&self, path: &Path) -> Result<()> {
        trash::delete(path)?;
        Ok(())
    }

    pub fn duplicate_or_paste_file(&self, src: &Path, dest: &Path) -> Result<()> {
        if src.is_dir() {
            Self::copy_dir_recursive(src, dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src, dest)?;
        }
        Ok(())
    }

    fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &dest_path)?;
            } else {
                std::fs::copy(&src_path, &dest_path)?;
            }
        }
        Ok(())
    }
}
