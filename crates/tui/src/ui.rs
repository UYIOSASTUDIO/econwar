//! TUI rendering with Ratatui.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
};

use crate::app::{App, InputMode, Panel};

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    draw_header(f, app, main_chunks[0]);
    draw_body(f, app, main_chunks[1]);
    draw_command_bar(f, app, main_chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let player = app.username.as_deref().unwrap_or("not logged in");
    let company = app.active_company.as_ref().map(|c| c.name.as_str()).unwrap_or("none");
    let header_text = format!(
        " ECONWAR v0.1  |  Player: {}  |  Company: {}  |  Type 'help' for commands",
        player, company
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .block(Block::default());
    f.render_widget(header, area);
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_log_panel(f, app, body_chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_chunks[1]);

    draw_market_panel(f, app, right_chunks[0]);
    draw_company_panel(f, app, right_chunks[1]);
}

fn draw_log_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Log;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Log ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.log_messages.is_empty() { return; }

    let visible_height = inner.height as usize;
    let total = app.log_messages.len();

    // Maximal möglicher Offset nach oben
    let max_scroll = total.saturating_sub(visible_height);

    // Wir kappen den Zähler des Users, falls er zu weit hochgescrollt hat
    let clamped_scroll = app.log_scroll.min(max_scroll);

    // Start-Index berechnen: Maximaler Index minus den Offset vom Boden
    let start = max_scroll.saturating_sub(clamped_scroll);
    let end = (start + visible_height).min(total);

    let items: Vec<ListItem> = app.log_messages[start..end]
        .iter()
        .map(|msg| {
            // ... (Hier bleibt dein bisheriger Style-Code exakt gleich) ...
            let style = if msg.contains("OK:") {
                Style::default().fg(Color::Green)
            } else if msg.contains("ERR:") {
                Style::default().fg(Color::Red)
            } else if msg.contains("===") || msg.contains("---") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(msg.as_str())).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn draw_market_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Markets;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Markets (live) ")
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.markets.is_empty() {
        let msg = Paragraph::new(" No data. Login to see markets.")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["Resource", "Price", "Supply", "Demand"])
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let rows: Vec<Row> = app.markets.iter().map(|m| {
        Row::new(vec![
            m.slug.clone(),
            truncate(&m.price),
            truncate(&m.supply),
            truncate(&m.demand),
        ]).style(Style::default().fg(Color::White))
    }).collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    f.render_widget(table, area);
}

fn draw_company_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Company;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let block = Block::default()
        .title(" Company ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    match &app.active_company {
        Some(c) => {
            let text = vec![
                Line::from(vec![
                    Span::styled(" Name:      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&c.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled(" Treasury:  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} credits", c.treasury), Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::styled(" Workers:   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} / {}", c.workers, c.capacity), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled(" Factories: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&c.factories, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled(" Tech Level:", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!(" {}", c.tech_level), Style::default().fg(Color::Magenta)),
                ]),
            ];
            let paragraph = Paragraph::new(text);
            f.render_widget(paragraph, inner);
        }
        None => {
            let msg = Paragraph::new(" No company selected.")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(msg, inner);
        }
    }
}

fn draw_command_bar(f: &mut Frame, app: &App, area: Rect) {
    let (prompt, style) = match app.input_mode {
        InputMode::Command => (" > ", Style::default().fg(Color::Cyan)),
        InputMode::Normal => (" NORMAL (press : or / to type) ", Style::default().fg(Color::DarkGray)),
    };
    let input_str: String = app.input.iter().collect();
    let display = format!("{prompt}{input_str}");
    let input_widget = Paragraph::new(display)
        .style(style)
        .block(Block::default().borders(Borders::ALL).border_style(
            match app.input_mode {
                InputMode::Command => Style::default().fg(Color::Cyan),
                InputMode::Normal => Style::default().fg(Color::DarkGray),
            },
        ));
    f.render_widget(input_widget, area);

    if app.input_mode == InputMode::Command {
        let cursor_x = area.x + prompt.len() as u16 + input_str.len() as u16 + 1;
        let cursor_y = area.y + 1;
        f.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

fn truncate(s: &str) -> String {
    if let Some(dot) = s.find('.') {
        let end = (dot + 3).min(s.len());
        s[..end].to_string()
    } else {
        s.to_string()
    }
}
