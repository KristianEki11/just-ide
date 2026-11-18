use crate::state::{AppState, Command, EditorCmd, SidebarView};
use egui::{Event, Key};
use jide_core::MoveOp;

pub fn commands(ctx: &egui::Context, state: &AppState, focused: bool, page: usize) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut suppress_text = false;
    let chord_active = state.chord_ctrl_k;

    for event in ctx.input(|input| input.events.clone()) {
        match event {
            Event::Text(text) if focused && !suppress_text && !chord_active => {
                commands.push(Command::Editor(EditorCmd::Insert(text)));
            }
            Event::Copy if focused => commands.push(Command::Editor(EditorCmd::Copy)),
            Event::Cut if focused => commands.push(Command::Editor(EditorCmd::Cut)),
            Event::Paste(_) if focused => commands.push(Command::Editor(EditorCmd::Paste)),
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let ctrl = modifiers.command;
                let shift = modifiers.shift;
                let alt = modifiers.alt;

                // On Windows/European keyboards, AltGr sets command (Ctrl) + alt together;
                // let Event::Text handle typed symbols while keeping Ctrl+Alt+Up/Down multi-cursor.
                if ctrl && alt && !matches!(key, Key::ArrowUp | Key::ArrowDown) {
                    continue;
                }

                if chord_active {
                    suppress_text = true;
                    commands.push(Command::CancelChord);
                    if let Some(cmd) = ctrl_k_chord(key, modifiers, focused) {
                        commands.push(cmd);
                    }
                    continue;
                }

                if let Some(cmd) = global_shortcut(key, ctrl, shift, alt) {
                    commands.push(cmd);
                    continue;
                }
                if !focused {
                    continue;
                }
                if let Some(cmd) = edit_shortcut(key, ctrl, shift, alt) {
                    commands.push(Command::Editor(cmd));
                    continue;
                }
                if !alt {
                    if let Some(op) = movement_shortcut(key, ctrl, page) {
                        commands.push(Command::Editor(EditorCmd::Move { op, extend: shift }));
                    }
                }
            }
            _ => {}
        }
    }
    commands
}

fn ctrl_k_chord(key: Key, modifiers: egui::Modifiers, focused: bool) -> Option<Command> {
    match (key, modifiers.command, modifiers.shift, modifiers.alt) {
        (Key::O, true, false, false) => Some(Command::OpenFolder),
        (Key::T, true, false, false) => Some(Command::OpenThemePicker),
        (Key::Z, false, false, false) => Some(Command::ToggleZenMode),
        (Key::W, true | false, false, false) => Some(Command::CloseAllTabs),
        (Key::S, true, false, false) => Some(Command::OpenSettings),
        (Key::S, false, false, false) => Some(Command::SaveAll),
        (Key::U, true, false, false) if focused => Some(Command::Editor(EditorCmd::TransformUpper)),
        (Key::L, true, false, false) if focused => Some(Command::Editor(EditorCmd::TransformLower)),
        (Key::X, true, false, false) if focused => {
            Some(Command::Editor(EditorCmd::TrimTrailingWhitespace))
        }
        _ => None,
    }
}

fn global_shortcut(key: Key, ctrl: bool, shift: bool, alt: bool) -> Option<Command> {
    match (key, ctrl, shift, alt) {
        (Key::K, true, false, false) => Some(Command::BeginChordCtrlK),
        (Key::N, true, false, false) => Some(Command::NewFile),
        (Key::O, true, false, false) => Some(Command::OpenFileDialog),
        (Key::S, true, false, false) => Some(Command::SaveActive),
        (Key::S, true, true, false) => Some(Command::SaveAsDialog),
        (Key::W, true, false, false) => Some(Command::CloseActiveTab),
        (Key::T, true, true, false) => Some(Command::ReopenClosedTab),
        (Key::Q, true, false, false) => Some(Command::QuitApp),
        (Key::Equals | Key::Plus, true, _, false) => Some(Command::ZoomIn),
        (Key::Minus, true, false, false) => Some(Command::ZoomOut),
        (Key::Num0, true, false, false) => Some(Command::ResetZoom),
        (Key::Z, false, false, true) => Some(Command::ToggleWordWrap),
        (Key::F, false, true, true) => Some(Command::FormatDocument),
        (Key::P, true, false, false) => Some(Command::OpenQuickOpen),
        (Key::P, true, true, false) | (Key::F1, false, false, false) => {
            Some(Command::OpenCommandPalette)
        }
        (Key::G, true, false, false) => Some(Command::RunById("__goto_line__".into())),
        (Key::B, true, false, false) => Some(Command::ToggleSidebar),
        (Key::E, true, true, false) => Some(Command::ActivateSidebar(SidebarView::Explorer)),
        (Key::Comma, true, false, false) => Some(Command::OpenSettings),
        (Key::Backslash, true, false, false) => Some(Command::ToggleSplitEditor),
        (Key::Num1, true, false, false) => Some(Command::FocusGroup(0)),
        (Key::Num2, true, false, false) => Some(Command::FocusGroup(1)),
        (Key::PageUp, true, false, false) => Some(Command::PrevTab),
        (Key::PageDown, true, false, false) => Some(Command::NextTab),
        (Key::ArrowLeft, false, false, true) => Some(Command::NavBack),
        (Key::ArrowRight, false, false, true) => Some(Command::NavForward),
        (Key::F, true, false, false) => Some(Command::OpenFind { replace: false }),
        (Key::H, true, false, false) => Some(Command::OpenFind { replace: true }),
        (Key::F, true, true, false) => Some(Command::OpenFindInFiles { replace: false }),
        (Key::H, true, true, false) => Some(Command::OpenFindInFiles { replace: true }),
        (Key::F3, false, false, false) => Some(Command::FindNext),
        (Key::F3, false, true, false) => Some(Command::FindPrev),
        (Key::J, true, false, false) => Some(Command::ToggleBottomSearchPanel),
        (Key::Tab, true, false, false) => Some(Command::MruNext),
        (Key::Tab, true, true, false) => Some(Command::MruPrev),
        _ => None,
    }
}

fn edit_shortcut(key: Key, ctrl: bool, shift: bool, alt: bool) -> Option<EditorCmd> {
    match (key, ctrl, shift, alt) {
        (Key::Z, true, false, false) => Some(EditorCmd::Undo),
        (Key::Z, true, true, false) | (Key::Y, true, _, false) => Some(EditorCmd::Redo),
        (Key::A, true, false, false) => Some(EditorCmd::SelectAll),
        (Key::K, true, true, false) => Some(EditorCmd::DeleteLine),
        (Key::Slash, true, false, false) => Some(EditorCmd::ToggleLineComment),
        (Key::A, false, true, true) => Some(EditorCmd::ToggleBlockComment),
        (Key::D, true, false, false) => Some(EditorCmd::AddNextOccurrence),
        (Key::L, true, true, false) => Some(EditorCmd::SelectAllOccurrences),
        (Key::L, true, false, false) => Some(EditorCmd::ExpandLineSelection),
        (Key::Enter, true, false, false) => Some(EditorCmd::InsertLineBelow),
        (Key::Enter, true, true, false) => Some(EditorCmd::InsertLineAbove),
        (Key::Backslash, true, true, false) => Some(EditorCmd::JumpToMatchingBracket),
        (Key::CloseBracket, true, false, false) | (Key::Tab, false, false, false) => {
            Some(EditorCmd::Indent)
        }
        (Key::OpenBracket, true, false, false) | (Key::Tab, false, true, false) => {
            Some(EditorCmd::Outdent)
        }
        (Key::F9, false, false, false) => Some(EditorCmd::SortLines {
            descending: false,
            unique: false,
        }),
        (Key::ArrowUp, true, false, true) => Some(EditorCmd::AddCursorAbove),
        (Key::ArrowDown, true, false, true) => Some(EditorCmd::AddCursorBelow),
        (Key::ArrowUp, false, false, true) => Some(EditorCmd::MoveLineUp),
        (Key::ArrowDown, false, false, true) => Some(EditorCmd::MoveLineDown),
        (Key::ArrowUp, false, true, true) => Some(EditorCmd::DuplicateLineUp),
        (Key::ArrowDown, false, true, true) => Some(EditorCmd::DuplicateLineDown),
        (Key::Backspace, false, _, false) => Some(EditorCmd::Backspace),
        (Key::Delete, false, _, false) => Some(EditorCmd::Delete),
        (Key::Enter, false, _, false) => Some(EditorCmd::Newline),
        _ => None,
    }
}

fn movement_shortcut(key: Key, ctrl: bool, page: usize) -> Option<MoveOp> {
    match key {
        Key::ArrowLeft => Some(if ctrl { MoveOp::WordLeft } else { MoveOp::Left }),
        Key::ArrowRight => Some(if ctrl {
            MoveOp::WordRight
        } else {
            MoveOp::Right
        }),
        Key::ArrowUp => Some(MoveOp::Up),
        Key::ArrowDown => Some(MoveOp::Down),
        Key::Home => Some(if ctrl {
            MoveOp::DocStart
        } else {
            MoveOp::LineStartSmart
        }),
        Key::End => Some(if ctrl {
            MoveOp::DocEnd
        } else {
            MoveOp::LineEnd
        }),
        Key::PageUp => Some(MoveOp::PageUp(page)),
        Key::PageDown => Some(MoveOp::PageDown(page)),
        _ => None,
    }
}
