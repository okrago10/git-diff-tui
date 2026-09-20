use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::git::{DiffLine, FileEntry, GitRepo};
use crate::highlight::{HighlightedLine, Highlighter};
use crate::viewport::DiffViewport;

/// `J` / `K` / `Ctrl+d` / `Ctrl+u` で動かす行数。
const PAGE_LINES: u16 = 10;
/// `h` / `l` で動かす桁数。
const STEP_COLUMNS: u16 = 4;

pub struct App {
    pub files: Vec<FileEntry>,
    pub list_state: ListState,
    pub current_diff: Vec<DiffLine>,
    /// `current_diff` にハイライトを適用した結果。描画のたびに計算し直すと
    /// 最大 10,000 行ぶんの syntect 呼び出しが毎フレーム走るため、diff が
    /// 切り替わったときにだけ更新する。
    pub highlighted_diff: Vec<HighlightedLine>,
    pub viewport: DiffViewport,
    pub should_quit: bool,
    git_repo: GitRepo,
    highlighter: Highlighter,
}

impl App {
    pub fn new(git_repo: GitRepo) -> Self {
        let files = git_repo.changed_files().unwrap_or_default();
        let mut app = Self {
            files,
            list_state: ListState::default(),
            current_diff: Vec::new(),
            highlighted_diff: Vec::new(),
            viewport: DiffViewport::new(),
            should_quit: false,
            git_repo,
            highlighter: Highlighter::new(),
        };
        if !app.files.is_empty() {
            app.list_state.select(Some(0));
            app.update_diff();
        }
        app
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
        let selected = self.list_state.selected();
        self.current_diff = match selected {
            Some(i) if i < self.files.len() => self
                .git_repo
                .file_diff(&self.files[i])
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let path = self.selected_file_path().map(str::to_owned);
        self.highlighted_diff = self
            .highlighter
            .highlight_diff(&self.current_diff, path.as_deref());
        self.viewport.set_content(&self.current_diff);
    }

    fn refresh(&mut self) {
        self.files = self.git_repo.changed_files().unwrap_or_default();
        if self.files.is_empty() {
            self.list_state.select(None);
        } else {
            let idx = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.files.len() - 1);
            self.list_state.select(Some(idx));
        }
        // 一覧が空になった場合も update_diff を通す。current_diff と
        // viewport の内容を別経路で更新すると、両者がずれる。
        self.update_diff();
        self.viewport.reset();
    }

    pub fn selected_file_path(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.files.get(i))
            .map(|f| f.path.as_str())
    }
}
