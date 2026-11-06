use super::*;
use crate::buffer::LineBuffer;
use crate::completer::Candidate;
use crate::keymap::{EditorAction, KeyCombo};

    #[test]
    fn test_editor_creation_default() {
        let editor = Editor::new();
        assert!(
            editor
                .keymap()
                .contains((KeyCode::Enter, KeyModifiers::NONE))
        );
    }

    #[test]
    fn test_editor_with_custom_writer_and_keymap() {
        let buffer = Vec::new();
        let mut keymap = Keymap::empty();
        keymap.bind(KeyCombo::ctrl('q'), EditorAction::Interrupt);

        let mut editor = Editor::new_with_writer(buffer).with_keymap(keymap);
        assert!(editor.keymap().contains(KeyCombo::ctrl('q')));
        assert!(!editor.keymap().contains(KeyCode::Enter));

        editor
            .keymap_mut()
            .bind(KeyCode::Enter, EditorAction::Submit);
        assert!(editor.keymap().contains(KeyCode::Enter));
    }

    #[test]
    fn test_render_line_to_buffer() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        let line_buf = LineBuffer::with_content("echo 1".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 8); // "> " (2) + "echo 1" (6)
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_line_with_formatter() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_formatter(|s: &str| format!("***{}***", s));
        let line_buf = LineBuffer::with_content("secret".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("***secret***"));
    }

    #[test]
    fn test_render_multiline() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        let line_buf = LineBuffer::with_content("first\nsecond".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Should contain continuation indicator "… "
        assert!(out.contains("… "));
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.end_row, 1);
    }

    #[test]
    fn test_render_line_with_soft_wrap_override() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_terminal_width(10);
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("123456789"); // 2 + 9 = 11 cols in 10-col terminal

        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 1);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);
    }

    #[test]
    fn test_render_line_with_ghost_text() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_hinter(|line: &str, cursor: usize| {
                if line == "git c" && cursor == 5 {
                    Some("heckout".to_string())
                } else {
                    None
                }
            });

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("git c");

        let (layout, hint) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(hint.as_deref(), Some("heckout"));
        // Prompt is "> " (2) + "git c" (5) = 7
        assert_eq!(layout.cursor_col, 7);
        assert_eq!(layout.cursor_row, 0);
        // Total printed text is "> git checkout" (2 + 12 = 14)
        assert_eq!(layout.end_col, 14);
        assert_eq!(layout.end_row, 0);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("heckout"));
    }

    #[test]
    fn test_render_line_suppresses_ghost_text_when_disabled_or_vi_normal() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_hinter(|line: &str, cursor: usize| {
                if line == "git c" && cursor == 5 {
                    Some("heckout".to_string())
                } else {
                    None
                }
            });

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("git c");

        // 1. When show_ghost_text is false (e.g. on submit or interrupt), ghost text is suppressed
        let (layout, hint) = editor
            .render_line_internal(&"> ", &line_buf, 0, false)
            .unwrap();
        assert_eq!(hint, None);
        assert_eq!(layout.cursor_col, 7);
        assert_eq!(layout.end_col, 7);

        // 2. In Vi Normal mode, ghost text is also suppressed even when show_ghost_text is true
        editor.set_edit_mode(EditMode::Vi);
        editor.vi_mode = ViMode::Normal;
        let (layout_vi, hint_vi) = editor
            .render_line_internal(&"> ", &line_buf, 0, true)
            .unwrap();
        assert_eq!(hint_vi, None);
        assert_eq!(layout_vi.end_col, 7);
    }

    #[test]
    fn test_normalise_paste_crlf_and_cr() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_normalise_paste_preserves_tabs_and_strips_control_chars() {
        let input = "hello\tworld\x00\x1b[31mred\x1b[0m\x08test";
        let normalised = normalise_paste(input);
        // \x00, \x1b, and \x08 are stripped; \t is retained
        assert_eq!(normalised, "hello\tworld[31mred[0mtest");
    }

    #[test]
    fn test_normalise_paste_multiline_code_block() {
        let input = "def foo():\r\n    return 42\r\n";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "def foo():\n    return 42\n");
    }

    #[test]
    fn test_editor_with_completer_and_menu() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer)
            .with_completer(|line: &str, cursor: usize| {
                if line == "car" && cursor == 3 {
                    (0, vec![Candidate::new("cargo")])
                } else {
                    (cursor, Vec::new())
                }
            })
            .with_menu(crate::menu::BasicMenu::default());

        assert!(editor.completer().is_some());
        assert!(editor.menu().is_some());

        let (start, items) = editor.completer().unwrap().complete("car", 3);
        assert_eq!(start, 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].display, "cargo");

        editor.set_menu(None);
        assert!(editor.menu().is_none());
        editor.set_completer(None);
        assert!(editor.completer().is_none());
    }

    #[test]
    fn test_render_line_with_active_completion_menu() {
        let buffer = Vec::new();
        let mut menu = crate::menu::BasicMenu::default();

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("ca");

        let completer = |_line: &str, _cursor: usize| {
            (
                0,
                vec![
                    Candidate::new("cargo").with_description("Rust package manager"),
                    Candidate::new("cat").with_description("Concatenate files"),
                ],
            )
        };

        menu.activate(&mut line_buf, Some(&completer));
        assert!(menu.is_active());

        let mut editor = Editor::new_with_writer(buffer)
            .with_terminal_width(80)
            .with_hinter(|_line: &str, _cursor: usize| Some("ghost-hint".to_string()))
            .with_menu(menu);

        let (layout, hint) = editor.render_line(&"> ", &line_buf, 0).unwrap();

        // Ghost text MUST be suppressed when completion menu is active
        assert_eq!(hint, None);

        // Layout cursor is at row 0, col 4 ("> " (2) + "ca" (2))
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 4);

        // Layout end_row includes menu lines: 1 (buffer row 0) + 4 (top border + 2 items + bottom border) = 4
        assert_eq!(layout.end_row, 4);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Menu borders and items are drawn in the rendered output
        assert!(out.contains("┌"));
        assert!(out.contains("└"));
        assert!(out.contains("cargo"));
        assert!(out.contains("cat"));
        // Ghost text is NOT drawn
        assert!(!out.contains("ghost-hint"));
    }

    #[test]
    fn test_editor_history_builder() {
        let mut hist = crate::history::DefaultHistory::new(10);
        hist.append("echo 1");
        hist.append("echo 2");

        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_history(hist);
        assert!(editor.history().is_some());
        assert_eq!(editor.history().unwrap().len(), 2);
        assert_eq!(editor.history().unwrap().get(0).as_deref(), Some("echo 1"));
        assert_eq!(editor.history().unwrap().get(1).as_deref(), Some("echo 2"));

        editor.set_history(None);
        assert!(editor.history().is_none());
    }

    #[test]
    fn test_render_line_multiline_prompt_redraws_in_place() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_terminal_width(80);
        let prompt = "\n\x1b[1;34m~/dev/lsh\x1b[0m (main)\n\x1b[35m❯ \x1b[0m";

        let mut line_buf = LineBuffer::new();
        let (layout1, _) = editor.render_line(&prompt, &line_buf, 0).unwrap();
        assert_eq!(layout1.cursor_row, 2);
        assert_eq!(layout1.cursor_col, 2);

        // Render second keystroke with previous_cursor_row = 2
        line_buf.insert_char('c');
        let (layout2, _) = editor
            .render_line(&prompt, &line_buf, layout1.cursor_row)
            .unwrap();
        assert_eq!(layout2.cursor_row, 2);
        assert_eq!(layout2.cursor_col, 3);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Prompt newlines should be rendered with \r\n for raw mode compatibility
        assert!(out.contains("\r\n"));
        // Second render must emit MoveUp(2) to return to row 0 of the prompt
        assert!(out.contains("\x1b[2A"));
    }

    #[test]
    fn test_editor_clipboard_cut_and_yank() {
        use crate::clipboard::InMemoryClipboard;

        let buffer = Vec::new();
        let cb = InMemoryClipboard::new();
        let mut editor = Editor::new_with_writer(buffer).with_clipboard(cb);

        assert!(editor.clipboard().is_some());

        // Check mutable borrow and value storage
        editor.clipboard_mut().unwrap().set("copied text");
        assert_eq!(
            editor.clipboard_mut().unwrap().get().as_deref(),
            Some("copied text")
        );

        editor.set_clipboard(None);
        assert!(editor.clipboard().is_none());
    }

    #[test]
    fn test_editor_vi_mode_configuration_and_keymaps() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        assert_eq!(editor.edit_mode(), EditMode::Emacs);
        assert_eq!(editor.vi_mode(), ViMode::Insert);

        // Switch to Vi mode
        editor.set_edit_mode(EditMode::Vi);
        assert_eq!(editor.edit_mode(), EditMode::Vi);
        assert_eq!(editor.vi_mode(), ViMode::Insert);

        // In Vi insert mode, Esc is mapped to SwitchToNormalMode
        assert_eq!(
            editor.keymap().get(KeyCode::Esc, KeyModifiers::NONE),
            Some(&EditorAction::SwitchToNormalMode)
        );

        // Check normal mode keybindings
        let normal_km = Keymap::default_vi_normal();
        assert_eq!(
            normal_km.get(KeyCode::Char('i'), KeyModifiers::NONE),
            Some(&EditorAction::SwitchToInsertMode)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('a'), KeyModifiers::NONE),
            Some(&EditorAction::Append)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('A'), KeyModifiers::NONE),
            Some(&EditorAction::AppendEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('I'), KeyModifiers::NONE),
            Some(&EditorAction::InsertAtFirstNonWhitespace)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('o'), KeyModifiers::NONE),
            Some(&EditorAction::OpenBelow)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('O'), KeyModifiers::NONE),
            Some(&EditorAction::OpenAbove)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('s'), KeyModifiers::NONE),
            Some(&EditorAction::SubstituteChar)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('S'), KeyModifiers::NONE),
            Some(&EditorAction::SubstituteLine)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('C'), KeyModifiers::NONE),
            Some(&EditorAction::ChangeToEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('d'), KeyModifiers::NONE),
            Some(&EditorAction::ViDeleteOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('c'), KeyModifiers::NONE),
            Some(&EditorAction::ViChangeOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('y'), KeyModifiers::NONE),
            Some(&EditorAction::ViYankOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('r'), KeyModifiers::NONE),
            Some(&EditorAction::ViReplaceCharOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('x'), KeyModifiers::NONE),
            Some(&EditorAction::CutChar)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('X'), KeyModifiers::NONE),
            Some(&EditorAction::CutCharBefore)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('D'), KeyModifiers::NONE),
            Some(&EditorAction::CutToEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('~'), KeyModifiers::NONE),
            Some(&EditorAction::ToggleCase)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('u'), KeyModifiers::NONE),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('p'), KeyModifiers::NONE),
            Some(&EditorAction::PasteAfter)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('P'), KeyModifiers::NONE),
            Some(&EditorAction::PasteBefore)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('w'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordRight)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('b'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordLeft)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('e'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('0'), KeyModifiers::NONE),
            Some(&EditorAction::MoveHome)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('^'), KeyModifiers::NONE),
            Some(&EditorAction::MoveToFirstNonWhitespace)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('$'), KeyModifiers::NONE),
            Some(&EditorAction::MoveEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('u'), KeyModifiers::NONE),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('r'), KeyModifiers::CONTROL),
            Some(&EditorAction::Redo)
        );

        // Switch back to Emacs
        editor.set_edit_mode(EditMode::Emacs);
        assert_eq!(editor.edit_mode(), EditMode::Emacs);

        // Emacs keybindings for Undo and Redo
        assert_eq!(
            editor
                .keymap()
                .get(KeyCode::Char('_'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            editor
                .keymap()
                .get(KeyCode::Char('z'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            editor.keymap().get(KeyCode::Char('r'), KeyModifiers::ALT),
            Some(&EditorAction::Redo)
        );
    }

    #[test]
    fn test_editor_cursor_style_configuration() {
        let mut editor = Editor::new();
        assert_eq!(editor.cursor_style(), cursor::SetCursorStyle::BlinkingBar);

        editor.set_cursor_style(cursor::SetCursorStyle::SteadyBar);
        assert_eq!(editor.cursor_style(), cursor::SetCursorStyle::SteadyBar);

        let editor2 = Editor::new_with_writer(Vec::<u8>::new())
            .with_cursor_style(cursor::SetCursorStyle::SteadyBlock);
        assert_eq!(editor2.cursor_style(), cursor::SetCursorStyle::SteadyBlock);
    }

    #[test]
    fn test_signal_methods() {
        let success = Signal::Success("echo hello".to_string());
        assert!(success.is_success());
        assert!(!success.is_ctrl_c());
        assert!(!success.is_ctrl_d());
        assert_eq!(success.into_success(), Some("echo hello".to_string()));

        let ctrl_c = Signal::CtrlC;
        assert!(!ctrl_c.is_success());
        assert!(ctrl_c.is_ctrl_c());
        assert!(!ctrl_c.is_ctrl_d());
        assert_eq!(ctrl_c.into_success(), None);

        let ctrl_d = Signal::CtrlD;
        assert!(!ctrl_d.is_success());
        assert!(!ctrl_d.is_ctrl_c());
        assert!(ctrl_d.is_ctrl_d());
        assert_eq!(ctrl_d.into_success(), None);
    }
