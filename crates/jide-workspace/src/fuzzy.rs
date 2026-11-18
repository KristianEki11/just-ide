use crate::error::Result;
use ignore::WalkBuilder;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub path: PathBuf,
    pub score: u32,
    pub indices: Vec<u32>,
}

pub struct FuzzyIndex {
    root: PathBuf,
    files: Vec<PathBuf>,
}

impl FuzzyIndex {
    pub fn build(root: PathBuf) -> Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let root_ref = &root;

        WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .require_git(false)
            .filter_entry(|entry| entry.file_name() != ".git")
            .build_parallel()
            .run(|| {
                let tx = tx.clone();
                Box::new(move |result| {
                    if let Ok(entry) = result {
                        if entry.file_type().is_some_and(|ft| ft.is_file()) {
                            if let Ok(rel_path) = entry.path().strip_prefix(root_ref) {
                                let _ = tx.send(rel_path.to_path_buf());
                            }
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
        drop(tx);

        let files: Vec<PathBuf> = rx.iter().collect();
        Ok(Self { root, files })
    }

    pub fn apply_fs_changes(&mut self, created: Vec<PathBuf>, removed: Vec<PathBuf>) {
        if !removed.is_empty() {
            let removed_rel: HashSet<PathBuf> = removed
                .into_iter()
                .map(|p| match p.strip_prefix(&self.root) {
                    Ok(rel) => rel.to_path_buf(),
                    Err(_) => p,
                })
                .collect();
            self.files.retain(|p| !removed_rel.contains(p));
        }

        let mut existing: HashSet<PathBuf> = self.files.iter().cloned().collect();
        for path in created {
            let rel = match path.strip_prefix(&self.root) {
                Ok(r) => r.to_path_buf(),
                Err(_) => path,
            };
            if existing.insert(rel.clone()) {
                self.files.push(rel);
            }
        }
    }

    #[must_use]
    pub fn filter(&self, query: &str, limit: usize) -> Vec<FuzzyMatch> {
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut results: Vec<FuzzyMatch> = Vec::new();
        let mut haystack_buf = Vec::new();
        let mut indices_buf = Vec::new();

        for path in &self.files {
            let path_str = path.to_string_lossy();
            haystack_buf.clear();
            let haystack = Utf32Str::new(&path_str, &mut haystack_buf);
            indices_buf.clear();

            if let Some(score) = pattern.indices(haystack, &mut matcher, &mut indices_buf) {
                results.push(FuzzyMatch {
                    path: self.root.join(path),
                    score,
                    indices: indices_buf.clone(),
                });
            }
        }

        results.sort_by_key(|a| std::cmp::Reverse(a.score));
        results.truncate(limit);
        results
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}
