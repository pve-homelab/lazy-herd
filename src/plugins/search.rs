//! Lazy Search — built-in terminal browser (no external Herdr plugin).

use crate::registry::{NavAction, PluginCtx, SubPlugin};
use crate::storage::{read_toml, write_toml};
use crate::ui::draw_select_list;
use crate::ui::{FormField, FormResult, FormState};
use crate::ui::ScrollList;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchConfig {
    #[serde(default)]
    default_url: String,
    #[serde(default)]
    home_url: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_url: "https://herdr.dev/docs/".into(),
            home_url: "https://herdr.dev/docs/".into(),
        }
    }
}

#[derive(Debug, Clone)]
struct PageLink {
    text: String,
    href: String,
}

struct BrowserState {
    url: String,
    title: String,
    lines: Vec<String>,
    links: Vec<PageLink>,
    scroll: u16,
    link_sel: usize,
    history: Vec<String>,
    status: String,
}

enum Mode {
    Menu,
    Browse(BrowserState),
    FormUrl,
    FormConfig,
}

pub struct SearchPlugin {
    list: ScrollList,
    config: SearchConfig,
    mode: Mode,
    form: Option<FormState>,
    help: String,
}

impl SearchPlugin {
    pub fn new() -> Self {
        Self {
            list: ScrollList::new(vec![
                "Browse default URL (in Lazy Herd)".into(),
                "Browse custom URL… (in Lazy Herd)".into(),
                "Open default URL (system browser)".into(),
                "Edit search config".into(),
            ]),
            config: SearchConfig::default(),
            mode: Mode::Menu,
            form: None,
            help: String::new(),
        }
    }

    fn path(ctx: &PluginCtx) -> PathBuf {
        ctx.paths.join_config("search.toml")
    }

    fn reload(&mut self, ctx: &mut PluginCtx) {
        match read_toml::<SearchConfig>(&Self::path(ctx)) {
            Ok(Some(c)) => self.config = c,
            Ok(None) => self.config = SearchConfig::default(),
            Err(e) => ctx.set_error(format!("search config: {e}")),
        }
        if self.config.home_url.is_empty() {
            self.config.home_url = self.config.default_url.clone();
        }
        self.help = "Built-in text browser (part of Lazy Herd).\nNo zenbu-labs / terminal-browser plugin required.\nj/k scroll · Tab links · Enter follow · b back · o system browser · Esc menu".into();
    }

    fn save(&mut self, ctx: &mut PluginCtx) {
        if let Err(e) = write_toml(&Self::path(ctx), &self.config) {
            ctx.set_error(format!("save search config: {e}"));
        } else {
            ctx.set_status("search config saved");
        }
    }

    fn open_system_browser(url: &str) -> Result<(), String> {
        let url = url.trim();
        if url.is_empty() {
            return Err("URL is empty".into());
        }
        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Command::new("xdg-open")
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }

    fn fetch_page(url: &str) -> Result<BrowserState, String> {
        let parsed = Url::parse(url).map_err(|e| format!("bad URL: {e}"))?;
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout_read(std::time::Duration::from_secs(20))
            .user_agent("LazyHerd/0.1 (+https://github.com/pve-homelab/lazy-herd)")
            .build();
        let resp = agent
            .get(parsed.as_str())
            .call()
            .map_err(|e| format!("fetch failed: {e}"))?;
        let final_url = resp.get_url().to_string();
        let content_type = resp.content_type().to_string();
        let body = resp
            .into_string()
            .map_err(|e| format!("read body: {e}"))?;

        let (text, links) = if content_type.contains("html") || body.contains("<html") {
            render_html(&final_url, &body)
        } else {
            (
                body.lines().map(str::to_string).collect::<Vec<_>>(),
                Vec::new(),
            )
        };

        let title = text
            .first()
            .cloned()
            .unwrap_or_else(|| final_url.clone());
        let link_count = links.len();

        Ok(BrowserState {
            url: final_url,
            title,
            lines: text,
            links,
            scroll: 0,
            link_sel: 0,
            history: Vec::new(),
            status: format!("{link_count} links"),
        })
    }

    fn navigate(&mut self, ctx: &mut PluginCtx, url: &str, push_history: bool) {
        ctx.set_status(format!("loading {url}…"));
        match Self::fetch_page(url) {
            Ok(mut page) => {
                if push_history {
                    if let Mode::Browse(cur) = &self.mode {
                        page.history = cur.history.clone();
                        page.history.push(cur.url.clone());
                    }
                } else if let Mode::Browse(cur) = &self.mode {
                    page.history = cur.history.clone();
                }
                ctx.set_status(format!("loaded {}", page.url));
                self.mode = Mode::Browse(page);
            }
            Err(e) => {
                ctx.set_error(e);
                if !matches!(self.mode, Mode::Browse(_)) {
                    self.mode = Mode::Menu;
                }
            }
        }
    }
}

fn render_html(base_url: &str, html: &str) -> (Vec<String>, Vec<PageLink>) {
    let text = html2text::from_read(html.as_bytes(), 100);
    let lines: Vec<String> = text.lines().map(str::to_string).collect();

    let mut links = Vec::new();
    let base = Url::parse(base_url).ok();
    // Rough href scrape for followable links.
    let lower = html;
    let mut rest = lower;
    while let Some(idx) = rest.find("href=") {
        rest = &rest[idx + 5..];
        let quote = rest.chars().next();
        let href = match quote {
            Some('"') | Some('\'') => {
                let q = quote.unwrap();
                rest = &rest[1..];
                match rest.find(q) {
                    Some(end) => {
                        let h = &rest[..end];
                        rest = &rest[end + 1..];
                        h
                    }
                    None => continue,
                }
            }
            _ => continue,
        };
        if href.starts_with('#') || href.starts_with("javascript:") || href.starts_with("mailto:")
        {
            continue;
        }
        let abs = if let Some(base) = &base {
            base.join(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string())
        } else {
            href.to_string()
        };
        if !links.iter().any(|l: &PageLink| l.href == abs) {
            let text = abs
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(abs.as_str())
                .chars()
                .take(60)
                .collect::<String>();
            links.push(PageLink { text, href: abs });
        }
        if links.len() >= 80 {
            break;
        }
    }
    (lines, links)
}

impl SubPlugin for SearchPlugin {
    fn id(&self) -> &'static str {
        "search"
    }
    fn title(&self) -> &'static str {
        "Lazy Search"
    }
    fn description(&self) -> &'static str {
        "Built-in terminal web browser inside Lazy Herd (fetch + text render). Optional system-browser open. Does not use zenbu-labs.terminal-browser."
    }

    fn on_enter(&mut self, ctx: &mut PluginCtx) {
        self.reload(ctx);
        self.mode = Mode::Menu;
        ctx.set_status("Enter browse · Esc back");
    }

    fn handle(&mut self, ctx: &mut PluginCtx, key: KeyEvent) -> NavAction {
        // Forms
        if self.form.is_some() {
            let Some(form) = self.form.as_mut() else {
                return NavAction::None;
            };
            match form.handle(key) {
                FormResult::Cancel => {
                    self.form = None;
                    self.mode = Mode::Menu;
                }
                FormResult::Submit => {
                    let vals = form.values();
                    match self.mode {
                        Mode::FormUrl => {
                            let url = vals.first().cloned().unwrap_or_default();
                            self.form = None;
                            self.navigate(ctx, &url, false);
                        }
                        Mode::FormConfig => {
                            self.config.default_url = vals.first().cloned().unwrap_or_default();
                            self.config.home_url = vals
                                .get(1)
                                .cloned()
                                .unwrap_or_else(|| self.config.default_url.clone());
                            self.save(ctx);
                            self.form = None;
                            self.mode = Mode::Menu;
                        }
                        _ => {
                            self.form = None;
                            self.mode = Mode::Menu;
                        }
                    }
                }
                FormResult::Continue => {}
            }
            return NavAction::None;
        }

        match &mut self.mode {
            Mode::Menu => {
                if self.list.handle_nav(key) {
                    return NavAction::None;
                }
                match key.code {
                    KeyCode::Esc => NavAction::Back,
                    KeyCode::Enter => {
                        match self.list.selected() {
                            Some(0) => {
                                let url = self.config.default_url.clone();
                                self.navigate(ctx, &url, false);
                            }
                            Some(1) => {
                                self.form = Some(FormState::new(
                                    "Browse URL",
                                    vec![FormField::new("url").with_value(&self.config.default_url)],
                                ));
                                self.mode = Mode::FormUrl;
                            }
                            Some(2) => match Self::open_system_browser(&self.config.default_url) {
                                Ok(()) => ctx.set_status("opened system browser"),
                                Err(e) => ctx.set_error(e),
                            },
                            Some(3) => {
                                self.form = Some(FormState::new(
                                    "Search config",
                                    vec![
                                        FormField::new("default_url")
                                            .with_value(&self.config.default_url),
                                        FormField::new("home_url")
                                            .with_value(&self.config.home_url),
                                    ],
                                ));
                                self.mode = Mode::FormConfig;
                            }
                            _ => {}
                        }
                        NavAction::None
                    }
                    _ => NavAction::None,
                }
            }
            Mode::Browse(page) => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Menu;
                    ctx.set_status("back to Search menu");
                    NavAction::None
                }
                KeyCode::Char('q') => {
                    self.mode = Mode::Menu;
                    NavAction::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    page.scroll = page.scroll.saturating_sub(1);
                    NavAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    page.scroll = page.scroll.saturating_add(1);
                    NavAction::None
                }
                KeyCode::PageUp => {
                    page.scroll = page.scroll.saturating_sub(10);
                    NavAction::None
                }
                KeyCode::PageDown => {
                    page.scroll = page.scroll.saturating_add(10);
                    NavAction::None
                }
                KeyCode::Tab | KeyCode::Char(']') => {
                    if !page.links.is_empty() {
                        page.link_sel = (page.link_sel + 1) % page.links.len();
                        page.status = format!(
                            "link {}/{}: {}",
                            page.link_sel + 1,
                            page.links.len(),
                            page.links[page.link_sel].href
                        );
                    }
                    NavAction::None
                }
                KeyCode::BackTab | KeyCode::Char('[') => {
                    if !page.links.is_empty() {
                        page.link_sel = if page.link_sel == 0 {
                            page.links.len() - 1
                        } else {
                            page.link_sel - 1
                        };
                        page.status = format!(
                            "link {}/{}: {}",
                            page.link_sel + 1,
                            page.links.len(),
                            page.links[page.link_sel].href
                        );
                    }
                    NavAction::None
                }
                KeyCode::Enter => {
                    if let Some(link) = page.links.get(page.link_sel).cloned() {
                        let url = link.href;
                        self.navigate(ctx, &url, true);
                    }
                    NavAction::None
                }
                KeyCode::Char('b') => {
                    if let Some(prev) = page.history.pop() {
                        // Don't push current when going back.
                        let hist = page.history.clone();
                        match Self::fetch_page(&prev) {
                            Ok(mut p) => {
                                p.history = hist;
                                ctx.set_status(format!("back → {}", p.url));
                                self.mode = Mode::Browse(p);
                            }
                            Err(e) => ctx.set_error(e),
                        }
                    } else {
                        ctx.set_status("no history");
                    }
                    NavAction::None
                }
                KeyCode::Char('o') => {
                    let url = page.url.clone();
                    match Self::open_system_browser(&url) {
                        Ok(()) => ctx.set_status("opened system browser"),
                        Err(e) => ctx.set_error(e),
                    }
                    NavAction::None
                }
                KeyCode::Char('r') => {
                    let url = page.url.clone();
                    self.navigate(ctx, &url, false);
                    NavAction::None
                }
                KeyCode::Char('h') => {
                    let home = self.config.home_url.clone();
                    self.navigate(ctx, &home, true);
                    NavAction::None
                }
                _ => NavAction::None,
            },
            Mode::FormUrl | Mode::FormConfig => NavAction::None,
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &PluginCtx) {
        match &self.mode {
            Mode::Menu | Mode::FormUrl | Mode::FormConfig => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(area);
                draw_select_list(
                    frame,
                    chunks[0],
                    "Lazy Search (built-in)",
                    &self.list.items,
                    self.list.selected(),
                );
                frame.render_widget(
                    Paragraph::new(self.help.as_str())
                        .wrap(Wrap { trim: false })
                        .block(Block::default().title("About").borders(Borders::ALL)),
                    chunks[1],
                );
                if let Some(form) = &self.form {
                    form.draw(frame, area);
                }
            }
            Mode::Browse(page) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Length(4),
                        Constraint::Length(1),
                    ])
                    .split(area);
                frame.render_widget(
                    Paragraph::new(format!("{}\n{}", page.title, page.url))
                        .block(Block::default().title("Lazy Search").borders(Borders::ALL)),
                    chunks[0],
                );
                let body = page.lines.join("\n");
                frame.render_widget(
                    Paragraph::new(body)
                        .wrap(Wrap { trim: false })
                        .scroll((page.scroll, 0))
                        .block(Block::default().title("Page").borders(Borders::ALL)),
                    chunks[1],
                );
                let link_text = if page.links.is_empty() {
                    "(no links found)".into()
                } else {
                    page.links
                        .iter()
                        .enumerate()
                        .map(|(i, l)| {
                            let mark = if i == page.link_sel { ">" } else { " " };
                            format!("{mark}{i:>2} {}", l.text)
                        })
                        .take(6)
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                frame.render_widget(
                    Paragraph::new(link_text).block(
                        Block::default()
                            .title("Links · Tab/[ ] · Enter follow")
                            .borders(Borders::ALL),
                    ),
                    chunks[2],
                );
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} · j/k scroll · b back · r reload · h home · o OS browser · Esc menu",
                        page.status
                    ))
                    .style(Style::default().fg(Color::DarkGray)),
                    chunks[3],
                );
            }
        }
    }
}
