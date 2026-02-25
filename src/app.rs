use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::git::{DiffLine, FileEntry, GitRepo};
use crate::highlight::Highlighter;

pub struct App {
    pub files: Vec<FileEntry>,
    pub list_state: ListState,
    pub current_diff: Vec<DiffLine>,
    pub diff_scroll: u16,
    pub diff_hscroll: u16,
    pub should_quit: bool,
    git_repo: GitRepo,
    pub highlighter: Highlighter,
}

impl App {
    pub fn new(git_repo: GitRepo) -> Self {
        let files = git_repo.changed_files().unwrap_or_default();
        let mut app = Self {
            files,
            list_state: ListState::default(),
            current_diff: Vec::new(),
            diff_scroll: 0,
            diff_hscroll: 0,
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
            KeyCode::Char('J') | KeyCode::PageDown => self.scroll_diff_down(10),
            KeyCode::Char('K') | KeyCode::PageUp => self.scroll_diff_up(10),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_diff_down(10)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_diff_up(10)
            }

            // horizontal scroll
            KeyCode::Char('l') | KeyCode::Right => self.scroll_diff_right(4),
            KeyCode::Char('h') | KeyCode::Left => self.scroll_diff_left(4),
            KeyCode::Char('0') => self.diff_hscroll = 0,

            // jump
            KeyCode::Char('g') => self.diff_scroll = 0,
            KeyCode::Char('G') => self.scroll_to_end(),

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
        self.diff_scroll = 0;
        self.diff_hscroll = 0;
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
        self.diff_scroll = 0;
        self.diff_hscroll = 0;
        self.update_diff();
    }

    pub fn scroll_diff_down(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_add(amount);
    }

    pub fn scroll_diff_up(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_sub(amount);
    }

    pub fn scroll_diff_right(&mut self, amount: u16) {
        self.diff_hscroll = self.diff_hscroll.saturating_add(amount);
    }

    pub fn scroll_diff_left(&mut self, amount: u16) {
        self.diff_hscroll = self.diff_hscroll.saturating_sub(amount);
    }

    fn scroll_to_end(&mut self) {
        let total = self.current_diff.len() as u16;
        self.diff_scroll = total.saturating_sub(1);
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
    }

    fn refresh(&mut self) {
        self.files = self.git_repo.changed_files().unwrap_or_default();
        if self.files.is_empty() {
            self.list_state.select(None);
            self.current_diff.clear();
        } else {
            let idx = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.files.len() - 1);
            self.list_state.select(Some(idx));
            self.update_diff();
        }
        self.diff_scroll = 0;
        self.diff_hscroll = 0;
    }

    pub fn selected_file_path(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.files.get(i))
            .map(|f| f.path.as_str())
    }
}
