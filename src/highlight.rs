use ratatui::style::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use unicode_width::UnicodeWidthStr;

use crate::git::{DiffLine, DiffLineKind};

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

pub struct HighlightedLine {
    pub spans: Vec<(ratatui::style::Style, String)>,
}

impl HighlightedLine {
    /// この行が端末で占める幅（セル数）。
    ///
    /// 行頭マーカーを含んだ、実際に描かれる幅。横スクロールの上限に使う。
    /// 全角文字は 2 セルを占めるため文字数とは一致せず、行末の改行は
    /// 表示されないので含めない。
    pub fn display_width(&self) -> usize {
        self.spans
            .iter()
            .map(|(_, text)| UnicodeWidthStr::width(text.trim_end_matches(['\n', '\r'])))
            .sum()
    }
}

/// ヘッダ行の文字色。構文ハイライトを当てないので、この表示側で決める。
const HEADER_FOREGROUND: Color = Color::White;

const BG_ADDITION: Color = Color::Rgb(0, 60, 0);
const BG_DELETION: Color = Color::Rgb(80, 0, 0);
const BG_HUNK_HEADER: Color = Color::Rgb(40, 40, 80);

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn highlight_diff(
        &self,
        lines: &[DiffLine],
        file_path: Option<&str>,
    ) -> Vec<HighlightedLine> {
        let syntax = file_path
            .and_then(|p| {
                let ext = std::path::Path::new(p).extension()?.to_str()?;
                self.syntax_set.find_syntax_by_extension(ext)
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        // diff には旧版と新版のコードが交互に現れる。syntect のパーサは
        // 文字列やブロックコメントの継続を行をまたいで持つため、片側で開いた
        // ままの状態がもう片側に漏れないよう、2 本に分ける。
        let mut new_side = HighlightLines::new(syntax, theme);
        let mut old_side = HighlightLines::new(syntax, theme);

        lines
            .iter()
            .map(|line| {
                let decoration = LineDecoration::for_kind(line.kind);

                // ヘッダ行はソースコードではないので構文パーサに通さない
                let parsed = match decoration.side {
                    SyntaxSide::Boundary => {
                        // 別のハンク（別のファイル）に入る。前のハンクで開いた
                        // ままの文字列やコメントは次のハンクには続かない。
                        old_side = HighlightLines::new(syntax, theme);
                        new_side = HighlightLines::new(syntax, theme);
                        None
                    }
                    SyntaxSide::Old => Some(
                        old_side
                            .highlight_line(&line.content, &self.syntax_set)
                            .unwrap_or_default(),
                    ),
                    SyntaxSide::New => Some(
                        new_side
                            .highlight_line(&line.content, &self.syntax_set)
                            .unwrap_or_default(),
                    ),
                    SyntaxSide::Both => {
                        // 文脈行は旧版にも新版にも属するので、両方の状態を進める
                        let _ = old_side.highlight_line(&line.content, &self.syntax_set);
                        Some(
                            new_side
                                .highlight_line(&line.content, &self.syntax_set)
                                .unwrap_or_default(),
                        )
                    }
                };

                let mut spans = Vec::new();

                if let Some((marker, color)) = decoration.marker {
                    spans.push((decoration.plain_style(color), marker.to_string()));
                }

                match parsed {
                    Some(highlighted) => {
                        for (style, text) in highlighted {
                            spans.push((
                                syntect_to_ratatui_style(style, decoration.background),
                                text.to_string(),
                            ));
                        }
                    }
                    None => spans.push((
                        decoration.plain_style(HEADER_FOREGROUND),
                        line.content.clone(),
                    )),
                }

                HighlightedLine { spans }
            })
            .collect()
    }
}

/// diff の行がどちらの版のコードに属するか。
///
/// syntect のパーサは文字列やブロックコメントの継続を行をまたいで持つ。
/// 旧版と新版は別々の状態で読む必要があり、diff 自身の書式であるヘッダ行は
/// どちらの状態にも入れてはいけない。
enum SyntaxSide {
    /// 旧版のみ（削除行）
    Old,
    /// 新版のみ（追加行）
    New,
    /// 両方（文脈行）
    Both,
    /// ソースコードではなく、ハンクやファイルの切れ目（ヘッダ行）
    Boundary,
}

/// 行種別から決まる見た目と読み方。行種別に対する分岐はここ 1 箇所に集める。
struct LineDecoration {
    /// unified diff の行頭マーカーと、その文字色。
    /// ヘッダ行はそれ自体が書式を持つのでマーカーを付けない。
    marker: Option<(&'static str, Color)>,
    background: Option<Color>,
    side: SyntaxSide,
}

impl LineDecoration {
    /// 構文ハイライトを当てない部分（マーカーとヘッダ行）の書式。
    fn plain_style(&self, foreground: Color) -> ratatui::style::Style {
        ratatui::style::Style::default()
            .fg(foreground)
            .bg(self.background.unwrap_or(Color::Reset))
    }
}

impl LineDecoration {
    fn for_kind(kind: DiffLineKind) -> Self {
        match kind {
            DiffLineKind::Addition => Self {
                marker: Some(("+", Color::Green)),
                background: Some(BG_ADDITION),
                side: SyntaxSide::New,
            },
            DiffLineKind::Deletion => Self {
                marker: Some(("-", Color::Red)),
                background: Some(BG_DELETION),
                side: SyntaxSide::Old,
            },
            DiffLineKind::Context => Self {
                marker: Some((" ", Color::White)),
                background: None,
                side: SyntaxSide::Both,
            },
            DiffLineKind::HunkHeader => Self {
                marker: None,
                background: Some(BG_HUNK_HEADER),
                side: SyntaxSide::Boundary,
            },
            DiffLineKind::FileHeader => Self {
                marker: None,
                background: None,
                side: SyntaxSide::Boundary,
            },
        }
    }
}

fn syntect_to_ratatui_style(style: Style, bg_override: Option<Color>) -> ratatui::style::Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let bg = bg_override.unwrap_or(Color::Rgb(
        style.background.r,
        style.background.g,
        style.background.b,
    ));
    ratatui::style::Style::default().fg(fg).bg(bg)
}

#[cfg(test)]
mod tests {
    use super::Highlighter;
    use crate::git::{DiffLine, DiffLineKind};
    use std::sync::OnceLock;

    /// `Highlighter::new()` は同梱ダンプ（数 MB）を毎回展開・デシリアライズする
    /// ため、テスト間で使い回す。`highlight_diff` は `&self` で、呼び出しごとに
    /// `HighlightLines` を作るので共有して問題ない。
    fn highlighter() -> &'static Highlighter {
        static H: OnceLock<Highlighter> = OnceLock::new();
        H.get_or_init(Highlighter::new)
    }

    /// 同梱のデフォルト構文定義とテーマが実行時に読めることを確認する。
    ///
    /// syntect の `default-syntaxes` / `default-themes` feature を外すと
    /// `load_defaults_newlines()` / `load_defaults()` 自体が消えてコンパイル
    /// エラーになるため、feature の外し忘れはビルドで気付ける。一方、
    /// syntect の更新で同梱ダンプの中身（テーマ名など）が変わった場合は
    /// ビルドが通ったまま実行時に落ちる。`themes[..]` はインデックス
    /// アクセスで、TUI が raw mode に入った後の描画中に panic するため、
    /// ここで先に検出する。
    #[test]
    fn loads_default_syntaxes_and_theme() {
        let h = highlighter();
        assert!(h.syntax_set.find_syntax_by_extension("rs").is_some());
        assert!(h.theme_set.themes.contains_key("base16-ocean.dark"));
    }

    /// マーカーを除いた本文の、構文ハイライト（前景色）とテキスト。
    /// 背景色は行種別ごとに違うので、構文の解釈だけを比べるときに使う。
    fn body_syntax(line: &super::HighlightedLine) -> Vec<(Option<ratatui::style::Color>, String)> {
        line.spans[1..]
            .iter()
            .map(|(style, text)| (style.fg, text.clone()))
            .collect()
    }

    fn line(kind: DiffLineKind, content: &str) -> DiffLine {
        DiffLine {
            kind,
            content: content.to_string(),
        }
    }

    fn render(line: &super::HighlightedLine) -> String {
        line.spans.iter().map(|(_, t)| t.as_str()).collect()
    }

    /// ハイライトを通してもテキストが失われないこと。
    ///
    /// フィクスチャは実際の `GitRepo::file_diff` の出力に合わせてある。
    /// libgit2 は `+` / `-` を `line.origin()` として別に返すため、
    /// `DiffLine::content` には行頭マーカーが含まれない（src/git.rs 参照）。
    /// マーカーは表示時に `DiffLineKind` から付け直す。
    #[test]
    fn preserves_text_of_diff_lines() {
        let lines = vec![
            line(DiffLineKind::HunkHeader, "@@ -1,3 +1,3 @@ fn main() {\n"),
            line(DiffLineKind::Deletion, "    let x = 0;\n"),
            line(DiffLineKind::Addition, "    let x = 1;\n"),
            line(DiffLineKind::Context, "    println!(\"{x}\");\n"),
        ];

        let out = highlighter().highlight_diff(&lines, Some("src/main.rs"));
        let rendered: Vec<String> = out.iter().map(render).collect();

        assert_eq!(
            rendered,
            vec![
                "@@ -1,3 +1,3 @@ fn main() {\n",
                "-    let x = 0;\n",
                "+    let x = 1;\n",
                "     println!(\"{x}\");\n",
            ]
        );
    }

    /// 追加行・削除行・文脈行には unified diff と同じ行頭マーカーを付ける。
    ///
    /// libgit2 はマーカーを `line.origin()` として別に返すため
    /// `DiffLine::content` には含まれない（src/git.rs 参照）。マーカーが無いと
    /// 追加と削除を背景色でしか区別できず、色を落とした端末では判別できない。
    #[test]
    fn prefixes_lines_with_diff_marker() {
        let lines = vec![
            line(DiffLineKind::Deletion, "let x = 0;\n"),
            line(DiffLineKind::Addition, "let x = 1;\n"),
            line(DiffLineKind::Context, "let y = 2;\n"),
        ];

        let out = highlighter().highlight_diff(&lines, Some("src/main.rs"));
        let rendered: Vec<String> = out.iter().map(render).collect();

        assert_eq!(
            rendered,
            vec!["-let x = 0;\n", "+let x = 1;\n", " let y = 2;\n"]
        );
    }

    /// 削除行と追加行は同じ箇所の旧版と新版であって、連続したソースコードでは
    /// ない。旧版で開いたままの文字列リテラルが新版のハイライトに漏れると、
    /// そこから下の配色がすべてずれる。
    #[test]
    fn deletion_does_not_leak_parser_state_into_addition() {
        let addition = line(DiffLineKind::Addition, "let ok = 1;\n");
        let deletion_with_open_string = line(DiffLineKind::Deletion, "let s = \"unterminated;\n");

        let alone = highlighter().highlight_diff(std::slice::from_ref(&addition), Some("a.rs"));
        let after_deletion =
            highlighter().highlight_diff(&[deletion_with_open_string, addition], Some("a.rs"));

        assert_eq!(after_deletion[1].spans, alone[0].spans);
    }

    /// ハンクヘッダは diff 自身の書式であってソースコードではない。
    /// 構文パーサに通すと、末尾に載る関数シグネチャの括弧や引用符が状態に
    /// 入り、そこから下の行の配色がずれる。
    #[test]
    fn does_not_parse_headers_as_source_code() {
        let header = line(DiffLineKind::HunkHeader, "@@ -1,3 +1,3 @@ fn main() {\n");
        let context = line(DiffLineKind::Context, "let x = 1;\n");

        let alone = highlighter().highlight_diff(std::slice::from_ref(&context), Some("a.rs"));
        let after_header = highlighter().highlight_diff(&[header, context], Some("a.rs"));

        // ヘッダは一様な書式で描かれ、構文で分割されない
        assert_eq!(after_header[0].spans.len(), 1);
        // 後続行のハイライトはヘッダの有無で変わらない
        assert_eq!(after_header[1].spans, alone[0].spans);
    }

    /// 文脈行は旧版にも新版にも含まれる。片側にしか流さないと、文脈行で
    /// 開いたブロックコメントがもう片側に反映されず、同じ内容の削除行と
    /// 追加行で配色が食い違う。
    #[test]
    fn context_advances_both_sides() {
        let inside = "still inside the comment\n";

        let out = highlighter().highlight_diff(
            &[
                line(DiffLineKind::Context, "/* comment starts here\n"),
                line(DiffLineKind::Deletion, inside),
                line(DiffLineKind::Addition, inside),
            ],
            Some("a.rs"),
        );

        // 背景色は行種別ごとに違うので、構文の解釈（前景色）だけを比べる
        assert_eq!(body_syntax(&out[1]), body_syntax(&out[2]));
    }

    /// ハンクが変われば、前のハンクで開いたままの構文状態は次のハンクには
    /// 続かない。ヘッダを構文パーサに通さないだけでは状態が残ってしまう。
    #[test]
    fn hunk_header_resets_parser_state() {
        let code = line(DiffLineKind::Context, "plain code\n");

        let alone = highlighter().highlight_diff(std::slice::from_ref(&code), Some("a.rs"));
        let after_new_hunk = highlighter().highlight_diff(
            &[
                line(DiffLineKind::Context, "let s = \"unterminated;\n"),
                line(DiffLineKind::HunkHeader, "@@ -10,3 +10,3 @@\n"),
                code,
            ],
            Some("a.rs"),
        );

        assert_eq!(after_new_hunk[2].spans, alone[0].spans);
    }

    /// 横スクロールの上限は実際に描かれる幅で決まる。マーカーは描画時に
    /// 足されるので、行の幅にも含まれていなければ最長行の右端 1 文字を
    /// 画面内に出せない。
    #[test]
    fn display_width_includes_marker() {
        let out =
            highlighter().highlight_diff(&[line(DiffLineKind::Addition, "abc\n")], Some("a.rs"));

        assert_eq!(out[0].display_width(), 4);
    }

    /// 未知の拡張子ではプレーンテキストにフォールバックし、内容を保つこと。
    #[test]
    fn falls_back_to_plain_text_for_unknown_extension() {
        let lines = vec![line(DiffLineKind::Context, "hello\n")];
        let out = highlighter().highlight_diff(&lines, Some("data.unknownext"));
        assert_eq!(out.len(), 1);
        assert_eq!(render(&out[0]), " hello\n");
    }

    /// パスが無い場合（拡張子なし・選択なし）も落ちず、内容を保つこと。
    #[test]
    fn handles_missing_file_path() {
        let lines = vec![line(DiffLineKind::Context, "plain\n")];
        let out = highlighter().highlight_diff(&lines, None);
        assert_eq!(out.len(), 1);
        assert_eq!(render(&out[0]), " plain\n");
    }
}
