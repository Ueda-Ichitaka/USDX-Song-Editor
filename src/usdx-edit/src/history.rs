//! The undo history of the edit operations.
//!
//! About me: this module keeps the undo and redo stacks of applied patches
//! and the saved position. A commit applies the patch and pushes it onto the
//! undo stack; undo applies the patch's inverse and moves it to the redo
//! stack; redo applies it again. The saved position is the undo stack length
//! at `mark_saved`; a commit made after undoing past it moves the saved file
//! off the current branch and remembers how many steps away it lies, so
//! `is_dirty` and `unsaved_steps` compare the current history position
//! against the saved file. It exists so the UI's undo and redo commands and
//! the unsaved-changes guard work on patches instead of document snapshots.

use usdx_format::rows::Document;

use crate::patch::{EditError, Patch};

/// The undo and redo history of one document.
#[derive(Debug, Default)]
pub struct History {
    /// The applied patches in commit order; the last one is the next undo.
    undo_stack: Vec<Patch>,
    /// The undone patches; the last one is the next redo. Emptied by a new
    /// commit.
    redo_stack: Vec<Patch>,
    /// Where the saved file lies relative to the current branch: the undo
    /// stack length of the last state the current branch shares with the
    /// saved file.
    saved: usize,
    /// The steps from that shared state to the saved file along a branch the
    /// history no longer holds; 0 while the saved file is on the current
    /// branch. A commit after undoing past the saved step discards that
    /// branch, so the saved file stays this many steps away.
    saved_detour: usize,
}

impl History {
    /// new builds an empty history for a freshly loaded document.
    pub fn new() -> History {
        History::default()
    }

    /// commit applies the patch and records it. A refused patch leaves the
    /// document and the history unchanged. A commit clears the redo stack,
    /// because redoing over a new edit would build a different document than
    /// the one the user saw.
    pub fn commit(&mut self, doc: &mut Document, patch: Patch) -> Result<(), EditError> {
        patch.apply(doc)?;
        // Below the saved position the commit starts a new branch: the saved
        // file moves off the current branch to the steps between here and it.
        let position = self.undo_stack.len();
        if position < self.saved {
            self.saved_detour += self.saved - position;
            self.saved = position;
        }
        self.undo_stack.push(patch);
        self.redo_stack.clear();
        Ok(())
    }

    /// undo takes the last patch back and returns its label, or `None` when
    /// the history is empty. The undone patch stays available for redo.
    pub fn undo(&mut self, doc: &mut Document) -> Option<String> {
        let patch = self.undo_stack.pop()?;
        match patch.inverse().apply(doc) {
            Ok(()) => {
                let label = patch.label.clone();
                self.redo_stack.push(patch);
                Some(label)
            }
            // The history is LIFO, so the inverse of the last patch always
            // fits; if it ever does not, the patch goes back and the document
            // stays as it was.
            Err(_) => {
                self.undo_stack.push(patch);
                None
            }
        }
    }

    /// redo applies the last undone patch again and returns its label, or
    /// `None` when there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Option<String> {
        let patch = self.redo_stack.pop()?;
        match patch.apply(doc) {
            Ok(()) => {
                let label = patch.label.clone();
                self.undo_stack.push(patch);
                Some(label)
            }
            Err(_) => {
                self.redo_stack.push(patch);
                None
            }
        }
    }

    /// mark_saved records the current history position as the saved file.
    pub fn mark_saved(&mut self) {
        self.saved = self.undo_stack.len();
        self.saved_detour = 0;
    }

    /// is_dirty tells whether the current history position differs from the
    /// saved one; undoing past the saved step dirties the document again,
    /// because it no longer matches the saved file, and so does any position
    /// after a new edit made there.
    pub fn is_dirty(&self) -> bool {
        self.unsaved_steps() != 0
    }

    /// unsaved_steps returns how many edit steps the current history
    /// position is away from the saved one; the UI uses it for the "N
    /// unsaved changes" pill and dialog texts.
    pub fn unsaved_steps(&self) -> usize {
        self.undo_stack.len().abs_diff(self.saved) + self.saved_detour
    }

    /// undo_label returns the label of the step `undo` would perform.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo_stack.last().map(|patch| patch.label.as_str())
    }

    /// redo_label returns the label of the step `redo` would perform.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo_stack.last().map(|patch| patch.label.as_str())
    }
}
