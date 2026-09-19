use ratatui::style::Color;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::git::{DiffLine, DiffLineKind};

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

pub struct HighlightedLine {
    pub spans: Vec<(ratatui::style::Style, String)>,
}

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
        let mut h = syntect::easy::HighlightLines::new(syntax, theme);

        lines
            .iter()
            .map(|line| {
                let bg = match line.kind {
                    DiffLineKind::Addition => Some(BG_ADDITION),
                    DiffLineKind::Deletion => Some(BG_DELETION),
                    DiffLineKind::HunkHeader => Some(BG_HUNK_HEADER),
                    _ => None,
                };

                // strip leading +/- for syntax highlighting, then restore prefix
                let (prefix, code) = match line.kind {
                    DiffLineKind::Addition if line.content.starts_with('+') => {
                        ("+", &line.content[1..])
                    }
                    DiffLineKind::Deletion if line.content.starts_with('-') => {
                        ("-", &line.content[1..])
                    }
                    _ => ("", line.content.as_str()),
                };

                let highlighted = h
                    .highlight_line(code, &self.syntax_set)
                    .unwrap_or_default();

                let mut spans = Vec::with_capacity(highlighted.len() + 1);

                // prefix (+/-)
                if !prefix.is_empty() {
                    let prefix_style = ratatui::style::Style::default()
                        .fg(match line.kind {
                            DiffLineKind::Addition => Color::Green,
                            DiffLineKind::Deletion => Color::Red,
                            _ => Color::White,
                        })
                        .bg(bg.unwrap_or(Color::Reset));
                    spans.push((prefix_style, prefix.to_string()));
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

fn syntect_to_ratatui_style(style: Style, bg_override: Option<Color>) -> ratatui::style::Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let bg = bg_override
        .unwrap_or_else(|| Color::Rgb(style.background.r, style.background.g, style.background.b));
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

    fn render(line: &super::HighlightedLine) -> String {
        line.spans.iter().map(|(_, t)| t.as_str()).collect()
    }

    /// ハイライトを通してもテキストが失われないこと。
    ///
    /// フィクスチャは実際の `GitRepo::file_diff` の出力に合わせてある。
    /// libgit2 は `+` / `-` を `line.origin()` として別に返すため、
    /// `DiffLine::content` には行頭マーカーが含まれない（src/git.rs 参照）。
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
                "    let x = 0;\n",
                "    let x = 1;\n",
                "    println!(\"{x}\");\n",
            ]
        );
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
        assert_eq!(render(&out[0]), "hello\n");
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
        assert_eq!(render(&out[0]), "plain\n");
    }
}
