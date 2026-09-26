# CLAUDE.md

git の作業ツリー差分をシンタックスハイライト付きで閲覧する TUI（バイナリ名 `gdf`）。設計の経緯は `PLAN.md` を参照。

## ビルドとテスト

- バイナリ専用クレート（`[[bin]] name = "gdf"`）。テストは `src/*.rs` 内の `#[cfg(test)] mod tests` に置く。テスト用の共通部品は `src/test_support.rs`（`main.rs` で `#[cfg(test)] mod test_support` として読み込む）
- 絞り込みは `cargo test viewport` / `cargo test ui::` のようにモジュール名で指定する。失敗時に表示される再実行コマンドは `cargo test --bin gdf <filter>` の形になる
- 初回ビルドは libgit2 のコンパイルを含み 40 秒強かかる
- `main` ブランチは rustfmt 済みではない（別 Issue で対応予定）。`cargo fmt` をツリー全体にかけない

## テストの書き方

- UI の検証は `ratatui::backend::TestBackend::new(W, H)` で描画し、`buffer().content()` を幅ごとに `chunks` して行単位の文字列にする（例: `src/ui.rs` の tests）
- git 読み取りの失敗は `DiffSource` トレイト（`src/git.rs`）と `src/test_support.rs` の `ScriptedSource`（`then_files` / `then_diff`）で注入する

## ライブラリの癖

- git2 の `DiffLine::content` には行頭の `+` / `-` が入らない（`line.origin()` として別に返る）
- syntect の `HighlightLines` は状態を持つ。diff では旧版側と新版側で別々に持ち、ヘッダ行（ハンクやファイルの境界）でリセットする（`src/highlight.rs`）
- 直接依存を足す前に `grep -A2 '^name = "<crate>"' Cargo.lock` で推移依存に既にあるか確かめる
- clippy の `wrong_self_convention` は `to_*(&mut self)` を警告する。状態を変えるメソッドは `scroll_to_*` のように動詞を前に置く
