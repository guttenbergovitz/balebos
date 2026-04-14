//! Inventory tab rendering.
//!
//! Displays the asset list (domains) with DNS/HTTP status and details panel.

use chrono::Utc;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::risk::risk_color;
use crate::tui::app::App;
use crate::tui::model::AssetState;
use crate::tui::state::VisibleItem;

/// Draw the Inventory tab content (table + details).
pub fn draw_inventory(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Table
            Constraint::Percentage(50), // Details
        ])
        .split(area);

    draw_table(f, app, chunks[0]);
    draw_details(f, app, chunks[1]);
}

fn draw_table(f: &mut Frame, app: &mut App, area: Rect) {
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(Color::Yellow);
    let header_cells = ["Asset", "Risk", "Life", "Status", "Tech"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);

    let rows: Vec<_> = app
        .inventory
        .visible_items
        .iter()
        .filter_map(|item| {
            let (asset, is_subdomain, has_children, is_expanded) = match item {
                VisibleItem::Root(name) => {
                    let group = app.model.groups.iter().find(|g| g.root.name == *name)?;
                    (
                        &group.root,
                        false,
                        !group.subdomains.is_empty(),
                        app.inventory.expanded_roots.contains(name),
                    )
                }
                VisibleItem::Subdomain(root_name, sub_name) => {
                    let group = app
                        .model
                        .groups
                        .iter()
                        .find(|g| g.root.name == *root_name)?;
                    let asset = group.subdomains.iter().find(|s| s.name == *sub_name)?;
                    (asset, true, false, false)
                }
            };

            let prefix = if is_subdomain {
                "  └─ "
            } else if has_children {
                if is_expanded {
                    "[-] "
                } else {
                    "[+] "
                }
            } else {
                "    "
            };

            let risk_score = app.model.risk_scores.get(&asset.name).copied().unwrap_or(0);
            let risk_fg = risk_color(risk_score);

            let name_span = if is_subdomain {
                Span::styled(asset.name.clone(), Style::default().fg(risk_fg))
            } else {
                Span::styled(
                    asset.name.clone(),
                    Style::default()
                        .fg(risk_fg)
                        .add_modifier(Modifier::BOLD),
                )
            };

            let risk_text = if risk_score > 0 {
                risk_score.to_string()
            } else {
                "-".to_string()
            };

            let lifecycle = crate::inventory::get_current_lifecycle(&app.model.lifecycle_events, &asset.name);
            let (life_text, life_color) = match lifecycle {
                Some(state) => (state.icon().to_string(), state.color()),
                None => ("-".to_string(), Color::DarkGray),
            };

            let mut status_style = Style::default();
            let mut status_text = "-".to_string();

            if let Some(http) = &asset.latest_http {
                if let Some(code) = http.status {
                    if code == 200 {
                        if let Some(ext_target) = asset.external_redirect() {
                            status_text = format!("EXT -> {}", ext_target);
                            status_style = status_style.fg(Color::Yellow);
                        } else {
                            status_text = "200 OK".to_string();
                            status_style = status_style.fg(Color::Green);
                        }
                    } else {
                        status_text = code.to_string();
                        status_style = status_style.fg(Color::Yellow);
                    }
                } else if let Some(kind) = &http.error_kind {
                    status_text = format!("{:?}", kind);
                    status_style = status_style.fg(Color::Red);
                }
            }

            let tech = match &asset.latest_http {
                Some(http) => {
                    if !http.tech.is_empty() {
                        let t = http.tech.join(", ");
                        if t.len() > 30 {
                            format!("{}...", &t[..27])
                        } else {
                            t
                        }
                    } else {
                        "-".to_string()
                    }
                }
                None => "-".to_string(),
            };

            let is_marked = app.inventory.marked_assets.contains(&asset.name);
            let mark_prefix = if is_marked { "[x] " } else { "[ ] " };

            Some(Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(
                        mark_prefix,
                        if is_marked {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    name_span,
                ])),
                Cell::from(risk_text).style(Style::default().fg(risk_fg)),
                Cell::from(life_text).style(Style::default().fg(life_color)),
                Cell::from(status_text).style(status_style),
                Cell::from(tech),
            ]))
        })
        .collect();

    let mut title = if let Some(ref tag) = app.inventory.filter_tag {
        format!(" Inventory (Space to toggle) [Tag: {}] ", tag)
    } else {
        " Inventory (Space to toggle) ".to_string()
    };
    if app.inventory.sort_by_risk {
        title = format!("{}[Risk sort] ", title);
    }

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(8),
            Constraint::Percentage(6),
            Constraint::Percentage(21),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, &mut app.inventory.table_state);
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Details ");

    if let Some(asset) = app.selected_asset() {
        let text = build_details_text(asset, app);
        let details = Paragraph::new(text)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(details, area);
    } else {
        let details = Paragraph::new("Select an asset to see details").block(block);
        f.render_widget(details, area);
    }
}

pub fn build_details_text(asset: &AssetState, app: &App) -> Vec<Line<'static>> {
    let mut text = vec![
        Line::from(vec![
            Span::styled("Target: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(asset.name.clone()),
        ]),
        Line::from(""),
    ];

    // Risk Score
    let risk_score = app.model.risk_scores.get(&asset.name).copied().unwrap_or(0);
    if risk_score > 0 {
        let risk_label_text = crate::app::risk::risk_label(risk_score);
        let risk_fg = risk_color(risk_score);
        text.push(Line::from(vec![
            Span::styled("Risk: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} ({})", risk_score, risk_label_text),
                Style::default().fg(risk_fg).add_modifier(Modifier::BOLD),
            ),
        ]));
        text.push(Line::from(""));
    }

    // Lifecycle
    let lifecycle = crate::inventory::get_current_lifecycle(&app.model.lifecycle_events, &asset.name);
    if let Some(state) = lifecycle {
        text.push(Line::from(vec![
            Span::styled("Lifecycle: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{} {}", state.icon(), state),
                Style::default().fg(state.color()),
            ),
        ]));
        text.push(Line::from(""));
    }

    // Tags
    if let Some(tags) = app.model.asset_tags.get(&asset.name) {
        if !tags.is_empty() {
            text.push(Line::from(Span::styled(
                "Tags:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::UNDERLINED),
            )));
            for tag in tags {
                text.push(Line::from(format!("  • {}", tag)));
            }
            text.push(Line::from(""));
        }
    }

    // Redirect Status
    if let Some(ext_target) = asset.external_redirect() {
        text.push(Line::from(vec![
            Span::styled(
                "Redirect: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("EXTERNAL -> {}", ext_target),
                Style::default().fg(Color::Yellow),
            ),
        ]));
        text.push(Line::from(""));
    }

    // DNS Details
    let dns_header = match &asset.latest_dns {
        Some(dns) => format!(
            "DNS Records (Last check: {}):",
            dns.checked_at.format("%Y-%m-%d %H:%M:%S")
        ),
        None => "DNS Records:".to_string(),
    };
    text.push(Line::from(Span::styled(
        dns_header,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    )));

    if let Some(dns) = &asset.latest_dns {
        if !dns.a.is_empty() {
            text.push(Line::from(format!("  A:     {}", dns.a.join(", "))));
        }
        if !dns.aaaa.is_empty() {
            text.push(Line::from(format!("  AAAA:  {}", dns.aaaa.join(", "))));
        }
        if !dns.cname.is_empty() {
            text.push(Line::from(format!("  CNAME: {}", dns.cname.join(", "))));
        }
        if !dns.mx.is_empty() {
            text.push(Line::from(format!("  MX:    {}", dns.mx.join(", "))));
        }
        if !dns.ns.is_empty() {
            text.push(Line::from(format!("  NS:    {}", dns.ns.join(", "))));
        }
        if !dns.txt.is_empty() {
            text.push(Line::from("  TXT:"));
            for t in &dns.txt {
                text.push(Line::from(format!("    {}", t)));
            }
        }
        if !dns.soa.is_empty() {
            text.push(Line::from(format!("  SOA:   {}", dns.soa.join(", "))));
        }

        if dns.a.is_empty() && dns.aaaa.is_empty() && dns.cname.is_empty() && dns.mx.is_empty() {
            text.push(Line::from("  No common records found"));
        }
    } else {
        text.push(Line::from("  Not probed"));
    }
    text.push(Line::from(""));

    // HTTP Details
    let http_header = match &asset.latest_http {
        Some(http) => format!(
            "HTTP Info (Last check: {}):",
            http.checked_at.format("%Y-%m-%d %H:%M:%S")
        ),
        None => "HTTP Info:".to_string(),
    };
    text.push(Line::from(Span::styled(
        http_header,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    )));

    if let Some(http) = &asset.latest_http {
        if let Some(title) = &http.title {
            text.push(Line::from(format!("  Title:  {}", title)));
        }
        if let Some(server) = &http.webserver {
            text.push(Line::from(format!("  Server: {}", server)));
        }
        if !http.tech.is_empty() {
            text.push(Line::from(format!("  Tech:   {}", http.tech.join(", "))));
        }
        if let Some(url) = &http.final_url {
            text.push(Line::from(format!("  Final:  {}", url)));
        }

        if !http.redirect_chain.is_empty() {
            text.push(Line::from(""));
            text.push(Line::from(Span::styled(
                "  Redirect Chain:",
                Style::default().add_modifier(Modifier::ITALIC),
            )));
            for (i, hop) in http.redirect_chain.iter().enumerate() {
                text.push(Line::from(format!(
                    "    {}. {} ({})",
                    i + 1,
                    hop.url,
                    hop.status
                )));
            }
        }

        // TLS Details
        if let Some(tls) = &http.tls {
            text.push(Line::from(""));
            text.push(Line::from(Span::styled(
                "  TLS Certificate:",
                Style::default().add_modifier(Modifier::ITALIC),
            )));
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
                    format!(
                        "{} ({} days left)",
                        tls.not_after.format("%Y-%m-%d"),
                        days_left
                    ),
                    expiry_style,
                ),
            ]));
        }
    } else {
        text.push(Line::from("  Not probed"));
    }

    // Registration Info
    if let Some(reg) = &asset.registration {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "Registration Info (RDAP/Whois):",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )));
        if let Some(registrar) = &reg.registrar {
            text.push(Line::from(format!("  Registrar:  {}", registrar)));
        }
        if let Some(registered) = reg.registered_at {
            text.push(Line::from(format!(
                "  Registered: {}",
                registered.format("%Y-%m-%d")
            )));
        }
        if let Some(expires) = reg.expires_at {
            let now = Utc::now();
            let days_left = expires.signed_duration_since(now).num_days();
            let expiry_style = if days_left < 0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if days_left < 30 {
                Style::default().fg(Color::Red)
            } else if days_left < 90 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };
            text.push(Line::from(vec![
                Span::raw("  Expires:    "),
                Span::styled(
                    format!("{} ({} days left)", expires.format("%Y-%m-%d"), days_left),
                    expiry_style,
                ),
            ]));
        }
        if !reg.status.is_empty() {
            text.push(Line::from(format!(
                "  Status:     {}",
                reg.status.join(", ")
            )));
        }

        // Contacts
        if let Some(c) = &reg.registrant {
            text.push(Line::from(Span::styled(
                "  Registrant:",
                Style::default().add_modifier(Modifier::ITALIC),
            )));
            text.push(Line::from(format!(
                "    Name:     {}",
                c.name.as_deref().unwrap_or("-")
            )));
            text.push(Line::from(format!(
                "    Email:    {}",
                c.email.as_deref().unwrap_or("-")
            )));
        }
        if let Some(c) = &reg.admin {
            text.push(Line::from(Span::styled(
                "  Admin:",
                Style::default().add_modifier(Modifier::ITALIC),
            )));
            text.push(Line::from(format!(
                "    Name:     {}",
                c.name.as_deref().unwrap_or("-")
            )));
            text.push(Line::from(format!(
                "    Email:    {}",
                c.email.as_deref().unwrap_or("-")
            )));
        }

        text.push(Line::from(format!("  Source:     {}", reg.source)));
    }

    // Hosting Detection
    if let Some(hosting) = &asset.hosting {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "Hosting Detection:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )));

        if let Some(provider) = &hosting.provider_name {
            let category = hosting.provider_category.as_deref().unwrap_or("Unknown");
            text.push(Line::from(format!(
                "  Provider: {} ({})",
                provider, category
            )));
        } else {
            text.push(Line::from(Span::styled(
                "  Provider: UNKNOWN",
                Style::default().fg(Color::DarkGray),
            )));
        }

        if hosting.cdn_detected {
            let cdn = hosting
                .cdn_name
                .clone()
                .unwrap_or_else(|| "Unknown CDN".to_string());
            text.push(Line::from(vec![
                Span::raw("  CDN:      "),
                Span::styled(cdn, Style::default().fg(Color::Yellow)),
            ]));
        }

        let confidence_style = match hosting.confidence {
            crate::app::probe::Confidence::Confirmed => Style::default().fg(Color::Green),
            crate::app::probe::Confidence::High => Style::default().fg(Color::Green),
            crate::app::probe::Confidence::Medium => Style::default().fg(Color::Yellow),
            crate::app::probe::Confidence::Low => Style::default().fg(Color::Red),
        };
        text.push(Line::from(vec![
            Span::raw("  Confidence: "),
            Span::styled(format!("{}", hosting.confidence), confidence_style),
            Span::raw(format!(" (via {})", hosting.detection_method)),
        ]));

        // Show evidence if any
        if !hosting.evidence.is_empty() && hosting.evidence.len() <= 3 {
            text.push(Line::from(Span::styled(
                "  Evidence:",
                Style::default().add_modifier(Modifier::ITALIC),
            )));
            for ev in &hosting.evidence {
                text.push(Line::from(format!("    • {}", ev)));
            }
        }
    }

    // Organization Details
    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Organization:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    )));

    if let Some(project) = &asset.project {
        text.push(Line::from(format!("  Project: {}", project.project_name)));
        if !project.brand_name.is_empty() {
            text.push(Line::from(format!("  Brand:   {}", project.brand_name)));
        }
        if !project.company_name.is_empty() {
            text.push(Line::from(format!("  Company: {}", project.company_name)));
        }

        if !project.assigned_persons.is_empty() {
            text.push(Line::from("  People:"));
            for person in &project.assigned_persons {
                text.push(Line::from(format!(
                    "    - {} ({}) <{}>",
                    person.name, person.role, person.email
                )));
            }
        }

        if !project.assigned_companies.is_empty() {
            text.push(Line::from("  Companies:"));
            for company in &project.assigned_companies {
                text.push(Line::from(format!(
                    "    - {} ({})",
                    company.name, company.role
                )));
            }
        }
    } else {
        text.push(Line::from(Span::styled(
            "  Not assigned",
            Style::default().fg(Color::DarkGray),
        )));
    }

    text
}
