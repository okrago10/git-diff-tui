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
    /// 一覧が空なら何もせず `false` を返す。
    pub fn select_next(&mut self) -> bool {
        let Some(last) = self.entries.len().checked_sub(1) else {
            return false;
        };
        let next = self.state.selected().map_or(0, |i| (i + 1).min(last));
        self.state.select(Some(next));
        true
    }

    /// 前のファイルを選ぶ。先頭では動かない。
    /// 一覧が空なら何もせず `false` を返す。
    pub fn select_prev(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let prev = self.state.selected().map_or(0, |i| i.saturating_sub(1));
        self.state.select(Some(prev));
        true
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

        assert!(!list.select_next());
        assert!(!list.select_prev());

        assert_eq!(selected(&list), None);
    }

    /// `state_mut` 経由で選択が残ったまま一覧が空になっていても、動かそうとして
    /// 桁あふれしない。
    #[test]
    fn empty_list_with_stale_selection_ignores_moves() {
        let mut list = list_of(&[]);
        list.state_mut().select(Some(0));

        assert!(!list.select_next());
        assert!(!list.select_prev());

        assert_eq!(selected(&list), None);
    }

    /// 端で止まっても、選ぶ対象はあったので `true` を返す。
    #[test]
    fn single_entry_stays_selected() {
        let mut list = list_of(&["a.rs"]);

        assert!(list.select_next());
        assert_eq!(selected(&list), Some("a.rs"));
        assert!(list.select_prev());
        assert_eq!(selected(&list), Some("a.rs"));
    }

    #[test]
    fn moves_without_selection_pick_first() {
        let mut list = list_of(&["a.rs", "b.rs"]);

        list.state_mut().select(None);
        assert!(list.select_next());
        assert_eq!(selected(&list), Some("a.rs"));

        list.state_mut().select(None);
        assert!(list.select_prev());
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
