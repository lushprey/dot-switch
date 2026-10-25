//! Interactive TUI: launches when `dot-switch` is run with no subcommand.
//! Fuzzy-searchable list of profiles + live diff preview.

use std::io::{self, Stdout};
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::config::Config;
use crate::switch;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    profiles: Vec<String>,
    filter: String,
    state: ListState,
    cfg: Config,
    diff: Option<String>,
    diff_err: Option<String>,
}

impl Tui {
    pub fn new(cfg: Config) -> Result<Self> {
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let profiles = switch::list_profiles(&PathBuf::from(&cfg.profiles_repo))?;
        let mut state = ListState::default();
        if !profiles.is_empty() {
            state.select(Some(0));
        }

        Ok(Tui {
            terminal,
            profiles,
            filter: String::new(),
            state,
            cfg,
            diff: None,
            diff_err: None,
        })
    }

    fn filtered(&self) -> Vec<String> {
        if self.filter.is_empty() {
            return self.profiles.clone();
        }
        let needle = self.filter.to_lowercase();
        self.profiles
            .iter()
            .filter(|p| p.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    pub fn run(mut self) -> Result<Option<String>> {
        let selected = self.run_loop()?;
        self.teardown()?;
        Ok(selected)
    }

    fn run_loop(&mut self) -> Result<Option<String>> {
        self.refresh_diff();
        loop {
            self.draw()?;

            if !event::poll(std::time::Duration::from_millis(250))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Enter => {
                    let filtered = self.filtered();
                    let idx = self.state.selected().unwrap_or(0);
                    if idx < filtered.len() {
                        return Ok(Some(filtered[idx].clone()));
                    }
                }
                KeyCode::Down => {
                    let n = self.filtered().len();
                    if n > 0 {
                        let i = self.state.selected().unwrap_or(0);
                        let next = if i + 1 >= n { 0 } else { i + 1 };
                        self.state.select(Some(next));
                        self.refresh_diff();
                    }
                }
                KeyCode::Up => {
                    let n = self.filtered().len();
                    if n > 0 {
                        let i = self.state.selected().unwrap_or(0);
                        let prev = if i == 0 { n - 1 } else { i - 1 };
                        self.state.select(Some(prev));
                        self.refresh_diff();
                    }
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.state.select(Some(0));
                    self.refresh_diff();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.state.select(Some(0));
                    self.refresh_diff();
                }
                _ => {}
            }
        }
    }

    fn refresh_diff(&mut self) {
        let filtered = self.filtered();
        let idx = self.state.selected().unwrap_or(0);
        if idx >= filtered.len() {
            self.diff = None;
            self.diff_err = None;
            return;
        }
        let name = &filtered[idx];
        match switch::diff_profile(&self.cfg, name, None) {
            Ok(report) => {
                let mut lines = String::new();
                lines.push_str(&report.summary_line());
                lines.push('\n');
                for f in &report.files {
                    let mark = match f.outcome {
                        switch::Outcome::Created | switch::Outcome::WouldCreate => "+",
                        switch::Outcome::Replaced | switch::Outcome::WouldReplace => "~",
                        switch::Outcome::AlreadyLinked => "=",
                    };
                    lines.push_str(&format!("  {} {} -> {}\n", mark, f.source, f.target));
                }
                self.diff = Some(lines);
                self.diff_err = None;
            }
            Err(e) => {
                self.diff = None;
                self.diff_err = Some(format!("{:#}", e));
            }
        }
    }

    fn draw(&mut self) -> Result<()> {
        let filtered = self.filtered();
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|p| {
                ListItem::new(Line::from(Span::styled(
                    p.clone(),
                    Style::default().fg(Color::White),
                )))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Profiles (filter: {:?}) ", self.filter)),
            )
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        let diff_text = self.diff.clone().unwrap_or_default();
        let err_text = self.diff_err.clone().unwrap_or_default();
        let diff_widget = Paragraph::new(format!("{}\n{}", diff_text, err_text))
            .block(Block::default().borders(Borders::ALL).title(" Diff Preview "))
            .wrap(Wrap { trim: false });

        let hint = Paragraph::new(" ↑/↓ navigate · type to filter · Enter switch · q quit ")
            .style(Style::default().fg(Color::Yellow));

        self.terminal.draw(|f| {
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(f.area());
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(main[0]);
            f.render_stateful_widget(list, cols[0], &mut self.state);
            f.render_widget(diff_widget, cols[1]);
            f.render_widget(hint, main[1]);
        })?;
        Ok(())
    }

    fn teardown(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

pub fn launch(cfg: Config) -> Result<Option<String>> {
    let tui = Tui::new(cfg)?;
    tui.run()
}
