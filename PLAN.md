# git-diff-tui 実装計画

## Context

Ghostty ターミナルでは VSCode のソースコントロールパネルのような差分ファイルの視覚的確認ができない。
ターミナル上で動作する TUI (Terminal UI) アプリを Rust で開発し、git の作業ツリー差分をシンタックスハイライト付きで閲覧できるようにする。

## 要件サマリー

| 項目 | 内容 |
|------|------|
| ツール名 | `git-diff-tui`（バイナリ名: `gdf`） |
| 配置先 | `~/git-diff-tui`（新規リポジトリ） |
| 言語 | Rust |
| 目的 | 差分の可視化・閲覧に特化（git 操作は行わない） |
| 対象 | 作業ツリーの差分のみ（staged + unstaged + untracked） |
| レイアウト | 左パネル: ファイル一覧（30%）、右パネル: diff プレビュー（70%） |
| ファイル一覧 | フラット表示、● staged / ○ unstaged アイコン + 変更種別（M/A/D/R/?） |
| Diff 表示 | シンタックスハイライト + 追加行(緑) / 削除行(赤) の色分け |

## 技術スタック

| Crate | バージョン | 用途 | 備考 |
|-------|-----------|------|------|
| ratatui | 0.30 | TUI フレームワーク（crossterm バックエンド） | 0.30 でモジュラーワークスペース化。メインクレートが全 re-export するため影響なし |
| crossterm | 0.29 | ターミナル制御・キーイベント | ratatui 0.30 のデフォルトバックエンド。イベント処理 API のために直接依存 |
| git2 | 0.20 | libgit2 バインディング（差分取得） | ビルド時に libgit2 を C ソースからコンパイル（cmake が必要になる場合あり） |
| syntect | 5.3 | シンタックスハイライト | default-fancy feature で pure Rust regex エンジンを使用 |

## セキュリティ対策

1. git2 (libgit2) を常に最新パッチバージョンに保つ
2. `cargo audit` を定期的に実行
   - git push 前に自動実行（pre-push フック）
   - 月一で手動実行
3. syntect は内蔵構文定義のみ使用（外部 .sublime-syntax の読み込みは行わない）
4. ファイルパス表示時の ANSI エスケープシーケンスに留意
5. git2 の使用は読み取り系 API に限定し、Index 書き込み系（`Index::add` 等）は使用しない

## ディレクトリ構成

```
~/git-diff-tui/
├── Cargo.toml
├── PLAN.md
├── src/
│   ├── main.rs          # エントリポイント: Terminal 初期化、メインループ
│   ├── app.rs           # アプリ状態管理（ファイル選択、スクロール、キーイベント処理）
│   ├── git.rs           # git2 による差分取得（changed_files, file_diff）
│   ├── ui.rs            # ratatui による 2 パネル描画
│   └── highlight.rs     # syntect → ratatui カラー変換
```

## 実装フェーズ

### Phase 0: 環境構築

0. **Rust インストール** — `rustup` によるツールチェーンセットアップ
1. **`Cargo.toml`** — プロジェクト作成、依存関係定義

### Phase 1: 最小動作版（ファイル一覧のみ）

2. **`src/git.rs`** — `GitRepo::open()` + `changed_files()` 実装
   - `diff_tree_to_index()` で staged、`diff_index_to_workdir()` で unstaged を取得
   - `repo.statuses()` で untracked files を取得
3. **`src/app.rs`** — `App::new()` + `handle_key()`（q 終了、j/k 移動）
4. **`src/ui.rs`** — 左パネルのみ描画（ファイル一覧 + 選択ハイライト）
5. **`src/main.rs`** — ターミナル初期化/復元、パニックハンドラ、メインループ
6. **動作確認**: `cargo run` でファイル一覧表示 → j/k 移動 → q 終了

### Phase 2: Diff プレビュー（色分けのみ）

7. **`src/git.rs`** — `file_diff()` 実装（unified diff を行単位で取得）
8. **`src/ui.rs`** — 右パネル追加（Paragraph で diff 表示、+行=緑 / -行=赤）
9. **`src/app.rs`** — `update_diff()` + `scroll_down/up()` 実装
10. **動作確認**: ファイル選択で diff 切替、J/K でスクロール

### Phase 3: シンタックスハイライト

11. **`src/highlight.rs`** — `Highlighter::new()` + `highlight_diff()` 実装
    - syntect の Color → ratatui の Color::Rgb 変換
    - diff 行種別ごとの背景色 + シンタックスハイライトの合成
12. **`src/ui.rs`** — diff プレビューをハイライト版に差し替え
13. **動作確認**: コード部分にシンタックスハイライトが適用されること

### Phase 4: 仕上げ

14. **エラーハンドリング** — git リポジトリ外、変更なし、バイナリファイル
15. **ステータスバー** — 画面下部にファイル数 + キーヒント表示
16. **`r` キーリフレッシュ** — ファイル一覧の再取得
17. **インストール** — `cargo build --release` + シンボリックリンク

## 主要な型定義

### git.rs

```rust
pub enum ChangeKind { Added, Modified, Deleted, Renamed, Copied, Typechange, Untracked }
pub enum Stage { Staged, Unstaged }
pub struct FileEntry { pub path: String, pub kind: ChangeKind, pub stage: Stage }
pub enum DiffLineKind { Context, Addition, Deletion, HunkHeader, FileHeader }
pub struct DiffLine { pub kind: DiffLineKind, pub content: String }

pub struct GitRepo { repo: Repository }
impl GitRepo {
    pub fn open(path: Option<&str>) -> Result<Self, git2::Error>;
    pub fn changed_files(&self) -> Result<Vec<FileEntry>, git2::Error>;
    pub fn file_diff(&self, entry: &FileEntry) -> Result<Vec<DiffLine>, git2::Error>;
}
```

### app.rs

```rust
pub struct App {
    pub files: Vec<FileEntry>,
    pub list_state: ListState,
    pub current_diff: Vec<DiffLine>,
    pub diff_scroll: u16,
    pub should_quit: bool,
    git_repo: GitRepo,
    pub highlighter: Highlighter,
}
```

## キーバインド

| キー | 動作 |
|------|------|
| `j` / `↓` | 次のファイル選択 |
| `k` / `↑` | 前のファイル選択 |
| `J` / `PageDown` / `Ctrl+d` | diff 10行下スクロール |
| `K` / `PageUp` / `Ctrl+u` | diff 10行上スクロール |
| `g` | diff 先頭へ |
| `G` | diff 末尾へ |
| `r` | ファイル一覧リフレッシュ |
| `q` / `Esc` | 終了 |

## エッジケース対処

| ケース | 対処 |
|--------|------|
| git リポジトリ外 | Terminal 初期化前に eprintln + exit(1) |
| 変更ファイル 0 件 | 右パネルに "No changes detected" 表示 |
| バイナリファイル | "Binary file differs" 表示 |
| 初期リポジトリ（コミットなし） | 空ツリーとの diff にフォールバック |
| 大きなファイル | 10,000 行上限 + truncated メッセージ |
| 未知の拡張子 | plain text にフォールバック |
| パニック時 | カスタムパニックハンドラでターミナル復元 |

## 検証方法

1. **Phase 1 完了後**: git リポジトリ内で `cargo run` を実行し、ファイル一覧が表示されること
2. **Phase 2 完了後**: ファイル選択→diff表示、スクロールが動作すること
3. **Phase 3 完了後**: `.ts` / `.rs` ファイルの diff にシンタックスハイライトが付くこと
4. **Phase 4 完了後**: git リポジトリ外で実行→エラーメッセージ、変更なし→適切なメッセージ
5. **最終**: `cargo build --release` → `gdf` コマンドで起動確認
