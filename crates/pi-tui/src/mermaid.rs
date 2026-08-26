use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;

pub struct MermaidRenderer;

impl MermaidRenderer {
    /// Renders Mermaid diagram code into styled terminal lines (mmdr-style)
    pub fn render(code: &str) -> Vec<Line<'static>> {
        let trimmed = code.trim();
        let first_line = trimmed.lines().next().unwrap_or("").trim().to_lowercase();

        if first_line.starts_with("sequencediagram") {
            Self::render_sequence_diagram(trimmed)
        } else if first_line.starts_with("graph") || first_line.starts_with("flowchart") {
            Self::render_flowchart(trimmed)
        } else {
            Self::render_generic_diagram(trimmed)
        }
    }

    fn render_flowchart(code: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut nodes: HashMap<String, String> = HashMap::new();
        let mut edges: Vec<(String, String, Option<String>)> = Vec::new();
        let mut node_order: Vec<String> = Vec::new();

        for raw_line in code.lines().skip(1) {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            // Check for edge patterns: "-->", "---", "-.->", "==>"
            let arrow_patterns = ["-->", "---", "-.->", "==>"];
            let mut matched_arrow = None;

            for pat in arrow_patterns {
                if line.contains(pat) {
                    matched_arrow = Some(pat);
                    break;
                }
            }

            if let Some(arrow) = matched_arrow {
                let parts: Vec<&str> = line.split(arrow).collect();
                if parts.len() >= 2 {
                    let left_part = parts[0].trim();
                    let right_raw = parts[1].trim();

                    let (edge_label, right_part) =
                        if let Some(stripped) = right_raw.strip_prefix('|') {
                            if let Some(second_pipe) = stripped.find('|') {
                                let safe_pipe = stripped.floor_char_boundary(second_pipe);
                                let lbl = stripped[..safe_pipe].trim().to_string();
                                let safe_rest = stripped
                                    .floor_char_boundary((safe_pipe + 1).min(stripped.len()));
                                let rest = stripped[safe_rest..].trim();
                                (Some(lbl), rest)
                            } else {
                                (None, right_raw)
                            }
                        } else if parts.len() > 2 {
                            (
                                Some(parts[1].trim().trim_matches('|').to_string()),
                                parts[2].trim(),
                            )
                        } else {
                            (None, right_raw)
                        };

                    let (left_id, left_label) = Self::parse_node(left_part);
                    let (right_id, right_label) = Self::parse_node(right_part);

                    if let Some(existing) = nodes.get_mut(&left_id) {
                        if *existing == left_id && left_label != left_id {
                            *existing = left_label;
                        }
                    } else {
                        node_order.push(left_id.clone());
                        nodes.insert(left_id.clone(), left_label);
                    }

                    if let Some(existing) = nodes.get_mut(&right_id) {
                        if *existing == right_id && right_label != right_id {
                            *existing = right_label;
                        }
                    } else {
                        node_order.push(right_id.clone());
                        nodes.insert(right_id.clone(), right_label);
                    }

                    edges.push((left_id, right_id, edge_label));
                    continue;
                }
            }

            // Single node definition
            let (id, label) = Self::parse_node(line);
            if !id.is_empty() {
                if let Some(existing) = nodes.get_mut(&id) {
                    if *existing == id && label != id {
                        *existing = label;
                    }
                } else {
                    node_order.push(id.clone());
                    nodes.insert(id, label);
                }
            }
        }

        if node_order.is_empty() {
            return vec![Line::from(Span::styled(
                "  [Empty Diagram]",
                Style::default().fg(Color::DarkGray),
            ))];
        }

        lines.push(Line::from(vec![
            Span::styled(
                "┌─── [Mermaid Flowchart] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "──────────────────────────────────┐",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        // Render sequential nodes with boxes and connecting arrows
        for (i, node_id) in node_order.iter().enumerate() {
            let label = nodes
                .get(node_id)
                .cloned()
                .unwrap_or_else(|| node_id.clone());
            let label_len = label.chars().count();
            let box_width = label_len.max(12) + 4;

            let top_border = format!("  ┌{}┐", "─".repeat(box_width));
            let padding_left = (box_width - label_len) / 2;
            let padding_right = box_width - label_len - padding_left;
            let bot_border = format!("  └{}┘", "─".repeat(box_width));

            lines.push(Line::from(Span::styled(
                top_border,
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::from(vec![
                Span::styled("  │", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!(
                        "{}{}{}",
                        " ".repeat(padding_left),
                        label,
                        " ".repeat(padding_right)
                    ),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("│", Style::default().fg(Color::Cyan)),
            ]));
            lines.push(Line::from(Span::styled(
                bot_border,
                Style::default().fg(Color::Cyan),
            )));

            // Draw outgoing arrow if there's a subsequent connection
            if i + 1 < node_order.len() {
                let next_id = &node_order[i + 1];
                let edge_label_opt = edges
                    .iter()
                    .find(|(src, dst, _)| src == node_id && dst == next_id)
                    .and_then(|(_, _, lbl)| lbl.clone());

                let center_pad = box_width / 2 + 2;
                lines.push(Line::from(Span::styled(
                    format!("{}│", " ".repeat(center_pad)),
                    Style::default().fg(Color::Yellow),
                )));

                if let Some(lbl) = edge_label_opt {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}│ (", " ".repeat(center_pad)),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(lbl, Style::default().fg(Color::Green)),
                        Span::styled(")", Style::default().fg(Color::Yellow)),
                    ]));
                }

                lines.push(Line::from(Span::styled(
                    format!("{}▼", " ".repeat(center_pad)),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "└─────────────────────────────────────────────────────┘",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    fn render_sequence_diagram(code: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut participants = Vec::new();
        let mut messages = Vec::new();

        for raw_line in code.lines().skip(1) {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            if let Some(rest) = line.strip_prefix("participant ") {
                let name = rest.trim().trim_matches('"');
                if !participants.contains(&name.to_string()) {
                    participants.push(name.to_string());
                }
            } else if line.contains("->>") || line.contains("-->>") || line.contains("->") {
                let is_reply = line.contains("-->>");
                let arrow = if is_reply {
                    "-->>"
                } else if line.contains("->>") {
                    "->>"
                } else {
                    "->"
                };
                let parts: Vec<&str> = line.split(arrow).collect();
                if parts.len() == 2 {
                    let from = parts[0].trim();
                    let (to, msg) = if let Some((t, m)) = parts[1].split_once(':') {
                        (t.trim(), m.trim())
                    } else {
                        (parts[1].trim(), "")
                    };

                    if !participants.contains(&from.to_string()) {
                        participants.push(from.to_string());
                    }
                    if !participants.contains(&to.to_string()) {
                        participants.push(to.to_string());
                    }

                    messages.push((from.to_string(), to.to_string(), msg.to_string(), is_reply));
                }
            }
        }

        if participants.len() < 2 {
            participants = vec!["Actor A".to_string(), "Actor B".to_string()];
        }

        lines.push(Line::from(vec![
            Span::styled(
                "┌─── [Mermaid Sequence Diagram] ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "────────────────────────────┐",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        // Header boxes
        let mut header_spans = Vec::new();
        header_spans.push(Span::raw("  "));
        for p in &participants {
            header_spans.push(Span::styled(
                format!(" ┌─── {} ───┐ ", p),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            header_spans.push(Span::raw("     "));
        }
        lines.push(Line::from(header_spans));

        // Lifelines and messages
        for (_from, _to, msg, is_reply) in messages {
            lines.push(Line::from(Span::styled(
                "       │                     │",
                Style::default().fg(Color::DarkGray),
            )));

            let arrow_line = if is_reply {
                format!(
                    "  ◀─────── {} ─────────",
                    if msg.is_empty() { "Response" } else { &msg }
                )
            } else {
                format!(
                    "  ──────── {} ─────────▶",
                    if msg.is_empty() { "Request" } else { &msg }
                )
            };

            lines.push(Line::from(vec![
                Span::styled("       │", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    arrow_line,
                    Style::default()
                        .fg(if is_reply {
                            Color::Green
                        } else {
                            Color::Yellow
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("│", Style::default().fg(Color::DarkGray)),
            ]));

            lines.push(Line::from(Span::styled(
                "       │                     │",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "└─────────────────────────────────────────────────────┘",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    fn render_generic_diagram(code: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "┌─── [Mermaid Diagram] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "──────────────────────────────────────┐",
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        for line in code.lines() {
            lines.push(Line::from(vec![
                Span::styled("  │ ", Style::default().fg(Color::DarkGray)),
                Span::styled(line.to_string(), Style::default().fg(Color::Cyan)),
            ]));
        }

        lines.push(Line::from(Span::styled(
            "└─────────────────────────────────────────────────────┘",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    fn parse_node(token: &str) -> (String, String) {
        let trimmed = token.trim();
        if let Some(pos) = trimmed.find('[') {
            let safe_pos = trimmed.floor_char_boundary(pos);
            let id = trimmed[..safe_pos].trim().to_string();
            let safe_rest = trimmed.floor_char_boundary((safe_pos + 1).min(trimmed.len()));
            let label = trimmed[safe_rest..]
                .trim_end_matches(']')
                .trim_matches('"')
                .to_string();
            (id, label)
        } else if let Some(pos) = trimmed.find('(') {
            let safe_pos = trimmed.floor_char_boundary(pos);
            let id = trimmed[..safe_pos].trim().to_string();
            let safe_rest = trimmed.floor_char_boundary((safe_pos + 1).min(trimmed.len()));
            let label = trimmed[safe_rest..]
                .trim_end_matches(')')
                .trim_matches('"')
                .to_string();
            (id, label)
        } else if let Some(pos) = trimmed.find('{') {
            let safe_pos = trimmed.floor_char_boundary(pos);
            let id = trimmed[..safe_pos].trim().to_string();
            let safe_rest = trimmed.floor_char_boundary((safe_pos + 1).min(trimmed.len()));
            let label = trimmed[safe_rest..]
                .trim_end_matches('}')
                .trim_matches('"')
                .to_string();
            (id, label)
        } else {
            (trimmed.to_string(), trimmed.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mermaid_flowchart() {
        let code = r#"
graph TD
    A[Client Request] --> B(API Gateway)
    B --> C{Authentication}
    C -->|Valid| D[(Database)]
"#;
        let rendered = MermaidRenderer::render(code);
        assert!(!rendered.is_empty());
        let full_text: String = rendered
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full_text.contains("Client Request"));
        assert!(full_text.contains("API Gateway"));
        assert!(full_text.contains("Database"));
    }

    #[test]
    fn test_render_mermaid_sequence() {
        let code = r#"
sequenceDiagram
    participant User
    participant Agent
    User->>Agent: Send task prompt
    Agent-->>User: Streaming token response
"#;
        let rendered = MermaidRenderer::render(code);
        assert!(!rendered.is_empty());
        let full_text: String = rendered
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full_text.contains("User"));
        assert!(full_text.contains("Agent"));
        assert!(full_text.contains("Send task prompt"));
    }

    #[test]
    fn test_render_mermaid_unicode_and_emojis() {
        let code = r#"
flowchart LR
    A[🚀 Client (ユーザー)] --> B{⚡ Proxy 🦀}
    B -->|OK ✨| C[🎯 Backend]
"#;
        let rendered = MermaidRenderer::render(code);
        assert!(!rendered.is_empty());
    }

    #[test]
    fn test_render_generic_diagram() {
        let code = r#"
pie title Pets
    "Dogs" : 386
    "Cats" : 85
"#;
        let rendered = MermaidRenderer::render(code);
        assert!(!rendered.is_empty());
        let full_text: String = rendered
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(full_text.contains("Pets"));
    }
}
