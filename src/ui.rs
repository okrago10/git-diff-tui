use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// 制御文字を可視のプレースホルダに置き換えてから表示する。
///
/// 端末に出す文字列のうち外から来たもの（git2 が返すファイル名やエラー
/// メッセージ）は、ほぼ生バイト列のまま渡ってくる。ESC/CSI などの制御
/// シーケンスが仕込まれていると、`Span::raw` 経由でそのまま端末に書き込まれ、
/// カーソル移動や配色変更を注入されうる（端末エスケープ・インジェクション）。
/// タブ・改行も含む制御文字を U+FFFD に置換して無害化する。
fn sanitize_for_display(text: &str) -> String {
    text.chars()
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

    // 実際に diff が見えるのは枠線の内側。枠の付け方を変えても追随するよう
    // Block 自身に内側の領域を計算させる。
    let inner = block.inner(area);
    app.viewport.set_visible_size(inner.width, inner.height);

    // 読み取りに失敗しているなら、その事実を「変更なし」より先に伝える。
    if let Some(message) = &app.last_error {
        let paragraph = Paragraph::new(format!("Error: {}", sanitize_for_display(message)))
            .block(block)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(paragraph, area);
        return;
    }

    if app.highlighted_diff.is_empty() {
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

    let offset = app.viewport.offset();
    // 文字列は App が保持しているものをそのまま借りる。毎フレーム複製すると
    // diff が長いほど描画が重くなる。
    let lines: Vec<Line> = app
        .highlighted_diff
        .iter()
        .map(|hl| {
            Line::from(
                hl.spans
                    .iter()
                    .map(|(style, text)| Span::styled(text.as_str(), *style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block).scroll(offset);

    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // 左側は状況によって変わるが、キーの案内は常に出す。
    let mut spans = match &app.last_error {
        Some(message) => vec![
            Span::styled(
                " ERROR ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", shorten(&sanitize_for_display(message))),
                Style::default().fg(Color::Red),
            ),
        ],
        None => {
            let file_count = app.files.len();
            let staged = app
                .files
                .iter()
                .filter(|f| f.stage == crate::git::Stage::Staged)
                .count();
            let unstaged = file_count - staged;
            vec![Span::styled(
                format!(" {file_count} files ({staged} staged, {unstaged} unstaged)"),
                Style::default().fg(Color::Cyan),
            )]
        }
    };

    spans.push(Span::raw("  │  "));
    spans.extend(key_hints(if app.last_error.is_some() {
        &RECOVERY_HINTS
    } else {
        &ALL_HINTS
    }));

    frame.render_widget(status_bar(Line::from(spans)), area);
}

const ALL_HINTS: [(&str, &str); 5] = [
    ("j/k", ": select  "),
    ("J/K", ": scroll  "),
    ("h/l", ": h-scroll  "),
    ("r", ": refresh  "),
    ("q", ": quit"),
];

/// エラー時はメッセージに場所を譲り、復旧に要るものだけ残す。`r` の案内が
/// 必要になるのは、まさにエラーが出ている場面であるため消さない。
const RECOVERY_HINTS: [(&str, &str); 2] = [("r", ": refresh  "), ("q", ": quit")];

fn key_hints(hints: &[(&'static str, &'static str)]) -> Vec<Span<'static>> {
    hints
        .iter()
        .copied()
        .flat_map(|(key, label)| {
            [
                Span::styled(
                    key,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(label, Style::default().fg(Color::DarkGray)),
            ]
        })
        .collect()
}

/// ステータスバーは 1 行しかないので、長いメッセージは切り詰める。
/// 全文は diff パネル側に出る。
fn shorten(text: &str) -> String {
    const MAX_CHARS: usize = 48;
    let mut shortened: String = text.chars().take(MAX_CHARS).collect();
    if text.chars().count() > MAX_CHARS {
        shortened.push('…');
    }
    shortened
}

/// ステータスバー 1 行ぶんの見た目。正常時もエラー時も同じ地色で描く。
fn status_bar(content: Line<'_>) -> Paragraph<'_> {
    Paragraph::new(content).style(Style::default().bg(Color::Rgb(30, 30, 30)))
}

#[cfg(test)]
mod tests {
    use super::{draw, sanitize_for_display};
    use crate::app::App;
    use crate::test_support::{ScriptedSource, error};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const SCREEN_WIDTH: u16 = 100;
    const SCREEN_HEIGHT: u16 = 20;

    /// 画面を 1 行ずつ文字列にして返す。
    fn rendered_lines(app: &mut App) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(SCREEN_WIDTH, SCREEN_HEIGHT)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(SCREEN_WIDTH as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect()
    }

    /// 画面に描かれた文字をすべて連結する。
    fn rendered(app: &mut App) -> String {
        rendered_lines(app).join("")
    }

    fn failing_app() -> App {
        App::new(ScriptedSource::new().then_files(Err(error("index is corrupt"))))
    }

    /// git の読み取りに失敗したとき、画面は「変更なし」ではなく
    /// 失敗した事実を示す。両者が同じ表示だと、ツールが嘘をつく。
    #[test]
    fn shows_error_instead_of_no_changes() {
        let mut app = failing_app();

        let screen = rendered(&mut app);

        assert!(screen.contains("index is corrupt"), "画面: {screen}");
        assert!(!screen.contains("No changes detected"), "画面: {screen}");
    }

    /// ステータスバーにも失敗を出す。diff パネルから目を離していても
    /// 「いま表示しているものが古い / 空である」理由が分かる。
    #[test]
    fn status_bar_shows_error() {
        let mut app = failing_app();

        let lines = rendered_lines(&mut app);
        let status = lines.last().unwrap();

        assert!(
            status.contains("index is corrupt"),
            "ステータスバー: {status}"
        );
    }

    /// エラー時もキー操作の案内は残す。復旧手段である `r` の案内が最も
    /// 必要になるのはエラーが出ている場面で、そこで消えると打つ手が
    /// 分からなくなる。
    #[test]
    fn status_bar_keeps_key_hints_on_error() {
        let mut app = failing_app();

        let lines = rendered_lines(&mut app);
        let status = lines.last().unwrap();

        assert!(status.contains("refresh"), "ステータスバー: {status}");
        assert!(status.contains("quit"), "ステータスバー: {status}");
    }

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
