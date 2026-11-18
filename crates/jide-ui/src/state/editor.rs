use super::{AppState, Effect};
use jide_core::{CoreError, Document, MoveOp, Range, Selection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorCmd {
    Insert(String),
    Backspace,
    Delete,
    Newline,
    Indent,
    Outdent,
    MoveLineUp,
    MoveLineDown,
    DuplicateLineUp,
    DuplicateLineDown,
    DeleteLine,
    Undo,
    Redo,
    SelectAll,
    Move { op: MoveOp, extend: bool },
    Click { index: usize, extend: bool },
    DoubleClick(usize),
    TripleClick(usize),
    Drag(usize),
    Copy,
    Cut,
    Paste,
    PasteText(String),
    AddNextOccurrence,
    SelectAllOccurrences,
    AddCursorAbove,
    AddCursorBelow,
    AddCursorAtClick(usize),
    ColumnBoxSelect { anchor_idx: usize, head_idx: usize },
    ToggleLineComment,
    ToggleBlockComment,
    TransformUpper,
    TransformLower,
    SortLines { descending: bool, unique: bool },
    TrimTrailingWhitespace,
    ExpandLineSelection,
    InsertLineBelow,
    InsertLineAbove,
    JumpToMatchingBracket,
}

pub(super) fn dispatch_editor(state: &mut AppState, cmd: EditorCmd) -> Vec<Effect> {
    let Some(index) = state.focused_doc_idx() else {
        return vec![];
    };
    let doc_id = state.documents[index].id;
    let prev_head = state.documents[index].selection.primary_range().head;
    if matches!(
        cmd,
        EditorCmd::Click { extend: false, .. } | EditorCmd::JumpToMatchingBracket
    ) {
        state.nav_history.record(doc_id, prev_head);
    }

    let auto_close_pairs = state.auto_close_pairs;
    let now_ms = state.now_ms;
    let whole_line_clip = state.whole_line_clipboard.clone();

    let doc = &mut state.documents[index];
    let prev_version = doc.version;
    let mut effects = Vec::new();
    let result = match cmd {
        EditorCmd::Copy | EditorCmd::Cut => {
            let whole_line = doc.selection.ranges.iter().all(Range::is_empty);
            let mut text = if whole_line {
                doc.rope
                    .line(doc.rope.char_to_line(doc.selection.primary_range().head))
                    .to_string()
            } else {
                doc.selected_text()
            };
            if whole_line && !text.ends_with('\n') {
                text.push('\n');
            }
            state.whole_line_clipboard = whole_line.then(|| text.clone());
            if cmd == EditorCmd::Cut {
                effects.push(Effect::CutToClipboard {
                    text,
                    doc_id: doc.id,
                    version: doc.version,
                    selection: doc.selection.clone(),
                    whole_line,
                });
            } else {
                effects.push(Effect::CopyToClipboard(text));
            }
            Ok(())
        }
        EditorCmd::Paste => {
            effects.push(Effect::RequestPaste);
            Ok(())
        }
        EditorCmd::PasteText(text) => {
            if whole_line_clip.as_ref() == Some(&text) {
                let line = doc.rope.char_to_line(doc.selection.primary_range().head);
                doc.selection = Selection::point(doc.rope.line_to_char(line));
            }
            doc.insert_text(&text.replace("\r\n", "\n").replace('\r', "\n"), now_ms)
        }
        EditorCmd::JumpToMatchingBracket => {
            jump_matching_bracket(doc);
            let target = doc.selection.primary_range().head;
            state.nav_history.record(doc_id, target);
            Ok(())
        }
        other => apply(doc, other, now_ms, auto_close_pairs),
    };
    if state.documents[index].version != prev_version {
        if let Some(meta) = state.tab_states.get_mut(index) {
            meta.preview = false;
        }
    }
    if let Err(error) = result {
        state.toast = Some(error.to_string());
    }
    effects.push(Effect::UpdateWindowTitle(state.window_title()));
    effects
}

fn apply(
    doc: &mut Document,
    cmd: EditorCmd,
    now: u64,
    auto_close_pairs: bool,
) -> Result<(), CoreError> {
    match cmd {
        EditorCmd::Insert(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) if auto_close_pairs => doc.insert_with_auto_close(ch, now),
                (Some(ch), None) => doc.insert_char(ch, now),
                _ => doc.insert_text(&text, now),
            }
        }
        EditorCmd::Backspace => {
            if auto_close_pairs {
                doc.backspace_with_auto_close(now)
            } else {
                doc.delete_backward(now)
            }
        }
        EditorCmd::Delete => doc.delete_forward(now),
        EditorCmd::Newline => doc.insert_newline_auto_indent(now),
        EditorCmd::Indent => doc.indent_lines(now),
        EditorCmd::Outdent => doc.outdent_lines(now),
        EditorCmd::MoveLineUp => doc.move_lines_up(now),
        EditorCmd::MoveLineDown => doc.move_lines_down(now),
        EditorCmd::DuplicateLineUp => doc.duplicate_lines_up(now),
        EditorCmd::DuplicateLineDown => doc.duplicate_lines_down(now),
        EditorCmd::DeleteLine => doc.delete_lines(now),
        EditorCmd::Undo => doc.undo().map(|_| ()),
        EditorCmd::Redo => doc.redo().map(|_| ()),
        EditorCmd::SelectAll => {
            doc.select_all();
            Ok(())
        }
        EditorCmd::Move { op, extend } => {
            doc.move_cursor(op, extend);
            Ok(())
        }
        EditorCmd::Click { index, extend } => {
            point(doc, index, extend);
            Ok(())
        }
        EditorCmd::Drag(index) => {
            point(doc, index, true);
            Ok(())
        }
        other => apply_power_edit(doc, &other, now),
    }
}

fn apply_power_edit(doc: &mut Document, cmd: &EditorCmd, now: u64) -> Result<(), CoreError> {
    match cmd {
        &EditorCmd::DoubleClick(index) => {
            let index = index.min(doc.char_count());
            let mut start = index;
            let mut end = index;
            while start > 0 && jide_core::grapheme::is_word_char(doc.rope.char(start - 1)) {
                start -= 1;
            }
            while end < doc.char_count() && jide_core::grapheme::is_word_char(doc.rope.char(end)) {
                end += 1;
            }
            doc.selection = Selection::single(Range {
                anchor: start,
                head: end,
            });
            doc.preferred_col = None;
            Ok(())
        }
        &EditorCmd::TripleClick(index) => {
            let line = doc.rope.char_to_line(index.min(doc.char_count()));
            let start = doc.rope.line_to_char(line);
            doc.selection = Selection::single(Range {
                anchor: start,
                head: start + doc.rope.line(line).len_chars(),
            });
            doc.preferred_col = None;
            Ok(())
        }
        EditorCmd::AddNextOccurrence => {
            doc.add_next_occurrence();
            Ok(())
        }
        EditorCmd::SelectAllOccurrences => {
            doc.select_all_occurrences();
            Ok(())
        }
        EditorCmd::AddCursorAbove => {
            doc.add_cursor_vertical(-1);
            Ok(())
        }
        EditorCmd::AddCursorBelow => {
            doc.add_cursor_vertical(1);
            Ok(())
        }
        &EditorCmd::AddCursorAtClick(idx) => {
            add_cursor_at(doc, idx);
            Ok(())
        }
        &EditorCmd::ColumnBoxSelect {
            anchor_idx,
            head_idx,
        } => {
            column_select(doc, anchor_idx, head_idx);
            Ok(())
        }
        EditorCmd::ToggleLineComment => {
            let token = jide_syntax::find_language_by_id(doc.language.as_str())
                .and_then(|s| s.line_comment)
                .unwrap_or("//");
            doc.toggle_line_comment(token, now)
        }
        EditorCmd::ToggleBlockComment => {
            let (open, close) = jide_syntax::find_language_by_id(doc.language.as_str())
                .and_then(|s| s.block_comment)
                .unwrap_or(("/*", "*/"));
            doc.toggle_block_comment(open, close, now)
        }
        EditorCmd::TransformUpper => doc.transform_case_upper(now),
        EditorCmd::TransformLower => doc.transform_case_lower(now),
        &EditorCmd::SortLines { descending, unique } => doc.sort_lines(descending, unique, now),
        EditorCmd::TrimTrailingWhitespace => doc.trim_trailing_whitespace(now),
        EditorCmd::ExpandLineSelection => {
            doc.expand_line_selection();
            Ok(())
        }
        EditorCmd::InsertLineBelow => doc.insert_line_below(now),
        EditorCmd::InsertLineAbove => doc.insert_line_above(now),
        _ => Ok(()),
    }
}

fn add_cursor_at(doc: &mut Document, idx: usize) {
    let clamped = idx.min(doc.char_count());
    let mut ranges: Vec<Range> = doc.selection.ranges.to_vec();
    ranges.push(Range::point(clamped));
    let primary = ranges.len().saturating_sub(1);
    doc.selection = Selection::new(ranges, primary);
    doc.preferred_col = None;
}

fn column_select(doc: &mut Document, anchor_idx: usize, head_idx: usize) {
    let to_lc = |idx: usize| {
        let clamped = idx.min(doc.char_count());
        let line = doc.rope.char_to_line(clamped);
        let col = clamped.saturating_sub(doc.rope.line_to_char(line));
        (line, col)
    };
    doc.select_column_box(to_lc(anchor_idx), to_lc(head_idx));
}

fn jump_matching_bracket(doc: &mut Document) {
    let head = doc.selection.primary_range().head;
    let found = doc
        .find_matching_bracket(head)
        .map(|pair| (head, pair))
        .or_else(|| {
            head.checked_sub(1)
                .and_then(|prev| doc.find_matching_bracket(prev).map(|pair| (prev, pair)))
        });
    if let Some((cur, (open, close))) = found {
        let target = if cur == open { close } else { open };
        doc.selection = Selection::point(target);
        doc.preferred_col = None;
    }
}

fn point(doc: &mut Document, index: usize, extend: bool) {
    let head = index.min(doc.char_count());
    let anchor = if extend {
        doc.selection.primary_range().anchor
    } else {
        head
    };
    doc.selection = Selection::single(Range { anchor, head });
    doc.preferred_col = None;
}
