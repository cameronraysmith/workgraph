//! Markdown-to-ratatui renderer.
//!
//! Converts markdown text into styled `Vec<Line<'static>>` suitable for
//! rendering in ratatui `Paragraph` widgets. Uses pulldown-cmark for parsing
//! and syntect for fenced code block highlighting.

use std::sync::OnceLock;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const COLOR_H1: Color = Color::Indexed(75);
const COLOR_H2: Color = Color::Indexed(114);
const COLOR_H3: Color = Color::Indexed(180);
const COLOR_H4: Color = Color::Indexed(174);
const COLOR_H5: Color = Color::Indexed(139);
const COLOR_H6: Color = Color::Indexed(109);
const COLOR_INLINE_CODE_FG: Color = Color::Indexed(222);
const COLOR_INLINE_CODE_BG: Color = Color::Indexed(236);
const COLOR_CODE_BLOCK_BG: Color = Color::Indexed(235);
const COLOR_CODE_BLOCK_BAR: Color = Color::Indexed(240);
const COLOR_LINK: Color = Color::Indexed(75);
const COLOR_LINK_URL: Color = Color::Indexed(245);
const COLOR_BLOCKQUOTE_BAR: Color = Color::Indexed(244);
const COLOR_BLOCKQUOTE_TEXT: Color = Color::Indexed(250);
const COLOR_RULE: Color = Color::Indexed(240);
const BULLETS: &[char] = &['•', '◦', '▸'];

struct SyntectAssets {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

fn syntect_assets() -> &'static SyntectAssets {
    static ASSETS: OnceLock<SyntectAssets> = OnceLock::new();
    ASSETS.get_or_init(|| SyntectAssets {
        syntax_set: SyntaxSet::load_defaults_newlines(),
        theme_set: ThemeSet::load_defaults(),
    })
}

/// Convert a markdown string into styled ratatui [`Line`]s.
///
/// `width` is the available character width for horizontal rules.
pub fn markdown_to_lines(md: &str, width: usize) -> Vec<Line<'static>> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut renderer = MdRenderer::new(width);
    for event in parser {
        renderer.handle_event(event);
    }
    renderer.finish()
}

struct MdRenderer {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<Option<u64>>,
    in_code_block: bool,
    code_lang: Option<String>,
    code_buffer: String,
    width: usize,
    blockquote_depth: usize,
    heading_level: Option<HeadingLevel>,
    link_url: Option<String>,
}

impl MdRenderer {
    fn new(width: usize) -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default()],
            list_stack: Vec::new(),
            in_code_block: false,
            code_lang: None,
            code_buffer: String::new(),
            width,
            blockquote_depth: 0,
            heading_level: None,
            link_url: None,
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, modifier: impl FnOnce(Style) -> Style) {
        let new = modifier(self.current_style());
        self.style_stack.push(new);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        let spans = std::mem::take(&mut self.current_spans);
        if self.blockquote_depth > 0 {
            let mut prefixed = Vec::with_capacity(spans.len() + 1);
            let bar = "▎ ".repeat(self.blockquote_depth);
            prefixed.push(Span::styled(bar, Style::default().fg(COLOR_BLOCKQUOTE_BAR)));
            prefixed.extend(spans);
            self.lines.push(Line::from(prefixed));
        } else {
            self.lines.push(Line::from(spans));
        }
    }

    fn blank_line(&mut self) {
        self.flush_line();
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                self.heading_level = Some(level);
                let color = heading_color(level);
                self.push_style(|s| s.fg(color).add_modifier(Modifier::BOLD));
            }
            Event::Start(Tag::Paragraph) => {
                if !self.lines.is_empty() && !self.in_code_block {
                    self.blank_line();
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                self.blockquote_depth += 1;
                self.push_style(|s| s.fg(COLOR_BLOCKQUOTE_TEXT));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                self.in_code_block = true;
                self.code_buffer.clear();
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string();
                        if l.is_empty() { None } else { Some(l) }
                    }
                    CodeBlockKind::Indented => None,
                };
                if !self.lines.is_empty() {
                    self.blank_line();
                }
            }
            Event::Start(Tag::List(start)) => {
                if self.list_stack.is_empty() {
                    if !self.lines.is_empty() {
                        self.blank_line();
                    }
                } else if !self.current_spans.is_empty() {
                    self.flush_line();
                }
                self.list_stack.push(start);
            }
            Event::Start(Tag::Item) => {
                let depth = self.list_stack.len().saturating_sub(1);
                let indent = "  ".repeat(depth);
                let marker = match self.list_stack.last() {
                    Some(Some(n)) => {
                        let marker = format!("{indent}{}. ", n);
                        if let Some(Some(counter)) = self.list_stack.last_mut() {
                            *counter += 1;
                        }
                        marker
                    }
                    _ => {
                        let bullet = BULLETS[depth % BULLETS.len()];
                        format!("{indent}{bullet} ")
                    }
                };
                self.current_spans
                    .push(Span::styled(marker, Style::default()));
            }
            Event::Start(Tag::Emphasis) => {
                self.push_style(|s| s.add_modifier(Modifier::ITALIC));
            }
            Event::Start(Tag::Strong) => {
                self.push_style(|s| s.add_modifier(Modifier::BOLD));
            }
            Event::Start(Tag::Strikethrough) => {
                self.push_style(|s| s.add_modifier(Modifier::CROSSED_OUT));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                self.link_url = Some(dest_url.to_string());
                self.push_style(|s| s.fg(COLOR_LINK).add_modifier(Modifier::UNDERLINED));
            }
            Event::End(TagEnd::Heading(_level)) => {
                self.flush_line();
                self.pop_style();
                self.heading_level = None;
            }
            Event::End(TagEnd::Paragraph) => {
                self.flush_line();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                self.flush_line();
                self.pop_style();
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
            }
            Event::End(TagEnd::CodeBlock) => {
                self.emit_code_block();
                self.in_code_block = false;
                self.code_lang = None;
            }
            Event::End(TagEnd::List(_)) => {
                self.list_stack.pop();
            }
            Event::End(TagEnd::Item) => {
                if !self.current_spans.is_empty() {
                    self.flush_line();
                }
            }
            Event::End(TagEnd::Emphasis) => {
                self.pop_style();
            }
            Event::End(TagEnd::Strong) => {
                self.pop_style();
            }
            Event::End(TagEnd::Strikethrough) => {
                self.pop_style();
            }
            Event::End(TagEnd::Link) => {
                self.pop_style();
                if let Some(url) = self.link_url.take()
                    && !url.is_empty()
                {
                    self.current_spans.push(Span::styled(
                        format!(" ({url})"),
                        Style::default().fg(COLOR_LINK_URL),
                    ));
                }
            }
            Event::Text(text) => {
                if self.in_code_block {
                    self.code_buffer.push_str(&text);
                } else {
                    let style = self.current_style();
                    self.current_spans
                        .push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                let style = Style::default()
                    .fg(COLOR_INLINE_CODE_FG)
                    .bg(COLOR_INLINE_CODE_BG);
                self.current_spans
                    .push(Span::styled(format!(" {code} "), style));
            }
            Event::SoftBreak => {
                if self.in_code_block {
                    self.code_buffer.push('\n');
                } else {
                    let style = self.current_style();
                    self.current_spans
                        .push(Span::styled(" ".to_string(), style));
                }
            }
            Event::HardBreak => {
                self.flush_line();
            }
            Event::Rule => {
                if !self.lines.is_empty() {
                    self.blank_line();
                }
                let rule_width = self.width.min(120);
                let rule = "━".repeat(rule_width);
                self.lines.push(Line::from(Span::styled(
                    rule,
                    Style::default().fg(COLOR_RULE),
                )));
            }
            _ => {}
        }
    }

    fn emit_code_block(&mut self) {
        let assets = syntect_assets();
        let theme = &assets.theme_set.themes["base16-ocean.dark"];
        let bar_span = Span::styled("│ ", Style::default().fg(COLOR_CODE_BLOCK_BAR));
        let bg_style = Style::default().bg(COLOR_CODE_BLOCK_BG);
        let syntax = self
            .code_lang
            .as_deref()
            .and_then(|lang| assets.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| assets.syntax_set.find_syntax_plain_text());
        let mut highlighter = HighlightLines::new(syntax, theme);
        for line_text in LinesWithEndings::from(&self.code_buffer) {
            let mut spans = vec![bar_span.clone()];
            match highlighter.highlight_line(line_text, &assets.syntax_set) {
                Ok(ranges) => {
                    for (style, text) in ranges {
                        let trimmed = text.trim_end_matches('\n');
                        if trimmed.is_empty() && text.contains('\n') {
                            continue;
                        }
                        match syntect_tui::into_span((style, trimmed)) {
                            Ok(span) => {
                                let mut s = span.style;
                                s = s.bg(COLOR_CODE_BLOCK_BG);
                                spans.push(Span::styled(span.content.into_owned(), s));
                            }
                            Err(_) => {
                                spans.push(Span::styled(trimmed.to_owned(), bg_style));
                            }
                        }
                    }
                }
                Err(_) => {
                    let trimmed = line_text.trim_end_matches('\n');
                    spans.push(Span::styled(trimmed.to_owned(), bg_style));
                }
            }
            self.lines.push(Line::from(spans));
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        if !self.current_spans.is_empty() {
            self.flush_line();
        }
        self.lines
    }
}

fn heading_color(level: HeadingLevel) -> Color {
    match level {
        HeadingLevel::H1 => COLOR_H1,
        HeadingLevel::H2 => COLOR_H2,
        HeadingLevel::H3 => COLOR_H3,
        HeadingLevel::H4 => COLOR_H4,
        HeadingLevel::H5 => COLOR_H5,
        HeadingLevel::H6 => COLOR_H6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn has_modifier(line: &Line, modifier: Modifier) -> bool {
        line.spans
            .iter()
            .any(|s| s.style.add_modifier.contains(modifier))
    }

    fn has_fg_color(line: &Line, color: Color) -> bool {
        line.spans.iter().any(|s| s.style.fg == Some(color))
    }

    #[test]
    fn test_bold_text() {
        let lines = markdown_to_lines("**bold**", 80);
        assert_eq!(lines.len(), 1);
        assert!(has_modifier(&lines[0], Modifier::BOLD));
    }

    #[test]
    fn test_italic_text() {
        let lines = markdown_to_lines("*italic*", 80);
        assert_eq!(lines.len(), 1);
        assert!(has_modifier(&lines[0], Modifier::ITALIC));
    }

    #[test]
    fn test_heading_levels() {
        for (md, color) in [
            ("# H1", COLOR_H1),
            ("## H2", COLOR_H2),
            ("### H3", COLOR_H3),
            ("#### H4", COLOR_H4),
            ("##### H5", COLOR_H5),
            ("###### H6", COLOR_H6),
        ] {
            let lines = markdown_to_lines(md, 80);
            assert!(!lines.is_empty());
            assert!(has_fg_color(&lines[0], color));
            assert!(has_modifier(&lines[0], Modifier::BOLD));
        }
    }

    #[test]
    fn test_unordered_list() {
        let lines = markdown_to_lines("- first\n- second", 80);
        assert_eq!(lines.len(), 2);
        assert!(line_text(&lines[0]).contains("• first"));
    }

    #[test]
    fn test_empty_input() {
        let lines = markdown_to_lines("", 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_plain_text() {
        let lines = markdown_to_lines("just text", 80);
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "just text");
    }

    #[test]
    fn test_code_block() {
        let lines = markdown_to_lines("```rust\nfn main() {}\n```", 80);
        assert!(!lines.is_empty());
        let text = line_text(&lines[lines.len() - 1]);
        assert!(text.contains('│'));
    }
}
