//! Incremental syntax highlighter with 64-line parse checkpoints and line-level convergence.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

pub use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

use jide_core::Document;

use crate::registry::find_language_by_id;
use crate::scope_map::{map_scope_stack, SyntaxToken};

/// Distance in lines between saved incremental parse state checkpoints.
pub const CHECKPOINT_INTERVAL: usize = 64;
const MAX_CACHED_LINES: usize = 2048;

/// Loads the embedded `two-face` syntax set with extra newlines support.
#[must_use]
pub fn load_syntax_set() -> SyntaxSet {
    two_face::syntax::extra_newlines()
}

/// A contiguous slice of bytes within a single line sharing a `SyntaxToken`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightSpan {
    /// Zero-based starting byte offset within the line.
    pub start_byte: usize,
    /// Zero-based ending byte offset (exclusive) within the line.
    pub end_byte: usize,
    /// Classified syntax token for styling.
    pub token: SyntaxToken,
}

#[derive(Clone)]
struct Checkpoint {
    parse_state: ParseState,
    scope_stack: ScopeStack,
}

#[derive(Clone)]
struct CachedLine {
    hash: u64,
    pre_state: ParseState,
    pre_stack: ScopeStack,
    post_state: ParseState,
    post_stack: ScopeStack,
    spans: Vec<HighlightSpan>,
}

/// Incremental syntax highlight cache for a single document buffer.
#[derive(Clone)]
pub struct HighlightCache {
    checkpoints: BTreeMap<usize, Checkpoint>,
    line_cache: HashMap<usize, CachedLine>,
    dirty_from: usize,
    force_reparse_line: Option<usize>,
    doc_version: u64,
    language_id: String,
    scratch: String,
}

impl HighlightCache {
    /// Creates a new empty cache for the specified language.
    #[must_use]
    pub fn new(language_id: impl Into<String>) -> Self {
        Self {
            checkpoints: BTreeMap::new(),
            line_cache: HashMap::new(),
            dirty_from: 0,
            force_reparse_line: None,
            doc_version: 0,
            language_id: language_id.into(),
            scratch: String::new(),
        }
    }

    /// Returns the active language identifier.
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Returns the document version this cache is currently aligned with.
    #[must_use]
    pub fn doc_version(&self) -> u64 {
        self.doc_version
    }

    /// Returns `true` if a checkpoint exists at the given line number.
    #[must_use]
    pub fn has_checkpoint(&self, line: usize) -> bool {
        self.checkpoints.contains_key(&line)
    }

    /// Returns the number of cached checkpoints.
    #[must_use]
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Resets the cache for a new language or completely rewritten buffer.
    pub fn reset(&mut self, language_id: impl Into<String>, doc_version: u64) {
        self.checkpoints.clear();
        self.line_cache.clear();
        self.dirty_from = 0;
        self.force_reparse_line = None;
        self.language_id = language_id.into();
        self.doc_version = doc_version;
    }

    /// Invalidates checkpoints strictly after `line`, keeping prior lines clean and
    /// preserving later line entries only for state-convergence fast-path matching.
    pub fn invalidate_from(&mut self, line: usize, new_version: u64) {
        self.checkpoints.split_off(&(line.saturating_add(1)));
        self.dirty_from = self.dirty_from.min(line);
        self.force_reparse_line = Some(line);
        self.doc_version = new_version;
    }

    /// Highlights the requested line range, reusing checkpoints and converging cached lines.
    #[must_use]
    pub fn highlight_visible(
        &mut self,
        syntax_set: &SyntaxSet,
        doc: &Document,
        lines: Range<usize>,
    ) -> Vec<Vec<HighlightSpan>> {
        let total_lines = doc.line_count();
        if total_lines == 0 {
            return Vec::new();
        }

        let start_req = lines.start.min(total_lines);
        let end_req = lines.end.min(total_lines);
        let requested_count = end_req.saturating_sub(start_req);
        let mut output = Vec::with_capacity(requested_count);

        if self.language_id == "plaintext" {
            for line_idx in start_req..end_req {
                let byte_len = doc.rope.line(line_idx).len_bytes();
                output.push(default_line_spans(byte_len));
            }
            return output;
        }

        let syntax = resolve_syntax(syntax_set, &self.language_id);
        if syntax.name == "Plain Text" {
            for line_idx in start_req..end_req {
                let byte_len = doc.rope.line(line_idx).len_bytes();
                output.push(default_line_spans(byte_len));
            }
            return output;
        }

        let (resume_line, mut current_state, mut current_stack) =
            self.find_resume_point(syntax, start_req);

        if resume_line == 0 && !self.checkpoints.contains_key(&0) {
            self.checkpoints.insert(
                0,
                Checkpoint {
                    parse_state: current_state.clone(),
                    scope_stack: current_stack.clone(),
                },
            );
        }

        for line_idx in resume_line..end_req {
            if line_idx > 0 && line_idx % CHECKPOINT_INTERVAL == 0 {
                self.checkpoints
                    .entry(line_idx)
                    .or_insert_with(|| Checkpoint {
                        parse_state: current_state.clone(),
                        scope_stack: current_stack.clone(),
                    });
            }

            let line_slice = doc.rope.line(line_idx);
            let actual_byte_len = line_slice.len_bytes();
            self.scratch.clear();
            for chunk in line_slice.chunks() {
                self.scratch.push_str(chunk);
            }
            let line_hash = hash_str(&self.scratch);

            let force_this_line = self.force_reparse_line == Some(line_idx);
            if force_this_line {
                self.force_reparse_line = None;
            } else if let Some(cached) = self.line_cache.get(&line_idx) {
                if cached.hash == line_hash
                    && cached.pre_state == current_state
                    && cached.pre_stack == current_stack
                {
                    current_state = cached.post_state.clone();
                    current_stack = cached.post_stack.clone();
                    if line_idx >= start_req {
                        output.push(cached.spans.clone());
                    }
                    continue;
                }
            }

            let pre_state = current_state.clone();
            let pre_stack = current_stack.clone();

            if !self.scratch.ends_with('\n') {
                self.scratch.push('\n');
            }

            let ops = current_state
                .parse_line(&self.scratch, syntax_set)
                .unwrap_or_default();

            let spans = build_spans_for_line(&ops, &mut current_stack, actual_byte_len);

            if self.line_cache.len() >= MAX_CACHED_LINES {
                self.prune_line_cache(start_req);
            }
            self.line_cache.insert(
                line_idx,
                CachedLine {
                    hash: line_hash,
                    pre_state,
                    pre_stack,
                    post_state: current_state.clone(),
                    post_stack: current_stack.clone(),
                    spans: spans.clone(),
                },
            );

            if line_idx >= start_req {
                output.push(spans);
            }
        }

        if end_req > self.dirty_from {
            self.dirty_from = end_req;
        }

        output
    }

    fn find_resume_point(
        &self,
        syntax: &SyntaxReference,
        start_req: usize,
    ) -> (usize, ParseState, ScopeStack) {
        if start_req > 0 && start_req <= self.dirty_from {
            if let Some(prev) = self.line_cache.get(&(start_req - 1)) {
                return (start_req, prev.post_state.clone(), prev.post_stack.clone());
            }
        }

        let clean_limit = start_req.min(self.dirty_from);
        if let Some((&cp_line, cp)) = self.checkpoints.range(..=clean_limit).next_back() {
            // Check if a clean cached line closer to start_req exists after cp_line.
            if start_req > cp_line && start_req <= self.dirty_from {
                if let Some((&cached_line, cached)) = self
                    .line_cache
                    .iter()
                    .filter(|(&line, _)| line >= cp_line && line < start_req)
                    .max_by_key(|(&line, _)| line)
                {
                    return (
                        cached_line + 1,
                        cached.post_state.clone(),
                        cached.post_stack.clone(),
                    );
                }
            }
            return (cp_line, cp.parse_state.clone(), cp.scope_stack.clone());
        }

        (0, ParseState::new(syntax), ScopeStack::new())
    }

    fn prune_line_cache(&mut self, focus_line: usize) {
        self.line_cache
            .retain(|&line, _| line.abs_diff(focus_line) <= MAX_CACHED_LINES / 4);
    }
}

fn hash_str(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn resolve_syntax<'a>(syntax_set: &'a SyntaxSet, language_id: &str) -> &'a SyntaxReference {
    if let Some(spec) = find_language_by_id(language_id) {
        for name in spec.syntect_names {
            if let Some(s) = syntax_set
                .find_syntax_by_name(name)
                .or_else(|| syntax_set.find_syntax_by_extension(name))
            {
                return s;
            }
        }
    }

    syntax_set
        .find_syntax_by_token(language_id)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn default_line_spans(byte_len: usize) -> Vec<HighlightSpan> {
    if byte_len == 0 {
        Vec::new()
    } else {
        vec![HighlightSpan {
            start_byte: 0,
            end_byte: byte_len,
            token: SyntaxToken::Default,
        }]
    }
}

fn build_spans_for_line(
    ops: &[(usize, ScopeStackOp)],
    stack: &mut ScopeStack,
    line_byte_len: usize,
) -> Vec<HighlightSpan> {
    if line_byte_len == 0 {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut last_offset = 0;

    for (raw_offset, op) in ops {
        let offset = (*raw_offset).min(line_byte_len);
        if offset > last_offset {
            let token = map_scope_stack(stack);
            push_or_merge_span(&mut spans, last_offset, offset, token);
            last_offset = offset;
        }
        let _ = stack.apply(op);
    }

    if last_offset < line_byte_len {
        let token = map_scope_stack(stack);
        push_or_merge_span(&mut spans, last_offset, line_byte_len, token);
    }

    spans
}

fn push_or_merge_span(
    spans: &mut Vec<HighlightSpan>,
    start: usize,
    end: usize,
    token: SyntaxToken,
) {
    if start >= end {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.token == token && last.end_byte == start {
            last.end_byte = end;
            return;
        }
    }
    spans.push(HighlightSpan {
        start_byte: start,
        end_byte: end,
        token,
    });
}
