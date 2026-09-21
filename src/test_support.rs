//! テスト用の差分ソースとフィクスチャ。
//!
//! `App` の振る舞いは git リポジトリの状態に左右されるため、`DiffSource` を
//! 差し替えて任意の結果（特に失敗）を再現できるようにする。

use std::cell::RefCell;
use std::collections::VecDeque;

use crate::git::{ChangeKind, DiffLine, DiffLineKind, DiffSource, FileEntry, Stage};

/// 呼び出しごとに、あらかじめ決めた結果を順に返す差分ソース。
/// 用意した結果を使い切ったあとは成功（空）を返す。
#[derive(Default)]
pub struct ScriptedSource {
    files: RefCell<VecDeque<Result<Vec<FileEntry>, git2::Error>>>,
    diffs: RefCell<VecDeque<Result<Vec<DiffLine>, git2::Error>>>,
}

impl ScriptedSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// `changed_files` が次に返す結果を積む。
    pub fn then_files(self, result: Result<Vec<FileEntry>, git2::Error>) -> Self {
        self.files.borrow_mut().push_back(result);
        self
    }

    /// `file_diff` が次に返す結果を積む。
    pub fn then_diff(self, result: Result<Vec<DiffLine>, git2::Error>) -> Self {
        self.diffs.borrow_mut().push_back(result);
        self
    }
}

impl DiffSource for ScriptedSource {
    fn changed_files(&self) -> Result<Vec<FileEntry>, git2::Error> {
        self.files
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }

    fn file_diff(&self, _entry: &FileEntry) -> Result<Vec<DiffLine>, git2::Error> {
        self.diffs
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }
}

pub fn error(message: &str) -> git2::Error {
    git2::Error::from_str(message)
}

pub fn entry(path: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        kind: ChangeKind::Modified,
        stage: Stage::Unstaged,
    }
}

pub fn diff_line(content: &str) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::Context,
        content: content.to_string(),
    }
}
