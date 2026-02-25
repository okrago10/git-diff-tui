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
