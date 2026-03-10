use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;

use super::app::{App, InputField, Screen};
use crate::coin_selection::script_types::ScriptType;

/// Main render function — dispatches to the active screen.
pub fn render(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Dashboard => render_dashboard(f, app),
        Screen::NewSession => render_new_session(f, app),
        Screen::Sessions => render_sessions(f, app),
        Screen::Help => render_help(f, app),
    }
}

fn render_dashboard(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // tabs
            Constraint::Length(5),  // wallet summary
            Constraint::Min(8),    // UTXO table
            Constraint::Length(8), // log
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_wallet_summary(f, app, chunks[2]);
    render_utxo_table(f, app, chunks[3]);
    render_log(f, app, chunks[4]);
    render_footer(f, chunks[5]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let status_color = if app.connected {
        Color::Green
    } else {
        Color::Red
    };
    let conn_text = if app.connected {
        "Connected"
    } else {
        "Disconnected"
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " pj-receive ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("| "),
        Span::styled(conn_text, Style::default().fg(status_color)),
        Span::raw(" | "),
        Span::styled(
            format!("Network: {}", app.network),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" | "),
        Span::raw(format!("Block: {}", app.block_height)),
        Span::raw(" | "),
        Span::raw(format!("UTXOs: {}", app.utxos.len())),
        Span::raw(" | "),
        Span::styled(
            format!("Balance: {} sats", app.total_balance()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Taproot-Optimized Payjoin Receiver ")
            .title_alignment(Alignment::Center),
    );
    f.render_widget(header, area);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["[1] Dashboard", "[2] New Session", "[3] Sessions", "[?] Help"];
    let tab_index = match app.screen {
        Screen::Dashboard => 0,
        Screen::NewSession => 1,
        Screen::Sessions => 2,
        Screen::Help => 3,
    };

    let tabs = Tabs::new(titles)
        .select(tab_index)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("|")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    f.render_widget(tabs, area);
}

fn render_wallet_summary(f: &mut Frame, app: &App, area: Rect) {
    let p2tr_count = app
        .utxos
        .iter()
        .filter(|u| ScriptType::detect(&u.script_pubkey) == ScriptType::P2TR)
        .count();
    let p2wpkh_count = app
        .utxos
        .iter()
        .filter(|u| ScriptType::detect(&u.script_pubkey) == ScriptType::P2WPKH)
        .count();
    let other_count = app.utxos.len() - p2tr_count - p2wpkh_count;

    let feerate_text = match app.feerate {
        Some(rate) => format!("{:.1} sat/vB", rate),
        None => "N/A".to_string(),
    };

    let text = vec![
        Line::from(vec![
            Span::styled("  Taproot (P2TR): ", Style::default().fg(Color::Green)),
            Span::raw(format!("{}", p2tr_count)),
            Span::raw("    "),
            Span::styled("SegWit (P2WPKH): ", Style::default().fg(Color::Blue)),
            Span::raw(format!("{}", p2wpkh_count)),
            Span::raw("    "),
            Span::styled("Other: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", other_count)),
        ]),
        Line::from(vec![
            Span::styled("  Fee Rate: ", Style::default().fg(Color::Yellow)),
            Span::raw(feerate_text),
            Span::raw("    "),
            Span::styled("Active Sessions: ", Style::default().fg(Color::Magenta)),
            Span::raw(format!("{}", app.active_session_count())),
            Span::raw("    "),
            Span::styled("Wallet: ", Style::default().fg(Color::Cyan)),
            Span::raw(app.wallet_name.as_deref().unwrap_or("default")),
        ]),
    ];

    let summary = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Wallet Summary ")
            .title_alignment(Alignment::Left),
    );
    f.render_widget(summary, area);
}

fn render_utxo_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from(" # "),
        Cell::from("Type"),
        Cell::from("Amount (sats)"),
        Cell::from("Confs"),
        Cell::from("Address"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = app
        .utxos
        .iter()
        .enumerate()
        .map(|(i, utxo)| {
            let script_type = ScriptType::detect(&utxo.script_pubkey);
            let type_color = match script_type {
                ScriptType::P2TR => Color::Green,
                ScriptType::P2WPKH => Color::Blue,
                _ => Color::DarkGray,
            };

            let addr_display = if utxo.address.len() > 20 {
                format!("{}...{}", &utxo.address[..10], &utxo.address[utxo.address.len() - 8..])
            } else {
                utxo.address.clone()
            };

            Row::new(vec![
                Cell::from(format!(" {} ", i + 1)),
                Cell::from(Span::styled(
                    format!("{}", script_type),
                    Style::default().fg(type_color),
                )),
                Cell::from(Span::styled(
                    format_sats(utxo.amount.to_sat()),
                    Style::default().fg(Color::White),
                )),
                Cell::from(format!("{}", utxo.confirmations)),
                Cell::from(Span::styled(
                    addr_display,
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Wallet UTXOs ")
            .title_alignment(Alignment::Left),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

fn render_log(f: &mut Frame, app: &App, area: Rect) {
    let log_lines: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|entry| {
            let (color, prefix) = match entry.level.as_str() {
                "INFO" => (Color::Green, "INFO "),
                "WARN" => (Color::Yellow, "WARN "),
                "ERROR" => (Color::Red, "ERR  "),
                _ => (Color::DarkGray, "DBG  "),
            };
            Line::from(vec![
                Span::styled(
                    format!("  {} ", entry.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(prefix, Style::default().fg(color)),
                Span::raw(&entry.message),
            ])
        })
        .collect();

    let log = Paragraph::new(log_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Log ")
            .title_alignment(Alignment::Left),
    );
    f.render_widget(log, area);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Quit  "),
        Span::styled("1-3", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Navigate  "),
        Span::styled("?", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Help  "),
        Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Refresh"),
    ]));
    f.render_widget(footer, area);
}

// ─── New Session Screen ──────────────────────────────────────────────────

fn render_new_session(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // tabs
            Constraint::Length(14), // form
            Constraint::Min(4),    // preview / URI
            Constraint::Length(8), // log
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_session_form(f, app, chunks[2]);
    render_session_preview(f, app, chunks[3]);
    render_log(f, app, chunks[4]);
    render_footer_session(f, chunks[5]);
}

fn render_session_form(f: &mut Frame, app: &App, area: Rect) {
    let fields = [
        ("Amount (sats)", &app.input_amount, InputField::Amount),
        ("Strategy", &app.input_strategy, InputField::Strategy),
        ("Max Inputs", &app.input_max_inputs, InputField::MaxInputs),
        ("Expiry (min)", &app.input_expiry, InputField::Expiry),
        ("Label", &app.input_label, InputField::Label),
    ];

    let lines: Vec<Line> = fields
        .iter()
        .map(|(label, value, field)| {
            let is_active = app.active_field == *field;
            let cursor = if is_active && app.editing { "_" } else { "" };

            let label_style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let value_style = if is_active {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let indicator = if is_active { "> " } else { "  " };

            Line::from(vec![
                Span::styled(indicator, Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:<15}", label), label_style),
                Span::styled(format!("{}{}", value, cursor), value_style),
            ])
        })
        .collect();

    // Add strategy hint
    let mut all_lines = lines;
    all_lines.push(Line::from(""));
    all_lines.push(Line::from(vec![
        Span::styled(
            "  Strategies: ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("balanced", Style::default().fg(Color::Green)),
        Span::raw(" | "),
        Span::styled("privacy-max", Style::default().fg(Color::Magenta)),
        Span::raw(" | "),
        Span::styled("fee-min", Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled("consolidate", Style::default().fg(Color::Blue)),
    ]));

    let form = Paragraph::new(all_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" New Receive Session ")
            .title_alignment(Alignment::Left),
    );
    f.render_widget(form, area);
}

fn render_session_preview(f: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(ref uri) = app.session_uri {
        vec![
            Line::from(vec![
                Span::styled(
                    "  Status: ",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    &app.session_status,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  BIP 21 URI: ", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![Span::styled(
                format!("  {}", uri),
                Style::default().fg(Color::White),
            )]),
        ]
    } else if let Some(ref preview) = app.selection_preview {
        let lines: Vec<Line> = preview
            .iter()
            .map(|line| {
                Line::from(vec![Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Gray),
                )])
            })
            .collect();
        lines
    } else {
        vec![Line::from(vec![Span::styled(
            "  Press Enter to start session, Tab/Up/Down to navigate fields",
            Style::default().fg(Color::DarkGray),
        )])]
    };

    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Session ")
                .title_alignment(Alignment::Left),
        );
    f.render_widget(preview, area);
}

fn render_footer_session(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Tab/\u{2191}\u{2193}", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Navigate  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Start Session  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Back  "),
        Span::styled("q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Quit"),
    ]));
    f.render_widget(footer, area);
}

// ─── Sessions Screen ─────────────────────────────────────────────────────

fn render_sessions(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(3), // tabs
            Constraint::Min(8),   // sessions table
            Constraint::Length(8), // log
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);
    render_sessions_table(f, app, chunks[2]);
    render_log(f, app, chunks[3]);
    render_footer(f, chunks[4]);
}

fn render_sessions_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from(" # "),
        Cell::from("Amount (sats)"),
        Cell::from("Strategy"),
        Cell::from("Status"),
        Cell::from("Label"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let status_color = match session.status {
                crate::session::SessionStatus::Pending => Color::Yellow,
                crate::session::SessionStatus::ProposalSent => Color::Blue,
                crate::session::SessionStatus::Completed => Color::Green,
                crate::session::SessionStatus::Expired => Color::DarkGray,
                crate::session::SessionStatus::Failed(_) => Color::Red,
            };

            Row::new(vec![
                Cell::from(format!(" {} ", i + 1)),
                Cell::from(format_sats(session.amount_sats)),
                Cell::from(session.strategy.clone()),
                Cell::from(Span::styled(
                    session.status.to_string(),
                    Style::default().fg(status_color),
                )),
                Cell::from(
                    session
                        .label
                        .as_deref()
                        .unwrap_or("-")
                        .to_string(),
                ),
            ])
        })
        .collect();

    let empty_msg = if rows.is_empty() {
        " No sessions yet. Press [2] to create one. "
    } else {
        ""
    };

    if rows.is_empty() {
        let msg = Paragraph::new(Line::from(vec![Span::styled(
            empty_msg,
            Style::default().fg(Color::DarkGray),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Sessions ")
                .title_alignment(Alignment::Left),
        );
        f.render_widget(msg, area);
    } else {
        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(16),
                Constraint::Length(14),
                Constraint::Length(16),
                Constraint::Min(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Sessions ")
                .title_alignment(Alignment::Left),
        );
        f.render_widget(table, area);
    }
}

// ─── Help Screen ─────────────────────────────────────────────────────────

fn render_help(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(3), // tabs
            Constraint::Min(10),  // help content
            Constraint::Length(1), // footer
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  1          ", Style::default().fg(Color::Yellow)),
            Span::raw("Dashboard — view wallet UTXOs and status"),
        ]),
        Line::from(vec![
            Span::styled("  2          ", Style::default().fg(Color::Yellow)),
            Span::raw("New Session — create a Payjoin receive session"),
        ]),
        Line::from(vec![
            Span::styled("  3          ", Style::default().fg(Color::Yellow)),
            Span::raw("Sessions — view active and past sessions"),
        ]),
        Line::from(vec![
            Span::styled("  r          ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh wallet data from Bitcoin Core"),
        ]),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
            Span::raw("Show this help screen"),
        ]),
        Line::from(vec![
            Span::styled("  q / Ctrl+C ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit the application"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  New Session Form", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab / Up/Down  ", Style::default().fg(Color::Yellow)),
            Span::raw("Move between fields"),
        ]),
        Line::from(vec![
            Span::styled("  Enter          ", Style::default().fg(Color::Yellow)),
            Span::raw("Start the receive session"),
        ]),
        Line::from(vec![
            Span::styled("  Esc            ", Style::default().fg(Color::Yellow)),
            Span::raw("Go back to Dashboard"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Coin Selection Strategies", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  balanced     ", Style::default().fg(Color::Green)),
            Span::raw("Default. Balances privacy, fees, and consolidation."),
        ]),
        Line::from(vec![
            Span::styled("  privacy-max  ", Style::default().fg(Color::Magenta)),
            Span::raw("Maximize heuristic breakage. Ignores fee cost."),
        ]),
        Line::from(vec![
            Span::styled("  fee-min      ", Style::default().fg(Color::Yellow)),
            Span::raw("Minimize fee impact. Choose cheapest inputs."),
        ]),
        Line::from(vec![
            Span::styled("  consolidate  ", Style::default().fg(Color::Blue)),
            Span::raw("Clean up dust UTXOs during low-fee periods."),
        ]),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Help ")
            .title_alignment(Alignment::Center),
    );
    f.render_widget(help, chunks[2]);
    render_footer(f, chunks[3]);
}

// ─── Helpers ─────────────────────────────────────────────────────────────

/// Format satoshis with thousand separators.
fn format_sats(sats: u64) -> String {
    let s = sats.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
