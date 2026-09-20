use crate::git::DiffLine;

/// 1 軸ぶんのスクロール状態。
///
/// 縦と横は「内容の大きさ」「見えている大きさ」「現在のオフセット」という
/// 同じ 3 つ組で決まり、上限の求め方も同じなので、1 つの型にまとめる。
#[derive(Debug, Default)]
struct Axis {
    offset: u16,
    content: usize,
    visible: u16,
}

impl Axis {
    fn set_content(&mut self, content: usize) {
        self.content = content;
        self.clamp();
    }

    fn set_visible(&mut self, visible: u16) {
        self.visible = visible;
        self.clamp();
    }

    fn forward(&mut self, amount: u16) {
        self.offset = self.offset.saturating_add(amount).min(self.max_offset());
    }

    fn backward(&mut self, amount: u16) {
        self.offset = self.offset.saturating_sub(amount);
    }

    fn scroll_to_start(&mut self) {
        self.offset = 0;
    }

    fn scroll_to_end(&mut self) {
        self.offset = self.max_offset();
    }

    fn clamp(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }

    /// これ以上送ると内容の末尾が画面から出てしまう、というオフセットの上限。
    fn max_offset(&self) -> u16 {
        let max = self.content.saturating_sub(self.visible as usize);
        max.min(u16::MAX as usize) as u16
    }
}

/// diff プレビューの表示位置を保持する。
///
/// スクロールの可否を決めるには「内容の大きさ」と「見えている範囲の大きさ」の
/// 両方が要る。この 2 つと現在のオフセットを 1 箇所に置き、`offset()` が返す値が
/// 常に表示可能な範囲に収まっていることを保証する。
#[derive(Debug, Default)]
pub struct DiffViewport {
    vertical: Axis,
    horizontal: Axis,
}

impl DiffViewport {
    pub fn new() -> Self {
        Self::default()
    }

    /// 表示中の diff を設定する。総行数と最長行の幅がスクロールの上限になる。
    pub fn set_content(&mut self, lines: &[DiffLine]) {
        self.vertical.set_content(lines.len());
        self.horizontal
            .set_content(lines.iter().map(DiffLine::display_width).max().unwrap_or(0));
    }

    /// 枠線を除いた、実際に diff が見えている範囲の大きさ。
    pub fn set_visible_size(&mut self, width: u16, height: u16) {
        self.horizontal.set_visible(width);
        self.vertical.set_visible(height);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.vertical.forward(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.vertical.backward(amount);
    }

    pub fn scroll_right(&mut self, amount: u16) {
        self.horizontal.forward(amount);
    }

    pub fn scroll_left(&mut self, amount: u16) {
        self.horizontal.backward(amount);
    }

    /// 先頭行へ戻す。横位置は保つ。
    pub fn scroll_to_top(&mut self) {
        self.vertical.scroll_to_start();
    }

    /// 左端へ戻す。縦位置は保つ。
    pub fn scroll_to_left(&mut self) {
        self.horizontal.scroll_to_start();
    }

    /// 最終行が画面の最下部に来る位置へ移動する。
    pub fn scroll_to_end(&mut self) {
        self.vertical.scroll_to_end();
    }

    /// 縦横とも先頭に戻す。別のファイルを選び直したときに使う。
    pub fn reset(&mut self) {
        self.scroll_to_top();
        self.scroll_to_left();
    }

    /// `Paragraph::scroll` にそのまま渡せる (縦, 横) のオフセット。
    pub fn offset(&self) -> (u16, u16) {
        (self.vertical.offset, self.horizontal.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::DiffViewport;
    use crate::git::{DiffLine, DiffLineKind};

    /// 内容が画面に収まっているときはスクロールしない。
    /// 収まっているのに動かせると、diff 全体が画面外に消える。
    #[test]
    fn does_not_scroll_when_content_fits_in_viewport() {
        let mut viewport = DiffViewport::new();
        viewport.set_content(&diff_of(5, 10));
        viewport.set_visible_size(80, 10);

        viewport.scroll_down(10);

        assert_eq!(viewport.offset(), (0, 0));
    }

    /// `G` は最終行が画面の最下部に来る位置へ移動する。
    /// 総行数 100・表示 10 行なら、91〜100 行目が見える 90 が正しい。
    /// 従来の `total - 1` = 99 は最終行だけを最上部に置き、残り 9 行を空白にしていた。
    #[test]
    fn scroll_to_end_places_last_line_at_bottom_of_viewport() {
        let mut viewport = DiffViewport::new();
        viewport.set_content(&diff_of(100, 10));
        viewport.set_visible_size(80, 10);

        viewport.scroll_to_end();

        assert_eq!(viewport.offset(), (90, 0));
    }

    /// 下方向のスクロールも同じ位置で止まる。
    #[test]
    fn stops_vertical_scroll_at_last_line() {
        let mut viewport = DiffViewport::new();
        viewport.set_content(&diff_of(100, 10));
        viewport.set_visible_size(80, 10);

        viewport.scroll_down(1000);

        assert_eq!(viewport.offset(), (90, 0));
    }

    /// 横スクロールは、最長行の右端が画面の右端に来る位置で止まる。
    /// 最長 20 桁・表示幅 10 桁なら 10 が上限で、それ以上は全面が空白になる。
    #[test]
    fn stops_horizontal_scroll_at_longest_line() {
        let mut viewport = DiffViewport::new();
        viewport.set_content(&diff_of(1, 20));
        viewport.set_visible_size(10, 10);

        viewport.scroll_right(1000);

        assert_eq!(viewport.offset(), (0, 10));
    }

    /// 短い diff に切り替わったり画面が広がったりしたら、オフセットは
    /// 新しい上限まで引き戻される。引き戻さないと、内容があるのに
    /// 画面が空白のままになる。
    #[test]
    fn clamps_offset_when_content_shrinks() {
        let mut viewport = DiffViewport::new();
        viewport.set_content(&diff_of(100, 10));
        viewport.set_visible_size(80, 10);
        viewport.scroll_to_end();
        assert_eq!(viewport.offset(), (90, 0));

        viewport.set_content(&diff_of(12, 10));

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
        viewport.set_content(&diff_of(100, 200));
        viewport.set_visible_size(20, 10);
        viewport.scroll_down(50);
        viewport.scroll_right(50);
        assert_eq!(viewport.offset(), (50, 50));
        viewport
    }

    /// 指定した行数・桁数の diff を作る。
    fn diff_of(line_count: usize, width: usize) -> Vec<DiffLine> {
        (0..line_count)
            .map(|_| DiffLine {
                kind: DiffLineKind::Context,
                content: format!("{}\n", "x".repeat(width)),
            })
            .collect()
    }
}
