use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Addition(String),
    Deletion(String),
    Context(String),
    Header(String),
}

#[derive(Debug, Clone)]
pub struct DiffViewState {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
    pub scroll_offset: usize,
    pub title: String,
    pub old_content: String,
    pub new_content: String,
    pub is_pending_review: bool,
}

impl DiffViewState {
    pub fn new(
        file_path: &str,
        old_content: &str,
        new_content: &str,
        is_pending_review: bool,
    ) -> Self {
        let lines = DiffView::compute_unified_diff(old_content, new_content, file_path);
        let title = if is_pending_review {
            format!(" Review Pending Diff: {} ", file_path)
        } else {
            format!(" Diff Visualizer: {} ", file_path)
        };
        Self {
            file_path: file_path.to_string(),
            lines,
            scroll_offset: 0,
            title,
            old_content: old_content.to_string(),
            new_content: new_content.to_string(),
            is_pending_review,
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, visible_height: usize) {
        let max_scroll = self.lines.len().saturating_sub(visible_height);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn scroll_home(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_end(&mut self, visible_height: usize) {
        self.scroll_offset = self.lines.len().saturating_sub(visible_height);
    }
}

pub struct DiffView;

#[derive(Clone, Debug, PartialEq, Eq)]
enum DiffOp<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

impl DiffView {
    /// Compute unified diff hunks with line-by-line classification
    pub fn compute_unified_diff(
        old_content: &str,
        new_content: &str,
        file_path: &str,
    ) -> Vec<DiffLine> {
        let mut result = vec![
            DiffLine::Header(format!("--- a/{}", file_path)),
            DiffLine::Header(format!("+++ b/{}", file_path)),
        ];

        if old_content == new_content {
            return result;
        }

        let old_lines: Vec<&str> = if old_content.is_empty() {
            Vec::new()
        } else {
            old_content
                .lines()
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .collect()
        };

        let new_lines: Vec<&str> = if new_content.is_empty() {
            Vec::new()
        } else {
            new_content
                .lines()
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .collect()
        };

        let n = old_lines.len();
        let m = new_lines.len();

        let ops = if n > 1000 || m > 1000 || n * m > 1_000_000 {
            // Fast prefix/suffix diff for large files
            let mut ops = Vec::new();
            let mut start_eq = 0;
            while start_eq < n && start_eq < m && old_lines[start_eq] == new_lines[start_eq] {
                ops.push(DiffOp::Equal(old_lines[start_eq]));
                start_eq += 1;
            }
            let mut old_end = n;
            let mut new_end = m;
            while old_end > start_eq
                && new_end > start_eq
                && old_lines[old_end - 1] == new_lines[new_end - 1]
            {
                old_end -= 1;
                new_end -= 1;
            }
            for line in &old_lines[start_eq..old_end] {
                ops.push(DiffOp::Delete(line));
            }
            for line in &new_lines[start_eq..new_end] {
                ops.push(DiffOp::Insert(line));
            }
            for line in &old_lines[old_end..n] {
                ops.push(DiffOp::Equal(line));
            }
            ops
        } else {
            // Standard LCS table calculation
            let mut dp = vec![vec![0u32; m + 1]; n + 1];
            for i in 0..n {
                for j in 0..m {
                    if old_lines[i] == new_lines[j] {
                        dp[i + 1][j + 1] = dp[i][j] + 1;
                    } else {
                        dp[i + 1][j + 1] = dp[i][j].max(dp[i + 1][j]);
                    }
                }
            }

            // Backtrack to extract edit script
            let mut i = n;
            let mut j = m;
            let mut script = Vec::new();
            while i > 0 || j > 0 {
                if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
                    script.push(DiffOp::Equal(old_lines[i - 1]));
                    i -= 1;
                    j -= 1;
                } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
                    script.push(DiffOp::Insert(new_lines[j - 1]));
                    j -= 1;
                } else if i > 0 {
                    script.push(DiffOp::Delete(old_lines[i - 1]));
                    i -= 1;
                }
            }
            script.reverse();
            script
        };

        // 3 lines of context around change hunks
        const CONTEXT_SIZE: usize = 3;

        let change_indices: Vec<usize> = ops
            .iter()
            .enumerate()
            .filter_map(|(idx, op)| match op {
                DiffOp::Delete(_) | DiffOp::Insert(_) => Some(idx),
                DiffOp::Equal(_) => None,
            })
            .collect();

        if change_indices.is_empty() {
            return result;
        }

        let mut hunk_ranges: Vec<(usize, usize)> = Vec::new();
        for &idx in &change_indices {
            let start = idx.saturating_sub(CONTEXT_SIZE);
            let end = (idx + CONTEXT_SIZE).min(ops.len().saturating_sub(1));
            if let Some(last) = hunk_ranges.last_mut()
                && start <= last.1 + 1
            {
                last.1 = last.1.max(end);
                continue;
            }
            hunk_ranges.push((start, end));
        }

        for (start, end) in hunk_ranges {
            let mut old_start = 1;
            let mut new_start = 1;
            for op in &ops[0..start] {
                match op {
                    DiffOp::Equal(_) => {
                        old_start += 1;
                        new_start += 1;
                    }
                    DiffOp::Delete(_) => {
                        old_start += 1;
                    }
                    DiffOp::Insert(_) => {
                        new_start += 1;
                    }
                }
            }

            let mut old_count = 0;
            let mut new_count = 0;
            for op in &ops[start..=end] {
                match op {
                    DiffOp::Equal(_) => {
                        old_count += 1;
                        new_count += 1;
                    }
                    DiffOp::Delete(_) => {
                        old_count += 1;
                    }
                    DiffOp::Insert(_) => {
                        new_count += 1;
                    }
                }
            }

            if old_count == 0 {
                old_start = 0;
            }
            if new_count == 0 {
                new_start = 0;
            }

            result.push(DiffLine::Header(format!(
                "@@ -{},{} +{},{} @@",
                old_start, old_count, new_start, new_count
            )));

            for op in &ops[start..=end] {
                match op {
                    DiffOp::Equal(line) => {
                        result.push(DiffLine::Context(format!(" {}", line)));
                    }
                    DiffOp::Delete(line) => {
                        result.push(DiffLine::Deletion(format!("-{}", line)));
                    }
                    DiffOp::Insert(line) => {
                        result.push(DiffLine::Addition(format!("+{}", line)));
                    }
                }
            }
        }

        result
    }

    /// Render interactive visual diff overlay into the given Ratatui frame area (theme-native)
    pub fn render(state: &DiffViewState, f: &mut Frame, area: Rect) {
        Self::render_with_theme(state, f, area, &crate::style::ThemePalette::default())
    }

    pub fn render_with_theme(
        state: &DiffViewState,
        f: &mut Frame,
        area: Rect,
        theme: &crate::style::ThemePalette,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border))
            .title(format!(
                " {} [Line {}/{} | Offset {}] ",
                state.title.trim(),
                if state.lines.is_empty() {
                    0
                } else {
                    state.scroll_offset.min(state.lines.len() - 1) + 1
                },
                state.lines.len(),
                state.scroll_offset
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let content_area = chunks[0];
        let visible_lines = content_area.height as usize;
        let total_lines = state.lines.len();

        let mut rendered_lines: Vec<Line> = Vec::new();

        if total_lines == 0 || (total_lines == 2 && state.old_content == state.new_content) {
            rendered_lines.push(Line::from(vec![Span::styled(
                "  (No differences detected)",
                Style::default().fg(Color::DarkGray),
            )]));
        } else {
            let start = state.scroll_offset.min(total_lines);
            let end = (start + visible_lines).min(total_lines);

            for (idx, diff_line) in state.lines[start..end].iter().enumerate() {
                let line_num = start + idx + 1;
                let line_num_str = format!("{:4} │ ", line_num);

                match diff_line {
                    DiffLine::Header(hdr) => {
                        rendered_lines.push(Line::from(vec![
                            Span::styled(
                                format!("{:4} │ ", "---"),
                                Style::default().fg(theme.muted),
                            ),
                            Span::styled(
                                hdr.clone(),
                                Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                    DiffLine::Addition(text) => {
                        let display_text = if text.starts_with('+') {
                            text.clone()
                        } else {
                            format!("+{}", text)
                        };
                        rendered_lines.push(Line::from(vec![
                            Span::styled(line_num_str, Style::default().fg(theme.muted)),
                            Span::styled(display_text, Style::default().fg(theme.green)),
                        ]));
                    }
                    DiffLine::Deletion(text) => {
                        let display_text = if text.starts_with('-') {
                            text.clone()
                        } else {
                            format!("-{}", text)
                        };
                        rendered_lines.push(Line::from(vec![
                            Span::styled(line_num_str, Style::default().fg(theme.muted)),
                            Span::styled(display_text, Style::default().fg(theme.red)),
                        ]));
                    }
                    DiffLine::Context(text) => {
                        let display_text = if text.starts_with(' ') {
                            text.clone()
                        } else {
                            format!(" {}", text)
                        };
                        rendered_lines.push(Line::from(vec![
                            Span::styled(line_num_str, Style::default().fg(theme.muted)),
                            Span::styled(display_text, Style::default().fg(theme.text)),
                        ]));
                    }
                }
            }
        }

        let diff_paragraph = Paragraph::new(rendered_lines).wrap(Wrap { trim: false });
        f.render_widget(diff_paragraph, content_area);

        // Separator line — theme-native, adaptive width (no hardcoded 80)
        let sep_width = chunks[1].width as usize;
        let sep_str = "─".repeat(sep_width);
        let sep_line = Paragraph::new(sep_str).style(Style::default().fg(theme.border));
        f.render_widget(sep_line, chunks[1]);

        // Keybinding footer
        let footer_spans = if state.is_pending_review {
            vec![
                Span::styled(
                    " [y/Enter: Accept] ",
                    Style::default()
                        .fg(theme.green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [n: Reject] ",
                    Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" [Esc/q: Close] ", Style::default().fg(theme.yellow)),
                Span::styled(" [↑/↓/PgUp/PgDn: Scroll] ", Style::default().fg(theme.cyan)),
            ]
        } else {
            vec![
                Span::styled(
                    " [Esc/q: Close] ",
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [↑/↓/j/k/PgUp/PgDn: Scroll] ",
                    Style::default().fg(theme.cyan),
                ),
                Span::styled(" [Home/End: Top/Bottom] ", Style::default().fg(theme.muted)),
            ]
        };
        let footer = Paragraph::new(Line::from(footer_spans));
        f.render_widget(footer, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_content_diff() {
        let content = "fn main() {\n    println!(\"Hello\");\n}\n";
        let diff = DiffView::compute_unified_diff(content, content, "src/main.rs");
        assert_eq!(diff.len(), 2);
        assert_eq!(diff[0], DiffLine::Header("--- a/src/main.rs".to_string()));
        assert_eq!(diff[1], DiffLine::Header("+++ b/src/main.rs".to_string()));
    }

    #[test]
    fn test_addition_and_deletion_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2_modified\nline3";
        let diff = DiffView::compute_unified_diff(old, new, "test.txt");

        assert!(diff.len() >= 5);
        assert!(matches!(&diff[0], DiffLine::Header(h) if h.contains("--- a/test.txt")));
        assert!(matches!(&diff[1], DiffLine::Header(h) if h.contains("+++ b/test.txt")));
        assert!(matches!(&diff[2], DiffLine::Header(h) if h.contains("@@ -1,3 +1,3 @@")));
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Deletion(d) if d == "-line2"))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a == "+line2_modified"))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Context(c) if c == " line1"))
        );
    }

    #[test]
    fn test_new_file_creation_diff() {
        let old = "";
        let new = "hello world\nsecond line";
        let diff = DiffView::compute_unified_diff(old, new, "new.rs");

        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a == "+hello world"))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a == "+second line"))
        );
    }

    #[test]
    fn test_file_deletion_diff() {
        let old = "deleted content\nall gone";
        let new = "";
        let diff = DiffView::compute_unified_diff(old, new, "deleted.rs");

        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Deletion(d) if d == "-deleted content"))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Deletion(d) if d == "-all gone"))
        );
    }

    #[test]
    fn test_unicode_and_emojis_diff() {
        let old = "🚀 Launching system...\n🦀 Rust is fast!";
        let new = "🚀 Launching system...\n🦀 Rust 2024 is blazing fast!\n🎉 Done";
        let diff = DiffView::compute_unified_diff(old, new, "unicode.md");

        // Verify UTF-8 safety
        for line in &diff {
            match line {
                DiffLine::Addition(s)
                | DiffLine::Deletion(s)
                | DiffLine::Context(s)
                | DiffLine::Header(s) => {
                    let boundary = s.floor_char_boundary(s.len());
                    assert_eq!(boundary, s.len());
                }
            }
        }

        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a.contains("🦀 Rust 2024")))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a.contains("🎉 Done")))
        );
    }

    #[test]
    fn test_diff_view_state_navigation() {
        let mut state = DiffViewState::new("test.rs", "old", "new", true);
        assert_eq!(state.scroll_offset, 0);
        assert!(state.is_pending_review);
        assert!(state.title.contains("Review Pending Diff"));

        state.scroll_down(5, 2);
        assert!(state.scroll_offset > 0);

        state.scroll_home();
        assert_eq!(state.scroll_offset, 0);

        state.scroll_end(2);
        assert!(state.scroll_offset <= state.lines.len());

        state.scroll_up(2);
    }

    #[test]
    fn test_large_file_prefix_suffix_diff_fallback() {
        // Create 1200 lines to trigger the n > 1000 branch (O(N+M) prefix/suffix fallback)
        let old_lines: Vec<String> = (0..1200).map(|i| format!("line_{:04}", i)).collect();
        let mut new_lines = old_lines.clone();
        // Modify a few lines in the middle
        new_lines[500] = "line_0500_MODIFIED".to_string();
        new_lines[501] = "line_0501_MODIFIED".to_string();
        // Insert a new line
        new_lines.insert(600, "line_inserted_xyz".to_string());

        let old_str = old_lines.join("\n");
        let new_str = new_lines.join("\n");

        let diff = DiffView::compute_unified_diff(&old_str, &new_str, "big_file.txt");
        assert!(!diff.is_empty());
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Deletion(d) if d.contains("line_0500")))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a.contains("line_0500_MODIFIED")))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a.contains("line_inserted_xyz")))
        );
    }

    #[test]
    fn test_windows_crlf_diff() {
        let old = "line1\r\nline2\r\nline3\r\n";
        let new = "line1\r\nline2_changed\r\nline3\r\n";
        let diff = DiffView::compute_unified_diff(old, new, "windows.txt");
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Addition(a) if a == "+line2_changed"))
        );
        assert!(
            diff.iter()
                .any(|l| matches!(l, DiffLine::Deletion(d) if d == "-line2"))
        );
    }
}
