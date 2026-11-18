//! Undo and redo history tracking with keystroke grouping heuristics.

use crate::types::{Change, EditKind, Selection, Transaction};

/// Maximum elapsed time in milliseconds between consecutive character insertions
/// for them to be merged into a single undo transaction.
pub const UNDO_MERGE_TIMEOUT_MS: u64 = 750;

/// Unlimited session undo and redo transaction stacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// Transactions available to be undone.
    pub undo_stack: Vec<Transaction>,
    /// Transactions available to be redone.
    pub redo_stack: Vec<Transaction>,
    /// Corresponding inverse changes for each transaction on the undo stack.
    undo_inverses: Vec<Vec<Change>>,
    /// Corresponding inverse changes for each transaction on the redo stack.
    redo_inverses: Vec<Vec<Change>>,
}

impl History {
    /// Creates a new empty history.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_inverses: Vec::new(),
            redo_inverses: Vec::new(),
        }
    }

    /// Pushes a new transaction into history with an automatic inverse, clearing the redo stack.
    pub fn push(&mut self, tx: Transaction) {
        let inverse = tx
            .changes
            .iter()
            .map(|c| Change {
                start: c.start,
                end: c.start + c.text.chars().count(),
                text: String::new(),
            })
            .collect();
        self.push_with_inverse(tx, inverse);
    }

    /// Pushes a transaction together with its explicit inverse changes.
    pub fn push_with_inverse(&mut self, tx: Transaction, inverse: Vec<Change>) {
        self.redo_stack.clear();
        self.redo_inverses.clear();

        if tx.kind == EditKind::InsertChar {
            if let Some(prev) = self.undo_stack.last_mut() {
                if prev.kind == EditKind::InsertChar
                    && tx.timestamp_ms.saturating_sub(prev.timestamp_ms) <= UNDO_MERGE_TIMEOUT_MS
                    && is_single_caret(&prev.before)
                    && is_single_caret(&prev.after)
                    && is_single_caret(&tx.before)
                    && is_single_caret(&tx.after)
                    && tx.before == prev.after
                    && prev.changes.len() == 1
                    && tx.changes.len() == 1
                {
                    let prev_change = &prev.changes[0];
                    let tx_change = &tx.changes[0];

                    if prev_change.start == prev_change.end
                        && tx_change.start == tx_change.end
                        && tx_change.start == prev_change.start + prev_change.text.chars().count()
                    {
                        prev.changes[0].text.push_str(&tx_change.text);
                        prev.after = tx.after;
                        prev.timestamp_ms = tx.timestamp_ms;

                        if let Some(prev_inv) = self.undo_inverses.last_mut() {
                            if let Some(inv_change) = prev_inv.first_mut() {
                                inv_change.end =
                                    prev.changes[0].start + prev.changes[0].text.chars().count();
                            }
                        }
                        return;
                    }
                }
            }
        }

        self.undo_stack.push(tx);
        self.undo_inverses.push(inverse);
    }

    /// Pops the latest transaction and its inverse from the undo stack and moves them to redo.
    pub fn pop_undo_entry(&mut self) -> Option<(Transaction, Vec<Change>)> {
        let tx = self.undo_stack.pop()?;
        let inv = self.undo_inverses.pop().unwrap_or_default();
        self.redo_stack.push(tx.clone());
        self.redo_inverses.push(inv.clone());
        Some((tx, inv))
    }

    /// Pops the latest transaction and its inverse from the redo stack and moves them to undo.
    pub fn pop_redo_entry(&mut self) -> Option<(Transaction, Vec<Change>)> {
        let tx = self.redo_stack.pop()?;
        let inv = self.redo_inverses.pop().unwrap_or_default();
        self.undo_stack.push(tx.clone());
        self.undo_inverses.push(inv.clone());
        Some((tx, inv))
    }

    /// Pops the latest transaction from the undo stack and moves it to the redo stack.
    pub fn pop_undo(&mut self) -> Option<Transaction> {
        self.pop_undo_entry().map(|(tx, _)| tx)
    }

    /// Pops the latest transaction from the redo stack and moves it to the undo stack.
    pub fn pop_redo(&mut self) -> Option<Transaction> {
        self.pop_redo_entry().map(|(tx, _)| tx)
    }

    /// Returns `true` if there are transactions that can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are transactions that can be redone.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the number of transactions on the undo stack.
    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of transactions on the redo stack.
    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clears both undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.undo_inverses.clear();
        self.redo_inverses.clear();
    }
}

/// Helper function to check if a selection is a single point caret.
fn is_single_caret(selection: &Selection) -> bool {
    selection.ranges.len() == 1 && selection.ranges[0].is_empty()
}
