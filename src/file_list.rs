use ratatui::widgets::ListState;

use crate::git::FileEntry;

/// 変更ファイルの一覧と、その中のどれを選んでいるか。
///
/// 一覧が空でなければ必ずどれか 1 つを選んでおり、空なら何も選んでいない。
/// 選択を動かす操作はすべてここを通し、この不変条件を保つ。
#[derive(Default)]
pub struct FileList {
    entries: Vec<FileEntry>,
    /// 選択位置に加えて一覧のスクロール位置も持つ。描画のたびに作り直すと
    /// 一覧のスクロールの仕方が変わるため、描画をまたいで持ち続ける。
    state: ListState,
}

impl FileList {
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn selected(&self) -> Option<&FileEntry> {
        self.state.selected().and_then(|i| self.entries.get(i))
    }

    /// 一覧を差し替える。選択は同じ位置に留め、はみ出すなら末尾に詰める。
    /// 同じファイルを追いかけはしない。
    pub fn replace(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        let index = match self.entries.len() {
            0 => None,
            len => Some(self.state.selected().unwrap_or(0).min(len - 1)),
        };
        self.state.select(index);
    }

    /// 次のファイルを選ぶ。末尾では動かない。
    pub fn select_next(&mut self) {
        if let Some(i) = self.state.selected() {
            self.state.select(Some((i + 1).min(self.entries.len() - 1)));
        }
    }

    /// 前のファイルを選ぶ。先頭では動かない。
    pub fn select_prev(&mut self) {
        if let Some(i) = self.state.selected() {
            self.state.select(Some(i.saturating_sub(1)));
        }
    }

    /// 描画用。ratatui の `List` は描画時にこれを読み書きする。
    pub fn state_mut(&mut self) -> &mut ListState {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::FileList;
    use crate::git::FileEntry;
    use crate::test_support::entry;

    fn entries(paths: &[&str]) -> Vec<FileEntry> {
        paths.iter().map(|path| entry(path)).collect()
    }

    fn list_of(paths: &[&str]) -> FileList {
        let mut list = FileList::default();
        list.replace(entries(paths));
        list
    }

    fn selected(list: &FileList) -> Option<&str> {
        list.selected().map(|e| e.path.as_str())
    }

    #[test]
    fn empty_list_selects_nothing_and_ignores_moves() {
        let mut list = list_of(&[]);

        list.select_next();
        list.select_prev();

        assert_eq!(selected(&list), None);
    }

    #[test]
    fn single_entry_stays_selected() {
        let mut list = list_of(&["a.rs"]);

        list.select_next();
        assert_eq!(selected(&list), Some("a.rs"));
        list.select_prev();
        assert_eq!(selected(&list), Some("a.rs"));
    }

    #[test]
    fn moves_stop_at_both_ends() {
        let mut list = list_of(&["a.rs", "b.rs", "c.rs"]);

        list.select_prev();
        assert_eq!(selected(&list), Some("a.rs"));
        list.select_next();
        list.select_next();
        list.select_next();
        assert_eq!(selected(&list), Some("c.rs"));
    }

    #[test]
    fn replace_keeps_position_and_clamps_to_last() {
        let mut list = list_of(&["a.rs", "b.rs", "c.rs"]);
        list.select_next();

        list.replace(entries(&["x.rs", "y.rs", "z.rs", "w.rs"]));
        assert_eq!(selected(&list), Some("y.rs"));

        list.replace(entries(&["only.rs"]));
        assert_eq!(selected(&list), Some("only.rs"));
    }

    #[test]
    fn replace_between_empty_and_filled() {
        let mut list = list_of(&["a.rs", "b.rs"]);
        list.select_next();

        list.replace(Vec::new());
        assert_eq!(selected(&list), None);

        list.replace(entries(&["c.rs", "d.rs"]));
        assert_eq!(selected(&list), Some("c.rs"));
    }
}
