use unicode_width::UnicodeWidthStr;

use crate::git::DiffLine;

/// diff 行のうち最も長い行の表示幅（端末のセル数）。
///
/// 横スクロールの上限に使う。行末の改行は表示されないので幅に含めない。
pub fn content_width(lines: &[DiffLine]) -> usize {
    lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.content.trim_end_matches(['\n', '\r'])))
        .max()
        .unwrap_or(0)
}

/// diff プレビューの表示位置を保持する。
///
/// スクロールの可否を決めるには「内容の大きさ」と「見えている範囲の大きさ」の
/// 両方が要る。この 2 つと現在のオフセットを 1 箇所に置き、`offset()` が返す値が
/// 常に表示可能な範囲に収まっていることを保証する。
#[derive(Debug, Default)]
pub struct DiffViewport {
    offset_y: u16,
    offset_x: u16,
    content_len: usize,
    content_width: usize,
    height: u16,
    width: u16,
}

impl DiffViewport {
    pub fn new() -> Self {
        Self::default()
    }

    /// 表示中の diff の総行数を設定する。
    pub fn set_content_len(&mut self, len: usize) {
        self.content_len = len;
        self.clamp();
    }

    /// 表示中の diff の最長行の幅を設定する。
    pub fn set_content_width(&mut self, width: usize) {
        self.content_width = width;
        self.clamp();
    }

    /// 枠線を除いた表示可能な行数を設定する。
    pub fn set_height(&mut self, height: u16) {
        self.height = height;
        self.clamp();
    }

    /// 枠線を除いた表示可能な桁数を設定する。
    pub fn set_width(&mut self, width: u16) {
        self.width = width;
        self.clamp();
    }

    /// 現在位置から相対的に移動する。正が下・右、負が上・左。
    /// 移動後の位置は必ず表示可能な範囲に収まる。
    pub fn scroll_by(&mut self, dy: i32, dx: i32) {
        self.offset_y = apply_delta(self.offset_y, dy).min(self.max_offset_y());
        self.offset_x = apply_delta(self.offset_x, dx).min(self.max_offset_x());
    }

    /// 先頭行へ戻す。横位置は保つ。
    pub fn scroll_to_top(&mut self) {
        self.offset_y = 0;
    }

    /// 左端へ戻す。縦位置は保つ。
    pub fn scroll_to_left(&mut self) {
        self.offset_x = 0;
    }

    /// 縦横とも先頭に戻す。別のファイルを選び直したときに使う。
    pub fn reset(&mut self) {
        self.scroll_to_top();
        self.scroll_to_left();
    }

    /// 最終行が画面の最下部に来る位置へ移動する。
    pub fn scroll_to_end(&mut self) {
        self.offset_y = self.max_offset_y();
    }

    /// 現在のオフセットを、内容と画面の大きさから決まる上限まで引き戻す。
    fn clamp(&mut self) {
        self.offset_y = self.offset_y.min(self.max_offset_y());
        self.offset_x = self.offset_x.min(self.max_offset_x());
    }

    /// これ以上下げると最終行が画面から出てしまう、という縦オフセットの上限。
    fn max_offset_y(&self) -> u16 {
        clamp_to_u16(self.content_len.saturating_sub(self.height as usize))
    }

    /// これ以上右へ送ると最長行が画面から出てしまう、という横オフセットの上限。
    fn max_offset_x(&self) -> u16 {
        clamp_to_u16(self.content_width.saturating_sub(self.width as usize))
    }

    /// `Paragraph::scroll` にそのまま渡せる (縦, 横) のオフセット。
    pub fn offset(&self) -> (u16, u16) {
        (self.offset_y, self.offset_x)
    }
}

fn clamp_to_u16(value: usize) -> u16 {
    value.min(u16::MAX as usize) as u16
}

fn apply_delta(current: u16, delta: i32) -> u16 {
    if delta >= 0 {
        current.saturating_add(delta.min(u16::MAX as i32) as u16)
    } else {
        current.saturating_sub(delta.unsigned_abs().min(u16::MAX as u32) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiffViewport, content_width};
    use crate::git::{DiffLine, DiffLineKind};

    /// 横スクロールの上限は文字数ではなく表示セル数で決まる。
    /// 全角 3 文字は 6 セルを占めるので、"abc"(3 セル) より長い。
    #[test]
    fn content_width_counts_display_cells_of_longest_line() {
        let lines = vec![
            diff_line("abc\n"),
            diff_line("あいう\n"),
        ];

        assert_eq!(content_width(&lines), 6);
    }

    fn diff_line(content: &str) -> DiffLine {
        DiffLine {
            kind: DiffLineKind::Context,
            content: content.to_string(),
        }
    }

    /// 内容が画面に収まっているときはスクロールしない。
    /// 収まっているのに動かせると、diff 全体が画面外に消える。
    #[test]
    fn does_not_scroll_when_content_fits_in_viewport() {
        let mut viewport = DiffViewport::new();
        viewport.set_content_len(5);
        viewport.set_height(10);

        viewport.scroll_by(10, 0);

        assert_eq!(viewport.offset(), (0, 0));
    }

    /// `G` は最終行が画面の最下部に来る位置へ移動する。
    /// 総行数 100・表示 10 行なら、91〜100 行目が見える 90 が正しい。
    /// 従来の `total - 1` = 99 は最終行だけを最上部に置き、残り 9 行を空白にしていた。
    #[test]
    fn scroll_to_end_places_last_line_at_bottom_of_viewport() {
        let mut viewport = DiffViewport::new();
        viewport.set_content_len(100);
        viewport.set_height(10);

        viewport.scroll_to_end();

        assert_eq!(viewport.offset(), (90, 0));
    }

    /// 横スクロールは、最長行の右端が画面の右端に来る位置で止まる。
    /// 最長 20 桁・表示幅 10 桁なら 10 が上限で、それ以上は全面が空白になる。
    #[test]
    fn stops_horizontal_scroll_at_longest_line() {
        let mut viewport = DiffViewport::new();
        viewport.set_content_width(20);
        viewport.set_width(10);

        viewport.scroll_by(0, 1000);

        assert_eq!(viewport.offset(), (0, 10));
    }

    /// 短い diff に切り替わったり画面が広がったりしたら、オフセットは
    /// 新しい上限まで引き戻される。引き戻さないと、内容があるのに
    /// 画面が空白のままになる。
    #[test]
    fn clamps_offset_when_content_shrinks() {
        let mut viewport = DiffViewport::new();
        viewport.set_content_len(100);
        viewport.set_height(10);
        viewport.scroll_to_end();
        assert_eq!(viewport.offset(), (90, 0));

        viewport.set_content_len(12);

        assert_eq!(viewport.offset(), (2, 0));
    }

    /// 別のファイルを選び直したときは縦横とも先頭に戻す。
    #[test]
    fn reset_returns_to_top_left() {
        let mut viewport = scrolled_viewport();

        viewport.reset();

        assert_eq!(viewport.offset(), (0, 0));
    }

    /// `g` は diff の先頭行へ戻すだけで、横位置はそのまま保つ。
    #[test]
    fn scroll_to_top_keeps_horizontal_offset() {
        let mut viewport = scrolled_viewport();

        viewport.scroll_to_top();

        assert_eq!(viewport.offset(), (0, 50));
    }

    /// `0` は左端へ戻すだけで、縦位置はそのまま保つ。
    #[test]
    fn scroll_to_left_keeps_vertical_offset() {
        let mut viewport = scrolled_viewport();

        viewport.scroll_to_left();

        assert_eq!(viewport.offset(), (50, 0));
    }

    /// 縦横ともスクロール済みのビューポート。
    fn scrolled_viewport() -> DiffViewport {
        let mut viewport = DiffViewport::new();
        viewport.set_content_len(100);
        viewport.set_content_width(200);
        viewport.set_height(10);
        viewport.set_width(20);
        viewport.scroll_by(50, 50);
        assert_eq!(viewport.offset(), (50, 50));
        viewport
    }
}
