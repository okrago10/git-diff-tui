# git-diff-tui (`gdf`)

ターミナル上で git の差分をシンタックスハイライト付きで閲覧できる TUI アプリ。

```
┌─ Files ──────────────┬─ Diff ─────────────────────────────┐
│ ● M src/main.rs      │ @@ -10,6 +10,8 @@                 │
│ ○ M src/app.rs       │   fn main() {                      │
│ ● A src/new_file.rs  │ -     let old = true;              │
│ ○ ? untracked.txt    │ +     let new = true;              │
│                      │ +     let added = true;            │
│                      │   }                                │
├──────────────────────┴────────────────────────────────────┤
│ 4 files (2 staged, 2 unstaged) │ j/k: select  q: quit    │
└───────────────────────────────────────────────────────────┘
```

## 特徴

- **2パネルレイアウト** — 左にファイル一覧、右に diff プレビュー
- **シンタックスハイライト** — Sublime Text 互換の構文定義による豊富な言語サポート
- **Staged / Unstaged 表示** — `●` staged、`○` unstaged をアイコンで区別
- **変更種別表示** — `M` 変更 / `A` 追加 / `D` 削除 / `R` リネーム / `?` untracked
- **閲覧専用** — git 操作は一切行わない安全設計

## インストール

### 前提条件

- Rust 1.70+

### ビルド

```bash
git clone <repository-url>
cd git-diff-tui
cargo build --release
```

### PATH に追加

```bash
# cargo bin に配置（推奨）
ln -sf "$(pwd)/target/release/gdf" ~/.cargo/bin/gdf

# または /usr/local/bin に配置
sudo ln -sf "$(pwd)/target/release/gdf" /usr/local/bin/gdf
```

## 使い方

```bash
# git リポジトリ内で実行
cd /path/to/your/repo
gdf
```

## キーバインド

| キー | 動作 |
|------|------|
| `j` / `↓` | 次のファイルを選択 |
| `k` / `↑` | 前のファイルを選択 |
| `J` / `PageDown` / `Ctrl+d` | diff を 10 行下スクロール |
| `K` / `PageUp` / `Ctrl+u` | diff を 10 行上スクロール |
| `g` | diff の先頭へ |
| `G` | diff の末尾へ |
| `r` | ファイル一覧をリフレッシュ |
| `q` / `Esc` | 終了 |

## 技術スタック

| Crate | 用途 |
|-------|------|
| [ratatui](https://github.com/ratatui/ratatui) | TUI フレームワーク |
| [crossterm](https://github.com/crossterm-rs/crossterm) | ターミナル制御 |
| [git2](https://github.com/rust-lang/git2-rs) | libgit2 バインディング |
| [syntect](https://github.com/trishume/syntect) | シンタックスハイライト |

## ライセンス

MIT
