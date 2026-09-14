//! Top-level Lazy Herd TUI: split menu + sub-plugin host.

use crate::plugins;
use crate::registry::{NavAction, PluginCtx, PluginRegistry};
use crate::storage::Paths;
use crate::ui::{draw_help_footer, draw_status};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{stdout, Stdout};
use std::time::Duration;

enum Screen {
    Menu { selected: usize },
    Plugin { id: String },
}

pub fn run() -> Result<()> {
    let paths = Paths::resolve();
    let mut ctx = PluginCtx::new(paths);
    let mut registry = plugins::build_registry();

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let result = run_loop(&mut terminal, &mut registry, &mut ctx);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    registry: &mut PluginRegistry,
    ctx: &mut PluginCtx,
) -> Result<()> {
    let metas = registry.metas();
    let mut screen = Screen::Menu { selected: 0 };

    loop {
        terminal.draw(|frame| draw(frame, &screen, registry, ctx))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match &mut screen {
            Screen::Menu { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => {
                    if *selected == 0 {
                        *selected = metas.len().saturating_sub(1);
                    } else {
                        *selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if metas.is_empty() {
                        continue;
                    }
                    *selected = (*selected + 1) % metas.len();
                }
                KeyCode::Home => *selected = 0,
                KeyCode::End => *selected = metas.len().saturating_sub(1),
                KeyCode::Enter | KeyCode::Char('l') => {
                    if let Some(meta) = metas.get(*selected) {
                        let id = meta.id.to_string();
                        if let Some(plugin) = registry.get_mut(&id) {
                            plugin.on_enter(ctx);
                        }
                        screen = Screen::Plugin { id };
                    }
                }
                KeyCode::Char('?') => {
                    if let Some(docs) = metas.iter().find(|m| m.id == "docs") {
                        let id = docs.id.to_string();
                        if let Some(plugin) = registry.get_mut(&id) {
                            plugin.on_enter(ctx);
                        }
                        screen = Screen::Plugin { id };
                    }
                }
                _ => {}
            },
            Screen::Plugin { id } => {
                let action = if let Some(plugin) = registry.get_mut(id) {
                    plugin.handle(ctx, key)
                } else {
                    NavAction::Back
                };
                match action {
                    NavAction::None => {}
                    NavAction::Back => {
                        ctx.status.clear();
                        ctx.error = None;
                        screen = Screen::Menu {
                            selected: metas
                                .iter()
                                .position(|m| m.id == id)
                                .unwrap_or(0),
                        };
                    }
                    NavAction::Quit => return Ok(()),
                }
            }
        }
    }
}

fn draw(
    frame: &mut Frame,
    screen: &Screen,
    registry: &mut PluginRegistry,
    ctx: &PluginCtx,
) {
    let area = frame.area();
    match screen {
        Screen::Menu { selected } => draw_menu(frame, area, registry, *selected, ctx),
        Screen::Plugin { id } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(area);
            if let Some(plugin) = registry.get_mut(id) {
                plugin.draw(frame, chunks[0], ctx);
            } else {
                frame.render_widget(
                    Paragraph::new(format!("Unknown sub-plugin: {id}")),
                    chunks[0],
                );
            }
            draw_status(frame, chunks[1], &ctx.status, ctx.error.as_deref());
            draw_help_footer(frame, chunks[2], "Esc back to menu · q only quits from main menu");
        }
    }
}

fn draw_menu(
    frame: &mut Frame,
    area: Rect,
    registry: &PluginRegistry,
    selected: usize,
    ctx: &PluginCtx,
) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new("Lazy Herd")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("set up once · stay lazy"),
        );
    frame.render_widget(header, outer[0]);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(outer[1]);

    let metas = registry.metas();
    let mut lines = Vec::new();
    for (i, meta) in metas.iter().enumerate() {
        let marker = if i == selected { "▶ " } else { "  " };
        let style = if i == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}", meta.title),
            style,
        )));
    }
    let list = Paragraph::new(lines).block(
        Block::default()
            .title("Sub-plugins")
            .borders(Borders::ALL),
    );
    frame.render_widget(list, panes[0]);

    let desc = metas
        .get(selected)
        .map(|m| m.description)
        .unwrap_or("No sub-plugins registered.");
    let title = metas
        .get(selected)
        .map(|m| m.title)
        .unwrap_or("Lazy Herd");
    let detail = Paragraph::new(desc)
        .wrap(Wrap { trim: true })
        .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(detail, panes[1]);

    draw_status(frame, outer[2], &ctx.status, ctx.error.as_deref());
    draw_help_footer(
        frame,
        outer[3],
        "↑/↓ j/k scroll · Enter open · ? docs · Esc/q quit",
    );
}
