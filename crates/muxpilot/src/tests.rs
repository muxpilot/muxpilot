use super::*;

const HOME: &str = "/home/user";

#[test]
fn tilde_collapses_leading_home() {
    assert_eq!(tilde("/home/user/gits/x", HOME), "~/gits/x");
    assert_eq!(tilde("/etc/passwd", HOME), "/etc/passwd");
}

#[test]
fn untilde_expands_leading_tilde() {
    assert_eq!(untilde("~/gits/x", HOME), "/home/user/gits/x");
    assert_eq!(untilde("/abs/path", HOME), "/abs/path");
}

#[test]
fn tilde_roundtrips() {
    let p = "/home/user/gits/github/org/repo";
    assert_eq!(untilde(&tilde(p, HOME), HOME), p);
}

#[test]
fn sanitize_replaces_dots_in_basename() {
    assert_eq!(sanitize_session_name("/home/a/my.project"), "my_project");
    assert_eq!(sanitize_session_name("/home/a/repo"), "repo");
    assert_eq!(sanitize_session_name("/home/a/repo/"), "repo");
}

#[test]
fn tmuxinator_name_skips_docs() {
    assert_eq!(
        tmuxinator_project_name("work.yml"),
        Some("work".to_string())
    );
    assert_eq!(tmuxinator_project_name("AGENTS.yml"), None);
    assert_eq!(tmuxinator_project_name("TEMPLATES.yml"), None);
    assert_eq!(tmuxinator_project_name("README.md"), None);
}

#[test]
fn parse_layout_name_reads_quoted_and_plain() {
    assert_eq!(
        parse_layout_name("name: payments-service\nroot: ~/x"),
        Some("payments-service".to_string())
    );
    assert_eq!(
        parse_layout_name("name: \"quoted\""),
        Some("quoted".to_string())
    );
    assert_eq!(
        parse_layout_name("name: 'single'"),
        Some("single".to_string())
    );
}

#[test]
fn parse_layout_name_skips_erb_and_missing() {
    assert_eq!(
        parse_layout_name("name: <%= File.basename(Dir.pwd) %>"),
        None
    );
    assert_eq!(parse_layout_name("root: ~/x\nwindows: []"), None);
    assert_eq!(parse_layout_name("name:"), None);
}

#[test]
fn parse_selection_layout_running_and_idle() {
    assert_eq!(
        parse_selection("🚀 payments-service · ~/gits/x ▶", HOME),
        Some(Selection::Layout {
            session: "payments-service".to_string(),
            full_path: "/home/user/gits/x".to_string(),
        })
    );
    assert_eq!(
        parse_selection("🚀 proj · ~/gits/y", HOME),
        Some(Selection::Layout {
            session: "proj".to_string(),
            full_path: "/home/user/gits/y".to_string(),
        })
    );
}

#[test]
fn parse_selection_sessions() {
    assert_eq!(
        parse_selection("📺 main", HOME),
        Some(Selection::Session("main".to_string()))
    );
    assert_eq!(
        parse_selection("🟢 here", HOME),
        Some(Selection::Session("here".to_string()))
    );
}

#[test]
fn parse_selection_project() {
    assert_eq!(
        parse_selection("🎬 work", HOME),
        Some(Selection::Project("work".to_string()))
    );
}

#[test]
fn parse_selection_dirs_with_and_without_config() {
    assert_eq!(
        parse_selection("⭐ ~/code 📄", HOME),
        Some(Selection::Dir {
            full_path: "/home/user/code".to_string(),
            has_local_config: true,
        })
    );
    assert_eq!(
        parse_selection("📁 ~/gits/github/org/repo", HOME),
        Some(Selection::Dir {
            full_path: "/home/user/gits/github/org/repo".to_string(),
            has_local_config: false,
        })
    );
}

#[test]
fn parse_selection_rejects_garbage() {
    assert_eq!(parse_selection("no emoji here", HOME), None);
    assert_eq!(parse_selection("", HOME), None);
}

#[test]
fn selection_roundtrips_through_render() {
    let model = MenuModel {
        layouts: vec![Layout {
            session: "payments-service".to_string(),
            display: "~/gits/github/acme/payments-service".to_string(),
            path: "/home/user/gits/github/acme/payments-service".to_string(),
            running: true,
        }],
        ..Default::default()
    };
    let line = &build_menu_lines(&model)[0];
    match parse_selection(line, HOME) {
        Some(Selection::Layout { full_path, .. }) => {
            assert_eq!(full_path, "/home/user/gits/github/acme/payments-service");
        }
        other => panic!("expected layout, got {other:?}"),
    }
}

#[test]
fn build_menu_orders_sections_and_marks_current() {
    let model = MenuModel {
        sessions: vec![
            SessionItem {
                name: "main".to_string(),
                windows: 3,
            },
            SessionItem {
                name: "side".to_string(),
                windows: 1,
            },
        ],
        current: "main".to_string(),
        layouts: vec![
            Layout {
                session: "zebra".to_string(),
                display: "~/z".to_string(),
                path: "/home/user/z".to_string(),
                running: false,
            },
            Layout {
                session: "alpha".to_string(),
                display: "~/a".to_string(),
                path: "/home/user/a".to_string(),
                running: true,
            },
        ],
        projects: vec!["glob".to_string()],
        zoxide: vec![DirItem {
            display: "~/dl".to_string(),
            path: "/home/user/dl".to_string(),
            has_local_config: true,
        }],
        plain_repos: vec![DirItem {
            display: "~/gits/github/o/r".to_string(),
            path: "/home/user/gits/github/o/r".to_string(),
            has_local_config: false,
        }],
    };
    let lines = build_menu_lines(&model);
    assert_eq!(
        lines,
        vec![
            "🟢 main · 3 windows".to_string(),
            "📺 side · 1 window".to_string(),
            "🚀 alpha · ~/a ▶".to_string(),
            "🚀 zebra · ~/z".to_string(),
            "🎬 glob".to_string(),
            "⭐ ~/dl 📄".to_string(),
            "📁 ~/gits/github/o/r".to_string(),
        ]
    );
}

#[test]
fn empty_model_renders_nothing() {
    assert!(build_menu_lines(&MenuModel::default()).is_empty());
}

#[test]
fn delete_filter_word_removes_last_token() {
    let mut filter = native_state::FilterInput::default();
    for ch in "agent window".chars() {
        filter.insert(ch);
    }
    filter.delete_word_before_cursor();
    assert_eq!(filter.text(), "agent ");

    filter.delete_word_before_cursor();
    assert_eq!(filter.text(), "");
}

#[test]
fn delete_filter_word_trims_trailing_spaces_and_handles_unicode() {
    let mut filter = native_state::FilterInput::default();
    for ch in "привет мир  ".chars() {
        filter.insert(ch);
    }
    filter.delete_word_before_cursor();
    assert_eq!(filter.text(), "привет ");

    let mut spaces = native_state::FilterInput::default();
    for ch in "   ".chars() {
        spaces.insert(ch);
    }
    spaces.delete_word_before_cursor();
    assert_eq!(spaces.text(), "");
}

#[test]
fn filter_cursor_supports_readline_movement() {
    let mut filter = native_state::FilterInput::default();
    for ch in "agent".chars() {
        filter.insert(ch);
    }
    filter.move_left();
    filter.move_left();
    filter.insert('-');
    assert_eq!(filter.text(), "age-nt");
    assert_eq!(filter.display_with_cursor(), "age-█nt");

    filter.move_start();
    filter.insert('!');
    assert_eq!(filter.text(), "!age-nt");

    filter.move_end();
    filter.backspace();
    assert_eq!(filter.text(), "!age-n");
}

#[test]
fn generated_help_mentions_every_key_binding() {
    let help = native_state::native_help_body().join("\n");
    for (key, _, _) in native_state::KEY_BINDINGS {
        assert!(help.contains(key), "missing key binding in help: {key}");
    }
}

#[test]
fn native_entries_merge_workspace_capabilities() {
    let model = MenuModel {
        sessions: vec![SessionItem {
            name: "payments-service".to_string(),
            windows: 2,
        }],
        current: "payments-service".to_string(),
        layouts: vec![Layout {
            session: "payments-service".to_string(),
            display: "~/gits/payments-service".to_string(),
            path: "/home/user/gits/payments-service".to_string(),
            running: true,
        }],
        zoxide: vec![DirItem {
            display: "~/gits/payments-service".to_string(),
            path: "/home/user/gits/payments-service".to_string(),
            has_local_config: true,
        }],
        ..Default::default()
    };
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "tmux",
        backend: "tmux",
        current_session: "payments-service".to_string(),
        current_window_id: "@1".to_string(),
        current_pane_id: "%1".to_string(),
        sessions: vec![TmuxSession {
            name: "payments-service".to_string(),
            windows: vec![TmuxWindow {
                id: "@1".to_string(),
                index: 0,
                name: "main".to_string(),
                active: true,
                last_activity: Some(100),
                panes: vec![TmuxPane {
                    id: "%1".to_string(),
                    active: true,
                    path: "/home/user/gits/payments-service".to_string(),
                    current_command: "zsh".to_string(),
                    pid: Some(123),
                    last_activity: Some(120),
                    role: String::new(),
                    agent: Some(AgentState {
                        kind: "codex".to_string(),
                        status: "busy".to_string(),
                        source: AgentStateSource::Process,
                        confidence: 80,
                        attention: false,
                        wait_reason: String::new(),
                        evidence: vec!["process".to_string()],
                    }),
                }],
            }],
        }],
    };

    let entries = build_native_entries(&model, &snapshot);
    let workspace = entries
        .iter()
        .find(|entry| entry.line.contains("payments-service"))
        .expect("payments-service workspace row");
    assert!(
        workspace.line.contains("◍"),
        "agent token: {}",
        workspace.line
    );
    assert!(
        workspace.line.contains("agent"),
        "agent status: {}",
        workspace.line
    );
    assert!(workspace.tags.contains(&"agent"));
}

#[test]
fn native_main_entries_hide_plain_recent_dirs_and_commands() {
    let model = MenuModel {
        zoxide: vec![
            DirItem {
                display: "~/configured".to_string(),
                path: "/home/user/configured".to_string(),
                has_local_config: true,
            },
            DirItem {
                display: "~/plain".to_string(),
                path: "/home/user/plain".to_string(),
                has_local_config: false,
            },
        ],
        ..Default::default()
    };
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "tmux",
        backend: "tmux",
        current_session: String::new(),
        current_window_id: String::new(),
        current_pane_id: String::new(),
        sessions: Vec::new(),
    };

    let entries = build_native_entries(&model, &snapshot);
    assert!(entries
        .iter()
        .any(|entry| entry.line.contains("configured")));
    assert!(!entries.iter().any(|entry| entry.line.contains("plain")));
    assert!(!entries.iter().any(|entry| entry.tags.contains(&"command")));
}

#[test]
fn native_main_entries_merge_configured_dir_with_layout_by_path() {
    let model = MenuModel {
        layouts: vec![Layout {
            session: "personal-productivity-app".to_string(),
            display: "~/gits/personal-productivity-app".to_string(),
            path: "/home/user/gits/personal-productivity-app".to_string(),
            running: false,
        }],
        zoxide: vec![DirItem {
            display: "~/gits/personal-productivity-app".to_string(),
            path: "/home/user/gits/personal-productivity-app".to_string(),
            has_local_config: true,
        }],
        ..Default::default()
    };
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "tmux",
        backend: "tmux",
        current_session: String::new(),
        current_window_id: String::new(),
        current_pane_id: String::new(),
        sessions: Vec::new(),
    };

    let entries = build_native_entries(&model, &snapshot);
    let matches = entries
        .iter()
        .filter(|entry| entry.line.contains("personal-productivity-app"))
        .count();
    assert_eq!(matches, 1);
}

#[test]
fn directory_picker_entries_include_plain_dirs() {
    let model = MenuModel {
        zoxide: vec![DirItem {
            display: "~/plain".to_string(),
            path: "/home/user/plain".to_string(),
            has_local_config: false,
        }],
        ..Default::default()
    };

    let entries = build_directory_entries(&model);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].line.contains("plain"));
    assert!(entries[0].line.contains("bare"));
    assert!(matches!(
        entries[0].action,
        native_state::NativeAction::Select(Selection::Dir {
            has_local_config: false,
            ..
        })
    ));
}

#[test]
fn directory_picker_entries_sort_by_display_name() {
    let model = MenuModel {
        zoxide: vec![
            DirItem {
                display: "~/zeta".to_string(),
                path: "/home/user/zeta".to_string(),
                has_local_config: false,
            },
            DirItem {
                display: "~/alpha".to_string(),
                path: "/home/user/alpha".to_string(),
                has_local_config: false,
            },
        ],
        ..Default::default()
    };

    let entries = build_directory_entries(&model);
    assert!(entries[0].line.contains("alpha"));
    assert!(entries[1].line.contains("zeta"));
}

#[test]
fn native_main_entries_sort_alphabetically_inside_group() {
    let model = MenuModel {
        sessions: vec![
            SessionItem {
                name: "zeta".to_string(),
                windows: 1,
            },
            SessionItem {
                name: "alpha".to_string(),
                windows: 1,
            },
        ],
        ..Default::default()
    };
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "tmux",
        backend: "tmux",
        current_session: String::new(),
        current_window_id: String::new(),
        current_pane_id: String::new(),
        sessions: Vec::new(),
    };

    let entries = build_native_entries(&model, &snapshot);
    let running: Vec<&NativeEntry> = entries
        .iter()
        .filter(|entry| entry.group == native_state::NativeGroup::Running)
        .collect();
    assert!(running[0].line.contains("alpha"));
    assert!(running[1].line.contains("zeta"));
}

#[test]
fn compact_entry_columns_use_spaces_before_activity() {
    let entry = native_state::NativeEntry::new(
        "● devservers · 󰆍 4 4 · active · 4h".to_string(),
        "Workspace".to_string(),
        native_state::NativeAction::Select(Selection::Session("devservers".to_string())),
        vec!["session"],
        native_state::NativeGroup::Running,
    );

    let rendered = entry_columns(&entry, 37);
    assert!(rendered.contains(" active"));
    assert!(!rendered.contains("· active"));
}

#[test]
fn entry_header_uses_same_width_as_entry_columns() {
    let entry = native_state::NativeEntry::new(
        "● devservers ·  4 4 · active · 4h".to_string(),
        "Workspace".to_string(),
        native_state::NativeAction::Select(Selection::Session("devservers".to_string())),
        vec!["session"],
        native_state::NativeGroup::Running,
    );
    let width = 72;

    assert_eq!(display_width(&entry_header(width)), width);
    assert_eq!(display_width(&entry_columns(&entry, width)), width);
}

#[test]
fn workspace_detail_includes_window_summaries() {
    let row = WorkspaceRow {
        name: "payments-service".to_string(),
        session: Some("payments-service".to_string()),
        windows: 2,
        panes: 3,
        window_details: vec![
            WindowSummary {
                index: 0,
                id: "@1".to_string(),
                name: "main".to_string(),
                active: true,
                panes: 2,
                agents: 1,
                last_activity: Some(100),
            },
            WindowSummary {
                index: 1,
                id: "@2".to_string(),
                name: "logs".to_string(),
                active: false,
                panes: 1,
                agents: 0,
                last_activity: None,
            },
        ],
        ..Default::default()
    };

    let detail = workspace_detail(&row);
    assert!(detail.contains("Windows"));
    assert!(detail.contains("  * 0:@1 main  2 󰚩1"));
    assert!(detail.contains("    1:@2 logs  1"));
}

#[test]
fn demo_generates_requested_count() {
    assert_eq!(build_demo_entries(0).len(), 0);
    assert_eq!(build_demo_entries(1).len(), 1);
    assert_eq!(build_demo_entries(500).len(), 500);
}

#[test]
fn demo_rows_never_shift_columns() {
    // Every rendered row (glyph prefix + column area) must be exactly the list
    // width, for a huge, deliberately overflow-heavy fake inventory across the
    // full range of terminal widths — this is the anti-shift guarantee.
    let entries = build_demo_entries(500);
    for &list_width in &[24usize, 32, 40, 46, 60, 80, 84, 120] {
        let content = list_width.saturating_sub(4);
        for entry in &entries {
            // State glyph is always a single cell (drawn in the fixed prefix).
            assert_eq!(display_width(&entry_glyph(entry).to_string()), 1);
            let cols = entry_columns(entry, content);
            assert_eq!(
                display_width(&cols),
                content,
                "width={list_width} line={:?}",
                entry.line
            );
        }
        // Header row planned from the same solver lines up with body rows.
        assert_eq!(display_width(&entry_header(content)), content);
    }
}

#[test]
fn demo_rows_truncate_long_names_with_ellipsis() {
    let entries = build_demo_entries(200);
    // The fixture guarantees at least one name far longer than any column.
    let long = entries
        .iter()
        .find(|e| e.line.contains("customer-support-chatbot-training-pipeline"))
        .expect("a very long demo name exists");
    let rendered = entry_columns(long, 30);
    assert_eq!(display_width(&rendered), 30);
    assert!(
        rendered.contains('…'),
        "long name should be clipped: {rendered}"
    );
}

#[test]
fn demo_filtering_is_correct() {
    let entries = build_demo_entries(500);

    // A query that exists narrows to only matching rows.
    let hits: Vec<_> = entries
        .iter()
        .filter(|e| entry_matches(e, "payments-service", native_state::SearchMode::All))
        .collect();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|e| e.search_text.contains("payments-service")));

    // A query that cannot match returns nothing.
    let none = entries
        .iter()
        .filter(|e| entry_matches(e, "zzzzz-no-such-workspace", native_state::SearchMode::All))
        .count();
    assert_eq!(none, 0);

    // The Agents scope only admits rows tagged as agents.
    let agents: Vec<_> = entries
        .iter()
        .filter(|e| entry_matches(e, "", native_state::SearchMode::Agents))
        .collect();
    assert!(!agents.is_empty());
    assert!(agents.iter().all(|e| e.tags.contains(&"agent")));

    // Multi-token filtering requires every token to be present.
    for e in &entries {
        let matched = entry_matches(e, "agent payments-service", native_state::SearchMode::All);
        assert_eq!(
            matched,
            e.search_text.contains("agent") && e.search_text.contains("payments-service")
        );
    }
}

#[test]
fn compact_picker_gives_small_screens_to_the_list() {
    assert!(picker_uses_compact_height(6));
    assert_eq!(picker_body_range(6), (1, 5));
    assert_eq!(picker_body_rows(6), 4);

    assert!(!picker_uses_compact_height(20));
    // Row 0 status bar, row 1 blank, last row footer; body fills the rest.
    assert_eq!(picker_body_range(20), (2, 19));
    assert_eq!(picker_body_rows(20), 17);
}


// --- session→windows tree expansion ---

fn tree_fixture() -> Vec<NativeEntry> {
    use crate::model::Selection;
    use native_state::{NativeAction, NativeGroup, WindowRow};
    let win = |id: &str, name: &str| WindowRow {
        index: 0,
        id: id.to_string(),
        name: name.to_string(),
        active: false,
        panes: 1,
        agents: 0,
        activity: "now".to_string(),
    };
    let session = NativeEntry::new(
        "● work · 2w · active · now".to_string(),
        "Workspace\nName: work".to_string(),
        NativeAction::Select(Selection::Session("work".to_string())),
        vec!["session", "window"],
        NativeGroup::Running,
    )
    .with_windows("work".to_string(), vec![win("@1", "editor"), win("@2", "server")]);
    let project = NativeEntry::new(
        "○ notes · · tmuxinator · -".to_string(),
        "Workspace\nName: notes".to_string(),
        NativeAction::Select(Selection::Project("notes".to_string())),
        vec!["project"],
        NativeGroup::Configured,
    );
    vec![session, project]
}

#[test]
fn tree_expansion_inserts_window_rows_and_toggles() {
    use crate::native_view::{apply_tree_key, selectable_rows, Selectable, TreeKey};
    use std::collections::HashSet;

    let entries = tree_fixture();
    let filtered: Vec<usize> = vec![0, 1];
    let mut expanded: HashSet<String> = HashSet::new();

    // Collapsed: only the two entries are navigable.
    assert_eq!(selectable_rows(&entries, &filtered, &expanded).len(), 2);

    // Expand the session at cursor 0 — cursor stays on the parent.
    let sel = selectable_rows(&entries, &filtered, &expanded);
    let cursor = apply_tree_key(TreeKey::Expand, &sel, &entries, &filtered, &mut expanded, 0);
    assert_eq!(cursor, 0);
    assert!(expanded.contains("work"));

    // Now the two window children are interleaved after the session.
    let sel = selectable_rows(&entries, &filtered, &expanded);
    assert_eq!(sel.len(), 4);
    assert!(matches!(sel[0], Selectable::Entry(0)));
    assert!(matches!(sel[1], Selectable::Window { pos: 0, win: 0 }));
    assert!(matches!(sel[2], Selectable::Window { pos: 0, win: 1 }));
    assert!(matches!(sel[3], Selectable::Entry(1)));

    // Collapsing from a window child closes the parent and lands the cursor
    // back on it.
    let cursor = apply_tree_key(TreeKey::Collapse, &sel, &entries, &filtered, &mut expanded, 2);
    assert_eq!(cursor, 0);
    assert!(!expanded.contains("work"));
    assert_eq!(selectable_rows(&entries, &filtered, &expanded).len(), 2);
}

#[test]
fn tree_expand_is_noop_on_non_session_rows() {
    use crate::native_view::{apply_tree_key, selectable_rows, TreeKey};
    use std::collections::HashSet;

    let entries = tree_fixture();
    let filtered: Vec<usize> = vec![0, 1];
    let mut expanded: HashSet<String> = HashSet::new();
    let sel = selectable_rows(&entries, &filtered, &expanded);
    // Cursor 1 is the configured project — nothing to expand.
    let cursor = apply_tree_key(TreeKey::Expand, &sel, &entries, &filtered, &mut expanded, 1);
    assert_eq!(cursor, 1);
    assert!(expanded.is_empty());
}
