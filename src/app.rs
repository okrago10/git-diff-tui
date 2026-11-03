use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::file_list::FileList;
use crate::git::DiffSource;
use crate::highlight::{HighlightedLine, Highlighter};
use crate::viewport::DiffViewport;

/// `J` / `K` / `Ctrl+d` / `Ctrl+u` で動かす行数。
const PAGE_LINES: u16 = 10;
/// `h` / `l` で動かす桁数。
const STEP_COLUMNS: u16 = 4;

pub struct App {
    pub files: FileList,
    /// 表示中の diff にハイライトを適用した結果。描画のたびに計算し直すと
    /// 最大 10,000 行ぶんの syntect 呼び出しが毎フレーム走るため、diff が
    /// 切り替わったときにだけ更新する。
    pub highlighted_diff: Vec<HighlightedLine>,
    pub viewport: DiffViewport,
    pub should_quit: bool,
    /// 直近の git 読み取りが失敗していれば、その理由。
    /// これが無いと、失敗と「変更が無い」が画面上で区別できない。
    pub last_error: Option<String>,
    source: Box<dyn DiffSource>,
    highlighter: Highlighter,
}

impl App {
    pub fn new(source: impl DiffSource + 'static) -> Self {
        let mut app = Self {
            files: FileList::default(),
            highlighted_diff: Vec::new(),
            viewport: DiffViewport::new(),
            should_quit: false,
            last_error: None,
            source: Box::new(source),
            highlighter: Highlighter::new(),
        };
        app.reload_files();
        app
    }

    /// git 読み取りの結果から中身を取り出す。失敗なら理由を記録して `None`
    /// を返し、成功すれば前の失敗を消す。
    ///
    /// `what` は何をしようとしていたかの説明。git2 のメッセージ単体では
    /// 「何に失敗したのか」が読み取れないことが多いので前置きに使う。
    fn ok_or_record_error<T>(&mut self, what: &str, result: Result<T, git2::Error>) -> Option<T> {
        match result {
            Ok(value) => {
                self.last_error = None;
                Some(value)
            }
            Err(error) => {
                self.last_error = Some(format!("{what}: {}", error.message()));
                None
            }
        }
    }

    /// ファイル一覧を読み直し、選択と diff を合わせる。
    fn reload_files(&mut self) {
        let result = self.source.changed_files();
        let files = self
            .ok_or_record_error("listing changed files", result)
            .unwrap_or_default();
        self.files.replace(files);
        // 一覧が空になった場合も update_diff を通す。表示内容と viewport を
        // 別経路で更新すると、両者がずれる。
        self.update_diff();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,

            // file navigation
            KeyCode::Char('j') | KeyCode::Down => self.next_file(),
            KeyCode::Char('k') | KeyCode::Up => self.prev_file(),

            // diff scroll
            KeyCode::Char('J') | KeyCode::PageDown => self.viewport.scroll_down(PAGE_LINES),
            KeyCode::Char('K') | KeyCode::PageUp => self.viewport.scroll_up(PAGE_LINES),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.viewport.scroll_down(PAGE_LINES)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.viewport.scroll_up(PAGE_LINES)
            }

            // horizontal scroll
            KeyCode::Char('l') | KeyCode::Right => self.viewport.scroll_right(STEP_COLUMNS),
            KeyCode::Char('h') | KeyCode::Left => self.viewport.scroll_left(STEP_COLUMNS),
            KeyCode::Char('0') => self.viewport.scroll_to_left(),

            // jump
            KeyCode::Char('g') => self.viewport.scroll_to_top(),
            KeyCode::Char('G') => self.viewport.scroll_to_end(),

            // refresh
            KeyCode::Char('r') => self.refresh(),

            _ => {}
        }
    }

    fn next_file(&mut self) {
        if !self.files.select_next() {
            return;
        }
        self.viewport.reset();
        self.update_diff();
    }

    fn prev_file(&mut self) {
        if !self.files.select_prev() {
            return;
        }
        self.viewport.reset();
        self.update_diff();
    }

    fn update_diff(&mut self) {
        let result = self
            .files
            .selected()
            .map(|entry| self.source.file_diff(entry));
        let diff = match result {
            Some(result) => self
                .ok_or_record_error("reading diff", result)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let path = self.files.selected().map(|entry| entry.path.as_str());
        let highlighted = self.highlighter.highlight_diff(&diff, path);
        self.highlighted_diff = highlighted;
        self.viewport.set_content(&self.highlighted_diff);
    }

    /// `r` キー。一覧を読み直し、表示位置も先頭に戻す。
    fn refresh(&mut self) {
        self.reload_files();
        self.viewport.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::highlight::HighlightedLine;
    use crate::test_support::{ScriptedSource, diff_line, entry, error};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Style;

    /// 一覧の取得に失敗したら、その理由を残す。
    /// 残さないと「変更なし」と区別がつかず、画面が嘘をつく。
    #[test]
    fn records_error_when_listing_files_fails() {
        let source = ScriptedSource::new().then_files(Err(error("index is corrupt")));

        let app = App::new(source);

        assert_eq!(
            app.last_error.as_deref(),
            Some("listing changed files: index is corrupt")
        );
        assert!(app.files.entries().is_empty());
    }

    /// diff の取得に失敗したときも理由を残す。表示は空になるので、
    /// 理由が無いと「差分が無いファイル」と区別できない。
    #[test]
    fn records_error_when_reading_diff_fails() {
        let source = ScriptedSource::new()
            .then_files(Ok(vec![entry("src/main.rs")]))
            .then_diff(Err(error("object not found")));

        let app = App::new(source);

        assert_eq!(
            app.last_error.as_deref(),
            Some("reading diff: object not found")
        );
        assert!(app.highlighted_diff.is_empty());
    }

    /// 読み直して成功したらエラーは消える。残り続けると、直っているのに
    /// 壊れているように見える。
    #[test]
    fn clears_error_after_successful_refresh() {
        let source = ScriptedSource::new()
            .then_files(Err(error("index is corrupt")))
            .then_files(Ok(vec![entry("src/main.rs")]))
            .then_diff(Ok(vec![diff_line("fn main() {}\n")]));
        let mut app = App::new(source);
        assert!(app.last_error.is_some());

        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        assert_eq!(app.last_error, None);
        assert_eq!(app.files.entries().len(), 1);
        assert!(!app.highlighted_diff.is_empty());
    }

    /// 成功しているあいだはエラーを持たない。
    #[test]
    fn has_no_error_when_everything_succeeds() {
        let source = ScriptedSource::new()
            .then_files(Ok(vec![entry("src/main.rs")]))
            .then_diff(Ok(vec![diff_line("fn main() {}\n")]));

        let app = App::new(source);

        assert_eq!(app.last_error, None);
        assert_eq!(app.files.entries().len(), 1);
    }

    fn selected(app: &App) -> Option<&str> {
        app.files.selected().map(|entry| entry.path.as_str())
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn paths(names: &[&str]) -> Vec<crate::git::FileEntry> {
        names.iter().map(|name| entry(name)).collect()
    }

    /// 選択中のファイルの diff が 30 行あり、10 行ぶんの枠で 10 行下まで
    /// スクロールした状態にする。
    fn scroll_down_in_long_diff(app: &mut App) {
        app.viewport.set_visible_size(80, 10);
        app.handle_key(press(KeyCode::Char('J')));
        assert_eq!(app.viewport.offset(), (10, 0));
    }

    fn long_diff() -> Result<Vec<crate::git::DiffLine>, git2::Error> {
        Ok(vec![diff_line("line\n"); 30])
    }

    #[test]
    fn j_and_k_on_empty_list_select_nothing_and_keep_error() {
        let source = ScriptedSource::new().then_files(Err(error("index is corrupt")));
        let mut app = App::new(source);

        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('k')));

        assert_eq!(selected(&app), None);
        assert!(app.highlighted_diff.is_empty());
        assert_eq!(
            app.last_error.as_deref(),
            Some("listing changed files: index is corrupt")
        );
    }

    /// 一覧が空なら `j` / `k` は表示位置も diff も触らない。
    ///
    /// 一覧が空のまま diff と表示位置が残る状態は通常の操作では作れないため、
    /// 直接置いて、触ったかどうかが見えるようにする。
    #[test]
    fn j_and_k_on_empty_list_leave_scroll_and_diff_untouched() {
        let mut app = App::new(ScriptedSource::new());
        app.highlighted_diff = (0..30)
            .map(|_| HighlightedLine {
                spans: vec![(Style::default(), "line".to_string())],
            })
            .collect();
        app.viewport.set_content(&app.highlighted_diff);
        scroll_down_in_long_diff(&mut app);

        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('k')));

        assert_eq!(app.viewport.offset(), (10, 0));
        assert_eq!(app.highlighted_diff.len(), 30);
    }

    /// 末尾で `j` を押しても選択は動かないが、表示位置は先頭に戻り、
    /// diff は読み直される。
    #[test]
    fn j_on_last_file_keeps_selection_but_resets_scroll_and_rereads_diff() {
        let source = ScriptedSource::new()
            .then_files(Ok(paths(&["a.rs", "b.rs"])))
            .then_diff(Ok(vec![diff_line("a\n")]))
            .then_diff(long_diff())
            .then_diff(Ok(vec![diff_line("reread\n")]));
        let mut app = App::new(source);
        app.handle_key(press(KeyCode::Char('j')));
        scroll_down_in_long_diff(&mut app);

        app.handle_key(press(KeyCode::Char('j')));

        assert_eq!(selected(&app), Some("b.rs"));
        assert_eq!(app.viewport.offset(), (0, 0));
        assert_eq!(app.highlighted_diff.len(), 1);
    }

    /// 先頭で `k` を押したときも同じ。
    #[test]
    fn k_on_first_file_keeps_selection_but_resets_scroll_and_rereads_diff() {
        let source = ScriptedSource::new()
            .then_files(Ok(paths(&["a.rs", "b.rs"])))
            .then_diff(long_diff())
            .then_diff(Ok(vec![diff_line("reread\n")]));
        let mut app = App::new(source);
        scroll_down_in_long_diff(&mut app);

        app.handle_key(press(KeyCode::Char('k')));

        assert_eq!(selected(&app), Some("a.rs"));
        assert_eq!(app.viewport.offset(), (0, 0));
        assert_eq!(app.highlighted_diff.len(), 1);
    }

    #[test]
    fn j_then_k_walks_through_files_in_order() {
        let source = ScriptedSource::new().then_files(Ok(paths(&["a.rs", "b.rs", "c.rs"])));
        let mut app = App::new(source);
        assert_eq!(selected(&app), Some("a.rs"));

        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(selected(&app), Some("b.rs"));
        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(selected(&app), Some("c.rs"));
        app.handle_key(press(KeyCode::Char('k')));
        assert_eq!(selected(&app), Some("b.rs"));
    }

    /// 一覧が縮んで選択位置がはみ出したら、末尾に詰める。
    #[test]
    fn refresh_clamps_selection_when_list_shrinks() {
        let source = ScriptedSource::new()
            .then_files(Ok(paths(&["a.rs", "b.rs", "c.rs"])))
            .then_files(Ok(paths(&["a.rs", "b.rs"])));
        let mut app = App::new(source);
        app.handle_key(press(KeyCode::Char('j')));
        app.handle_key(press(KeyCode::Char('j')));

        app.handle_key(press(KeyCode::Char('r')));

        assert_eq!(selected(&app), Some("b.rs"));
    }

    /// 一覧が伸びても、選択は同じ位置に留まる（同じファイルを追うのではない）。
    #[test]
    fn refresh_keeps_selection_index_when_list_grows() {
        let source = ScriptedSource::new()
            .then_files(Ok(paths(&["b.rs", "c.rs"])))
            .then_files(Ok(paths(&["a.rs", "b.rs", "c.rs"])));
        let mut app = App::new(source);
        app.handle_key(press(KeyCode::Char('j')));
        assert_eq!(selected(&app), Some("c.rs"));

        app.handle_key(press(KeyCode::Char('r')));

        assert_eq!(selected(&app), Some("b.rs"));
    }

    #[test]
    fn refresh_from_empty_list_selects_first_file() {
        let source = ScriptedSource::new()
            .then_files(Ok(Vec::new()))
            .then_files(Ok(paths(&["a.rs", "b.rs"])));
        let mut app = App::new(source);
        assert_eq!(selected(&app), None);

        app.handle_key(press(KeyCode::Char('r')));

        assert_eq!(selected(&app), Some("a.rs"));
    }

    #[test]
    fn refresh_to_empty_list_clears_selection_and_diff() {
        let source = ScriptedSource::new()
            .then_files(Ok(paths(&["a.rs"])))
            .then_diff(Ok(vec![diff_line("a\n")]))
            .then_files(Ok(Vec::new()));
        let mut app = App::new(source);
        assert!(!app.highlighted_diff.is_empty());

        app.handle_key(press(KeyCode::Char('r')));

        assert_eq!(selected(&app), None);
        assert!(app.highlighted_diff.is_empty());
    }
}
