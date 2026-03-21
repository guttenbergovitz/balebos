use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use chrono::Utc;
use crate::tui::app::{App, VisibleItem, InputMode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/Stats
            Constraint::Length(if app.input_mode == InputMode::Filtering || !app.filter.is_empty() { 3 } else { 0 }), // Filter Bar
            Constraint::Min(10),   // Main View
            Constraint::Length(3), // Footer/Help/Input
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    if chunks[1].height > 0 {
        draw_filter_bar(f, app, chunks[1]);
    }
    draw_main(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);
    
    if app.input_mode == InputMode::Editing {
        draw_input_modal(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let stats = &app.model.stats;
    let scanning = if *app.is_scanning.lock().unwrap() {
        Span::styled(" [SCANNING...] ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
    } else {
        Span::raw("")
    };

    let text = vec![
        Line::from(vec![
            Span::styled(" Balebos Dashboard ", Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Span::raw(" | "),
            Span::styled(format!(" Assets: {} ", stats.total), Style::default().fg(Color::White)),
            Span::styled(format!(" Alive: {} ", stats.alive), Style::default().fg(Color::Green)),
            Span::styled(format!(" HTTP Errors: {} ", stats.http_errors), Style::default().fg(Color::Red)),
            Span::styled(format!(" DNS Issues: {} ", stats.dns_issues), Style::default().fg(Color::Yellow)),
            scanning,
        ]),
    ];

    let header = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Overview "));
    f.render_widget(header, area);
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.input_mode == InputMode::Filtering {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let filter_text = if app.filter.is_empty() && app.input_mode == InputMode::Filtering {
        "Type to filter domains...".to_string()
    } else {
        app.filter.clone()
    };

    let p = Paragraph::new(format!(" / {}", filter_text))
        .style(style)
        .block(Block::default().borders(Borders::ALL).title(" Filter (Root domains) "));
    f.render_widget(p, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Table
            Constraint::Percentage(50), // Details
        ])
        .split(area);

    draw_table(f, app, main_chunks[0]);
    draw_details(f, app, main_chunks[1]);
}

fn draw_table(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_style = Style::default().add_modifier(Modifier::REVERSED).fg(Color::Yellow);
    let header_cells = ["Asset", "Status", "Tech"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows = app.visible_items.iter().map(|item| {
        let (asset, is_subdomain, has_children, is_expanded) = match item {
            VisibleItem::Root(name) => {
                let group = app.model.groups.iter().find(|g| g.root.name == *name).unwrap();
                (&group.root, false, !group.subdomains.is_empty(), app.expanded_roots.contains(name))
            }
            VisibleItem::Subdomain(root_name, sub_name) => {
                let group = app.model.groups.iter().find(|g| g.root.name == *root_name).unwrap();
                let asset = group.subdomains.iter().find(|s| s.name == *sub_name).unwrap();
                (asset, true, false, false)
            }
        };

        let prefix = if is_subdomain {
            "  └─ "
        } else if has_children {
            if is_expanded { "[-] " } else { "[+] " }
        } else {
            "    "
        };

        let name_span = if is_subdomain {
            Span::styled(asset.name.clone(), Style::default().fg(Color::White))
        } else {
            Span::styled(asset.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        };

        let status = match &asset.latest_http {
            Some(http) => match http.status {
                Some(200) => Cell::from("200 OK").style(Style::default().fg(Color::Green)),
                Some(code) => Cell::from(code.to_string()).style(Style::default().fg(Color::Yellow)),
                None => match &http.error_kind {
                    Some(kind) => Cell::from(format!("{:?}", kind)).style(Style::default().fg(Color::Red)),
                    None => Cell::from("-"),
                },
            },
            None => Cell::from("-"),
        };

        let tech = match &asset.latest_http {
            Some(http) => {
                if !http.tech.is_empty() {
                    let t = http.tech.join(", ");
                    if t.len() > 30 { format!("{}...", &t[..27]) } else { t }
                } else {
                    "-".to_string()
                }
            }
            None => "-".to_string(),
        };

        Row::new(vec![
            Cell::from(Line::from(vec![Span::raw(prefix), name_span])),
            status,
            Cell::from(tech),
        ])
    });

    let t = Table::new(rows, [
        Constraint::Percentage(50),
        Constraint::Percentage(15),
        Constraint::Percentage(35),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Inventory (Space to toggle) "))
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.table_state);
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Details ");
    
    if let Some(asset) = app.selected_asset() {
        let mut text = vec![
            Line::from(vec![
                Span::styled("Target: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&asset.name),
            ]),
            Line::from(""),
        ];

        // DNS Details - ENHANCED
        let dns_header = match &asset.latest_dns {
            Some(dns) => format!("DNS Records (Last check: {}):", dns.checked_at.format("%Y-%m-%d %H:%M:%S")),
            None => "DNS Records:".to_string(),
        };
        text.push(Line::from(Span::styled(dns_header, Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))));
        
        if let Some(dns) = &asset.latest_dns {
            if !dns.a.is_empty() { text.push(Line::from(format!("  A:     {}", dns.a.join(", ")))); }
            if !dns.aaaa.is_empty() { text.push(Line::from(format!("  AAAA:  {}", dns.aaaa.join(", ")))); }
            if !dns.cname.is_empty() { text.push(Line::from(format!("  CNAME: {}", dns.cname.join(", ")))); }
            if !dns.mx.is_empty() { text.push(Line::from(format!("  MX:    {}", dns.mx.join(", ")))); }
            if !dns.ns.is_empty() { text.push(Line::from(format!("  NS:    {}", dns.ns.join(", ")))); }
            if !dns.txt.is_empty() { 
                text.push(Line::from("  TXT:"));
                for t in &dns.txt { text.push(Line::from(format!("    {}", t))); }
            }
            if !dns.soa.is_empty() { text.push(Line::from(format!("  SOA:   {}", dns.soa.join(", ")))); }
            
            if dns.a.is_empty() && dns.aaaa.is_empty() && dns.cname.is_empty() && dns.mx.is_empty() {
                text.push(Line::from("  No common records found"));
            }
        } else {
            text.push(Line::from("  Not probed"));
        }
        text.push(Line::from(""));

        // HTTP Details
        let http_header = match &asset.latest_http {
            Some(http) => format!("HTTP Info (Last check: {}):", http.checked_at.format("%Y-%m-%d %H:%M:%S")),
            None => "HTTP Info:".to_string(),
        };
        text.push(Line::from(Span::styled(http_header, Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))));
        
        if let Some(http) = &asset.latest_http {
            if let Some(title) = &http.title { text.push(Line::from(format!("  Title:  {}", title))); }
            if let Some(server) = &http.webserver { text.push(Line::from(format!("  Server: {}", server))); }
            if !http.tech.is_empty() { text.push(Line::from(format!("  Tech:   {}", http.tech.join(", ")))); }
            if let Some(url) = &http.final_url { text.push(Line::from(format!("  Final:  {}", url))); }
            
            if !http.redirect_chain.is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled("  Redirect Chain:", Style::default().add_modifier(Modifier::ITALIC))));
                for (i, hop) in http.redirect_chain.iter().enumerate() {
                    text.push(Line::from(format!("    {}. {} ({})", i + 1, hop.url, hop.status)));
                }
            }

            // TLS Details
            if let Some(tls) = &http.tls {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled("  TLS Certificate:", Style::default().add_modifier(Modifier::ITALIC))));
                text.push(Line::from(format!("    Issuer:  {}", tls.issuer_dn)));
                
                let now = Utc::now();
                let days_left = tls.not_after.signed_duration_since(now).num_days();
                let expiry_style = if days_left < 0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if days_left < 14 {
                    Style::default().fg(Color::Red)
                } else if days_left < 30 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                };

                text.push(Line::from(vec![
                    Span::raw("    Expires: "),
                    Span::styled(
                        format!("{} ({} days left)", tls.not_after.format("%Y-%m-%d"), days_left),
                        expiry_style
                    ),
                ]));
            }
        } else {
            text.push(Line::from("  Not probed"));
        }

        let details = Paragraph::new(text).block(block).wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(details, area);
    } else {
        let details = Paragraph::new("Select an asset to see details").block(block);
        f.render_widget(details, area);
    }
}

fn draw_footer(f: &mut Frame, _app: &App, area: Rect) {
    let footer = Paragraph::new(" q: Quit | /: Filter | a: Add | s: Scan | Space: Toggle | k/↑: Up | j/↓: Down ")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn draw_input_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, f.area());
    let block = Block::default().title(" Add New Domain to Scan ").borders(Borders::ALL).style(Style::default().bg(Color::Black));
    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(block);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
    
    f.set_cursor_position(ratatui::layout::Position::new(
        area.x + app.input.len() as u16 + 1,
        area.y + 1,
    ));
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
