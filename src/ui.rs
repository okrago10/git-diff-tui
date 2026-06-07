use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// 制御文字を可視のプレースホルダに置き換えてから表示する。
///
/// git2 はファイル名をほぼ生バイト列のまま返すため、悪意あるリポジトリが
/// ファイル名に ESC/CSI などの制御シーケンスを仕込んでいると、`Span::raw`
/// 経由でそのまま端末に書き込まれ、カーソル移動や配色変更を注入されうる
/// （端末エスケープ・インジェクション）。タブ・改行も含む制御文字を
/// U+FFFD に置換して無害化する。
fn sanitize_for_display(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    draw_file_list(frame, app, panels[0]);
    draw_diff_preview(frame, app, panels[1]);
    draw_status_bar(frame, app, status_area);
}

fn draw_file_list(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = app
        .files
        .iter()
        .map(|entry| {
            let icon_color = match entry.stage {
                crate::git::Stage::Staged => Color::Green,
                crate::git::Stage::Unstaged => Color::Yellow,
            };
            let kind_color = match entry.kind {
                crate::git::ChangeKind::Added | crate::git::ChangeKind::Untracked => Color::Green,
                crate::git::ChangeKind::Deleted => Color::Red,
                crate::git::ChangeKind::Modified => Color::Yellow,
                crate::git::ChangeKind::Renamed | crate::git::ChangeKind::Copied => Color::Cyan,
                crate::git::ChangeKind::Typechange => Color::Magenta,
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", entry.stage.icon()),
                    Style::default().fg(icon_color),
                ),
                Span::styled(
                    format!("{} ", entry.kind.label()),
                    Style::default().fg(kind_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(sanitize_for_display(&entry.path)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Files "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_diff_preview(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Diff ");

    if app.current_diff.is_empty() {
        let msg = if app.files.is_empty() {
            "No changes detected"
        } else {
            "Select a file to view diff"
        };
        let paragraph = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let highlighted = app
        .highlighter
        .highlight_diff(&app.current_diff, app.selected_file_path());

    let lines: Vec<Line> = highlighted
        .iter()
        .map(|hl| {
            Line::from(
                hl.spans
                    .iter()
                    .map(|(style, text)| Span::styled(text.clone(), *style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.diff_scroll, app.diff_hscroll));

    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let file_count = app.files.len();
    let staged = app
        .files
        .iter()
        .filter(|f| f.stage == crate::git::Stage::Staged)
        .count();
    let unstaged = file_count - staged;

    let status = Line::from(vec![
        Span::styled(
            format!(" {file_count} files ({staged} staged, {unstaged} unstaged)"),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  │  "),
        Span::styled(
            "j/k",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": select  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "J/K",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": scroll  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "h/l",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": h-scroll  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "r",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": refresh  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": quit", Style::default().fg(Color::DarkGray)),
    ]);

    let bar = Paragraph::new(status).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::sanitize_for_display;

    #[test]
    fn keeps_normal_paths_unchanged() {
        assert_eq!(sanitize_for_display("src/ui.rs"), "src/ui.rs");
        assert_eq!(sanitize_for_display("ディレクトリ/ファイル.rs"), "ディレクトリ/ファイル.rs");
    }

    #[test]
    fn replaces_escape_and_control_chars() {
        // ESC[31m のような色変更シーケンスや改行・タブを無害化する
        assert_eq!(sanitize_for_display("a\x1b[31mb"), "a\u{FFFD}[31mb");
        assert_eq!(sanitize_for_display("a\nb\tc"), "a\u{FFFD}b\u{FFFD}c");
        assert_eq!(sanitize_for_display("a\x7fb"), "a\u{FFFD}b");
    }
}
