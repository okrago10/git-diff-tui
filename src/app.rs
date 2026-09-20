use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::git::{DiffSource, FileEntry};
use crate::highlight::{HighlightedLine, Highlighter};
use crate::viewport::DiffViewport;

/// `J` / `K` / `Ctrl+d` / `Ctrl+u` で動かす行数。
const PAGE_LINES: u16 = 10;
/// `h` / `l` で動かす桁数。
const STEP_COLUMNS: u16 = 4;

pub struct App {
    pub files: Vec<FileEntry>,
    pub list_state: ListState,
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
            files: Vec::new(),
            list_state: ListState::default(),
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
        self.files = self
            .ok_or_record_error("listing changed files", result)
            .unwrap_or_default();
        if self.files.is_empty() {
            self.list_state.select(None);
        } else {
            let index = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.files.len() - 1);
            self.list_state.select(Some(index));
        }
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
        if self.files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.files.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(i));
        self.viewport.reset();
        self.update_diff();
    }

    fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.list_state.select(Some(i));
        self.viewport.reset();
        self.update_diff();
    }

    fn update_diff(&mut self) {
        let diff = match self.list_state.selected() {
            Some(i) if i < self.files.len() => {
                let result = self.source.file_diff(&self.files[i]);
                self.ok_or_record_error("reading diff", result)
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let highlighted = self
            .highlighter
            .highlight_diff(&diff, self.selected_file_path());
        self.highlighted_diff = highlighted;
        self.viewport.set_content(&self.highlighted_diff);
    }

    /// `r` キー。一覧を読み直し、表示位置も先頭に戻す。
    fn refresh(&mut self) {
        self.reload_files();
        self.viewport.reset();
    }

    pub fn selected_file_path(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.files.get(i))
            .map(|f| f.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::test_support::{ScriptedSource, diff_line, entry, error};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        assert!(app.files.is_empty());
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
        assert_eq!(app.files.len(), 1);
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
        assert_eq!(app.files.len(), 1);
    }
}
