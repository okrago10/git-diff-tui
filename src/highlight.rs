use ratatui::style::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SyntectColor, FontStyle, Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::git::{DiffLine, DiffLineKind};

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

pub struct HighlightedLine {
    pub spans: Vec<(ratatui::style::Style, String)>,
}

/// テーマが前景色・背景色を持たない場合の代替。
const DEFAULT_FOREGROUND: SyntectColor = SyntectColor {
    r: 0xc0,
    g: 0xc5,
    b: 0xce,
    a: 0xff,
};
const DEFAULT_BACKGROUND: SyntectColor = SyntectColor {
    r: 0x2b,
    g: 0x30,
    b: 0x3b,
    a: 0xff,
};

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

    pub fn highlight_diff(&self, lines: &[DiffLine], file_path: Option<&str>) -> Vec<HighlightedLine> {
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
        // ヘッダ行はソースコードではないので、テーマの既定色で一様に描く。
        let header_style = Style {
            foreground: theme.settings.foreground.unwrap_or(DEFAULT_FOREGROUND),
            background: theme.settings.background.unwrap_or(DEFAULT_BACKGROUND),
            font_style: FontStyle::empty(),
        };

        lines
            .iter()
            .map(|line| {
                let bg = match line.kind {
                    DiffLineKind::Addition => Some(BG_ADDITION),
                    DiffLineKind::Deletion => Some(BG_DELETION),
                    DiffLineKind::HunkHeader => Some(BG_HUNK_HEADER),
                    _ => None,
                };

                let highlighted = match line.kind {
                    DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                        Ok(vec![(header_style, line.content.as_str())])
                    }
                    DiffLineKind::Deletion => {
                        old_side.highlight_line(&line.content, &self.syntax_set)
                    }
                    DiffLineKind::Context => {
                        // 文脈行は旧版にも新版にも属するので、両方の状態を進める
                        let _ = old_side.highlight_line(&line.content, &self.syntax_set);
                        new_side.highlight_line(&line.content, &self.syntax_set)
                    }
                    DiffLineKind::Addition => {
                        new_side.highlight_line(&line.content, &self.syntax_set)
                    }
                }
                .unwrap_or_default();

                let mut spans = Vec::with_capacity(highlighted.len() + 1);

                // 行頭マーカー（unified diff と同じ +/-/空白）
                let marker = marker_for(line.kind);
                if !marker.is_empty() {
                    let marker_style = ratatui::style::Style::default()
                        .fg(match line.kind {
                            DiffLineKind::Addition => Color::Green,
                            DiffLineKind::Deletion => Color::Red,
                            _ => Color::White,
                        })
                        .bg(bg.unwrap_or(Color::Reset));
                    spans.push((marker_style, marker.to_string()));
                }

                // highlighted code spans
                for (style, text) in highlighted {
                    let ratatui_style = syntect_to_ratatui_style(style, bg);
                    spans.push((ratatui_style, text.to_string()));
                }

                HighlightedLine { spans }
            })
            .collect()
    }
}

/// unified diff の行頭マーカー。ヘッダ行はそれ自体が書式を持つので付けない。
fn marker_for(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Addition => "+",
        DiffLineKind::Deletion => "-",
        DiffLineKind::Context => " ",
        DiffLineKind::HunkHeader | DiffLineKind::FileHeader => "",
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
            DiffLine {
                kind: DiffLineKind::HunkHeader,
                content: "@@ -1,3 +1,3 @@ fn main() {\n".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deletion,
                content: "    let x = 0;\n".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                content: "    let x = 1;\n".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                content: "    println!(\"{x}\");\n".to_string(),
            },
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
            DiffLine {
                kind: DiffLineKind::Deletion,
                content: "let x = 0;\n".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Addition,
                content: "let x = 1;\n".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                content: "let y = 2;\n".to_string(),
            },
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
        let addition = DiffLine {
            kind: DiffLineKind::Addition,
            content: "let ok = 1;\n".to_string(),
        };
        let deletion_with_open_string = DiffLine {
            kind: DiffLineKind::Deletion,
            content: "let s = \"unterminated;\n".to_string(),
        };

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
        let header = DiffLine {
            kind: DiffLineKind::HunkHeader,
            content: "@@ -1,3 +1,3 @@ fn main() {\n".to_string(),
        };
        let context = DiffLine {
            kind: DiffLineKind::Context,
            content: "let x = 1;\n".to_string(),
        };

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
        let comment_opens = DiffLine {
            kind: DiffLineKind::Context,
            content: "/* comment starts here\n".to_string(),
        };
        let inside = "still inside the comment\n".to_string();

        let out = highlighter().highlight_diff(
            &[
                comment_opens,
                DiffLine {
                    kind: DiffLineKind::Deletion,
                    content: inside.clone(),
                },
                DiffLine {
                    kind: DiffLineKind::Addition,
                    content: inside,
                },
            ],
            Some("a.rs"),
        );

        // 背景色は行種別ごとに違うので、構文の解釈（前景色）だけを比べる
        assert_eq!(body_syntax(&out[1]), body_syntax(&out[2]));
    }

    /// 未知の拡張子ではプレーンテキストにフォールバックし、内容を保つこと。
    #[test]
    fn falls_back_to_plain_text_for_unknown_extension() {
        let lines = vec![DiffLine {
            kind: DiffLineKind::Context,
            content: "hello\n".to_string(),
        }];
        let out = highlighter().highlight_diff(&lines, Some("data.unknownext"));
        assert_eq!(out.len(), 1);
        assert_eq!(render(&out[0]), " hello\n");
    }

    /// パスが無い場合（拡張子なし・選択なし）も落ちず、内容を保つこと。
    #[test]
    fn handles_missing_file_path() {
        let lines = vec![DiffLine {
            kind: DiffLineKind::Context,
            content: "plain\n".to_string(),
        }];
        let out = highlighter().highlight_diff(&lines, None);
        assert_eq!(out.len(), 1);
        assert_eq!(render(&out[0]), " plain\n");
    }
}
