use crate::{
    config::{Config, Paths},
    git::{self, Progress, Upload},
    ingest::{self, QueuedFile},
};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
};
use ratatui::{
    DefaultTerminal,
    prelude::*,
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
        canvas::{Canvas, Circle, Line as RadarLine, Points},
    },
};
use std::{
    fs, io,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const BG: Color = Color::Rgb(9, 17, 27);
const GREEN: Color = Color::Rgb(103, 232, 173);
const CYAN: Color = Color::Rgb(96, 207, 233);
const MUTED: Color = Color::Rgb(138, 159, 177);

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Settings,
    Doctor,
    History,
    Help,
    Review,
}

struct App {
    paths: Paths,
    config: Config,
    screen: Screen,
    focus: usize,
    fields: [String; 4],
    files: Vec<QueuedFile>,
    selected: ListState,
    tags: String,
    notes: String,
    input: String,
    status: String,
    diagnostics: Vec<String>,
    busy: bool,
    receiver: Option<Receiver<Progress>>,
    drop_area: Rect,
    fields_area: Vec<Rect>,
    scroll: u16,
    error: bool,
}

impl App {
    fn new(paths: Paths, mut config: Config) -> Self {
        if config.author.is_empty() {
            config.author = git::global_identity("user.name");
        }
        if config.email.is_empty() {
            config.email = git::global_identity("user.email");
        }
        let diagnostics = git::doctor(&config);
        let needs_setup = diagnostics.iter().any(|s| s.starts_with("WARN"));
        let fields = [
            config.remote.clone(),
            config.project.clone(),
            config.author.clone(),
            config.email.clone(),
        ];
        let pending = paths
            .repository(&config.remote)
            .with_extension("pending.json")
            .exists();
        Self {
            paths,
            config,
            screen: if needs_setup {
                Screen::Doctor
            } else {
                Screen::Home
            },
            focus: 0,
            fields,
            files: Vec::new(),
            selected: ListState::default(),
            tags: String::new(),
            notes: String::new(),
            input: String::new(),
            status: if pending {
                "An earlier upload is pending. Press F6 to finish it before importing another batch."
            } else {
                "Ready. F4 browses files · F3 configures your repository · F1 opens help."
            }.into(),
            diagnostics,
            busy: false,
            receiver: None,
            drop_area: Rect::default(),
            fields_area: Vec::new(),
            scroll: 0,
            error: pending,
        }
    }

    fn message(&mut self, text: impl Into<String>, error: bool) {
        self.status = text.into();
        self.error = error;
        self.scroll = 0;
    }

    fn add(&mut self, paths: Vec<PathBuf>) {
        let mut added = 0;
        let mut errors = Vec::new();
        for path in paths {
            match QueuedFile::inspect(path) {
                Ok(file) if !self.files.iter().any(|q| q.path == file.path) => {
                    self.files.push(file);
                    added += 1;
                }
                Ok(_) => {}
                Err(e) => errors.push(format!("{e:#}")),
            }
        }
        if !self.files.is_empty() && self.selected.selected().is_none() {
            self.selected.select(Some(0));
        }
        self.message(
            if errors.is_empty() {
                format!("Added {added} file(s). F5 reviews the batch before upload.")
            } else {
                format!("Added {added} file(s). {}", errors.join("\n"))
            },
            !errors.is_empty(),
        );
    }

    fn start(&mut self, retry: bool) {
        if self.busy {
            return;
        }
        if let Err(e) = self.config.validate() {
            self.message(e.to_string(), true);
            self.screen = Screen::Settings;
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.busy = true;
        self.screen = Screen::Home;
        self.message("Starting upload…", false);
        git::start(
            Upload {
                paths: self.paths.clone(),
                config: self.config.clone(),
                files: self.files.clone(),
                tags: self.tags.clone(),
                notes: self.notes.clone(),
            },
            retry,
            tx,
        );
    }

    fn poll(&mut self) {
        let messages: Vec<_> = self
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for message in messages {
            match message {
                Progress::Status(s) => self.message(s, false),
                Progress::Done(result) => {
                    self.busy = false;
                    self.receiver = None;
                    match result {
                        Ok(s) => {
                            self.message(format!("Uploaded successfully: {s}"), false);
                            self.files.clear();
                            self.selected.select(None);
                            self.notes.clear();
                        }
                        Err(e) => self.message(e, true),
                    }
                }
            }
        }
    }

    fn editable(&mut self) -> Option<&mut String> {
        match self.screen {
            Screen::Settings => self.fields.get_mut(self.focus),
            Screen::Home => match self.focus {
                0 => Some(&mut self.input),
                2 => Some(&mut self.tags),
                3 => Some(&mut self.notes),
                _ => None,
            },
            _ => None,
        }
    }

    fn switch(&mut self, screen: Screen) {
        self.screen = screen;
        self.focus = 0;
        self.scroll = 0;
    }
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        ratatui::restore();
    }
}

pub fn run(skip_splash: bool, files: Vec<PathBuf>) -> Result<()> {
    let paths = Paths::new()?;
    let config = Config::load(&paths)?;
    let mut app = App::new(paths, config);
    if !files.is_empty() {
        app.add(files);
    }
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        ratatui::restore();
        previous_hook(info);
    }));
    execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    if !skip_splash {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(2200) {
            terminal.draw(|f| splash(f, start.elapsed().as_secs_f64()))?;
            if event::poll(Duration::from_millis(40))? {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if k.code == KeyCode::Char('c')
                            && k.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            return Ok(());
                        }
                        break;
                    }
                    Event::Paste(s) => {
                        if let Ok(paths) = ingest::dropped_paths(&s) {
                            app.add(paths);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    loop {
        app.poll();
        terminal.draw(|f| draw(f, &mut app))?;
        if !event::poll(Duration::from_millis(60))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'q'))
                {
                    if app.busy {
                        app.message(
                            "Upload is running. Wait for completion before quitting.",
                            true,
                        );
                    } else {
                        break;
                    }
                    continue;
                }
                if app.busy {
                    continue;
                }
                match key.code {
                    KeyCode::F(1) => app.switch(Screen::Help),
                    KeyCode::F(2) => {
                        app.diagnostics = git::doctor(&app.config);
                        app.switch(Screen::Doctor);
                    }
                    KeyCode::F(3) => app.switch(Screen::Settings),
                    KeyCode::F(4) => browse(&mut app, &mut terminal)?,
                    KeyCode::F(5) => {
                        if app.files.is_empty() {
                            app.message("Add files with F4 before reviewing an upload.", true);
                        } else if let Err(e) = app.config.validate() {
                            app.message(e.to_string(), true);
                            app.switch(Screen::Settings);
                        } else {
                            app.switch(Screen::Review);
                        }
                    }
                    KeyCode::F(6) => app.start(true),
                    KeyCode::F(7) => app.switch(Screen::History),
                    KeyCode::Esc => app.switch(Screen::Home),
                    KeyCode::Tab => app.focus = (app.focus + 1) % 4,
                    KeyCode::BackTab => app.focus = (app.focus + 3) % 4,
                    KeyCode::Enter => match app.screen {
                        Screen::Review => app.start(false),
                        Screen::Settings => {
                            let config = Config {
                                remote: app.fields[0].trim().into(),
                                project: app.fields[1].trim().into(),
                                author: app.fields[2].trim().into(),
                                email: app.fields[3].trim().into(),
                            };
                            match config.save(&app.paths) {
                                Ok(()) => {
                                    app.config = config;
                                    app.diagnostics = git::doctor(&app.config);
                                    app.switch(Screen::Home);
                                    app.message("Settings saved. Add files with F4; F5 reviews and uploads.", false);
                                }
                                Err(e) => app.message(format!("{e:#}"), true),
                            }
                        }
                        Screen::Home if app.focus == 0 => {
                            if app.input.trim().is_empty() {
                                browse(&mut app, &mut terminal)?;
                            } else {
                                match ingest::dropped_paths(&app.input) {
                                    Ok(paths) => {
                                        app.add(paths);
                                        app.input.clear();
                                    }
                                    Err(e) => app.message(e.to_string(), true),
                                }
                            }
                        }
                        _ => {}
                    },
                    KeyCode::Up | KeyCode::Down if app.screen == Screen::Home && app.focus == 1 => {
                        if !app.files.is_empty() {
                            let index = app.selected.selected().unwrap_or(0);
                            app.selected.select(Some(if key.code == KeyCode::Up {
                                index.saturating_sub(1)
                            } else {
                                (index + 1).min(app.files.len() - 1)
                            }));
                        }
                    }
                    KeyCode::Delete if app.screen == Screen::Home && app.focus == 1 => {
                        if let Some(index) =
                            app.selected.selected().filter(|i| *i < app.files.len())
                        {
                            app.files.remove(index);
                            app.selected.select(if app.files.is_empty() {
                                None
                            } else {
                                Some(index.min(app.files.len() - 1))
                            });
                        }
                    }
                    KeyCode::PageDown => app.scroll = app.scroll.saturating_add(5),
                    KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(5),
                    KeyCode::Backspace => {
                        if let Some(text) = app.editable() {
                            text.pop();
                        }
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(text) = app.editable() {
                            text.clear();
                        }
                    }
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if let Some(text) = app.editable() {
                            if text.len() < 8192 {
                                text.push(c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Paste(text) if !app.busy => {
                if app.screen == Screen::Home && app.focus <= 1 {
                    match ingest::dropped_paths(&text) {
                        Ok(paths) if !paths.is_empty() && paths.iter().all(|p| p.is_file()) => {
                            app.add(paths)
                        }
                        _ => {
                            app.input.push_str(&text);
                            app.focus = 0;
                        }
                    }
                } else if let Some(field) = app.editable() {
                    field.extend(text.chars().filter(|c| !c.is_control()).take(8192));
                }
            }
            Event::Mouse(mouse)
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) && !app.busy =>
            {
                let position = Position::new(mouse.column, mouse.row);
                if app.screen == Screen::Home && app.drop_area.contains(position) {
                    browse(&mut app, &mut terminal)?;
                } else if let Some(index) =
                    app.fields_area.iter().position(|r| r.contains(position))
                {
                    app.focus = index;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn browse(app: &mut App, terminal: &mut DefaultTerminal) -> Result<()> {
    // Native dialogs run on the main thread (required on macOS).
    let picked = rfd::FileDialog::new()
        .set_title("Aero Archive — select simulation files")
        .pick_files();
    app.switch(Screen::Home);
    if let Some(paths) = picked {
        app.add(paths);
    }
    terminal.clear()?;
    Ok(())
}

fn panel(title: impl Into<Line<'static>>, active: bool) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if active {
            GREEN
        } else {
            Color::Rgb(42, 65, 83)
        }))
}

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(Color::White)),
        area,
    );
    app.fields_area.clear();
    app.drop_area = Rect::default();
    if area.width < 66 || area.height < 24 {
        frame.render_widget(
            Paragraph::new(
                "AERO ARCHIVE\n\nResize your terminal to at least 66 × 24.\nCtrl+Q exits.",
            )
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let layout = Layout::vertical([
        Constraint::Length(if area.height >= 30 { 3 } else { 2 }),
        Constraint::Min(10),
        Constraint::Length(if area.height >= 30 { 6 } else { 4 }),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(area);
    let title = Line::from(vec![
        Span::styled(" ◉ AERO ARCHIVE ", Style::default().fg(GREEN).bold()),
        Span::styled(" / SIMULATION MISSION CONTROL", Style::default().fg(MUTED)),
    ]);
    frame.render_widget(
        Paragraph::new(title).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(CYAN)),
        ),
        layout[0],
    );
    match app.screen {
        Screen::Home => home(frame, app, layout[1]),
        Screen::Settings => settings(frame, app, layout[1]),
        Screen::Doctor => {
            let text = format!(
                "{}\n\n{}\n\nF2 rechecks · F3 opens settings · Esc opens the workspace",
                app.diagnostics.join("\n"),
                git::setup_help()
            );
            page(
                frame,
                " FIRST FLIGHT / SETUP CHECK ",
                &text,
                layout[1],
                app.scroll,
            );
        }
        Screen::History => {
            let history =
                fs::read_to_string(app.paths.data.join("history.log")).unwrap_or_default();
            let text = if history.is_empty() {
                "No successful uploads yet.\n\nF4 selects your first files.".into()
            } else {
                history.lines().rev().collect::<Vec<_>>().join("\n\n")
            };
            page(
                frame,
                " UPLOAD HISTORY / newest first ",
                &text,
                layout[1],
                app.scroll,
            );
        }
        Screen::Help => page(
            frame,
            " FLIGHT MANUAL ",
            "1  F3: Set your GitHub URL, project, author and email.\n2  F4 or click the drop box: select files in a native dialog.\n3  Drop files into the terminal, or paste quoted paths and Enter.\n4  Tab: move between path entry, queue, tags and review notes.\n5  Add comma-separated tags. File-type tags are automatic.\n6  F5: review the destination and batch; Enter uploads it.\n7  F6: resume a failed upload without creating another version.\n8  F7: view successful upload receipts.\n\nQueue: Up/Down selects a file; Delete removes it.\nText fields: Backspace edits; Ctrl+U clears.\nPgUp/PgDn scrolls the page and status details. Esc returns home.\nCtrl+Q exits. Any key skips the radar splash.\n\nDrag-and-drop depends on your terminal. If it sends a path as\nplain keystrokes, focus the path field first and press Enter.\nThe native file picker also works when terminal drops do not.\n\nFiles are copied; original data stays in place.\nEach batch contains manifest.json with SHA-256 checksums.\nThe managed checkout follows the repository's default branch.\nFiles above 95 MiB need a separate Git LFS workflow.\nGitHub authentication: gh auth login; gh auth setup-git.\nProtected branches require a repository workflow that allows\nthese commits; ask your repository owner about write access.",
            layout[1],
            app.scroll,
        ),
        Screen::Review => {
            let types = app
                .files
                .iter()
                .map(|f| f.kind.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let text = format!(
                "DESTINATION\n{}\n\nsim-data/{}/versions/<UTC version>/\n\nBATCH\n{} file(s) · {:.2} MiB\nAutomatic tags: {}\nYour tags: {}\nReview notes: {}\n\nFiles are copied, hashed, committed, then pushed.\nAn existing repository uses its default branch.\n\nENTER  Confirm upload       ESC  Continue editing",
                app.config.remote,
                app.config.project,
                app.files.len(),
                app.files.iter().map(|f| f.bytes).sum::<u64>() as f64 / 1048576.0,
                types,
                app.tags,
                app.notes
            );
            page(
                frame,
                " PRE-FLIGHT / REVIEW UPLOAD ",
                &text,
                layout[1],
                app.scroll,
            );
        }
    }
    let status_title = if app.busy {
        " TRANSMITTING / please wait "
    } else if app.error {
        " ACTION NEEDED / PgDn for details "
    } else {
        " FLIGHT LOG "
    };
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .style(Style::default().fg(if app.error { Color::LightRed } else { GREEN }))
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0))
            .block(panel(status_title, app.busy)),
        layout[2],
    );
    frame.render_widget(Paragraph::new("F1 Help  F2 Git  F3 Settings  F4 Browse  F5 Upload  F6 Retry  F7 History\nTab Focus · Esc Home · Ctrl+Q Quit").style(Style::default().fg(MUTED)), layout[3]);
}

fn home(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);
    app.drop_area = rows[0];
    app.fields_area = rows.to_vec();
    let prompt = if app.input.is_empty() {
        "Click or press F4 to browse · drop files here\nOr type / paste file paths, then press Enter"
    } else {
        app.input.as_str()
    };
    frame.render_widget(
        Paragraph::new(prompt)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .block(panel(" 01 / ADD SIMULATION DATA ", app.focus == 0)),
        rows[0],
    );
    let items: Vec<ListItem> = if app.files.is_empty() {
        vec![
            ListItem::new("  No files queued. OpenRocket · CFD · meshes · results · review assets")
                .style(Style::default().fg(MUTED)),
        ]
    } else {
        app.files
            .iter()
            .map(|file| {
                ListItem::new(format!(
                    "{:<12} {:>7.2} MiB  {}",
                    file.kind,
                    file.bytes as f64 / 1048576.0,
                    file.path.file_name().unwrap_or_default().to_string_lossy()
                ))
            })
            .collect()
    };
    let title = format!(
        " 02 / UPLOAD QUEUE · {} file(s) · Delete removes ",
        app.files.len()
    );
    frame.render_stateful_widget(
        List::new(items)
            .block(panel(title, app.focus == 1))
            .highlight_style(Style::default().bg(Color::Rgb(25, 49, 62)).fg(GREEN))
            .highlight_symbol("› "),
        rows[1],
        &mut app.selected,
    );
    text_field(
        frame,
        " 03 / TAGS · comma separated ",
        &app.tags,
        rows[2],
        app.focus == 2,
    );
    text_field(
        frame,
        " 04 / REVIEW NOTES · conditions, solver, purpose ",
        &app.notes,
        rows[3],
        app.focus == 3,
    );
}

fn settings(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);
    app.fields_area = rows[..4].to_vec();
    for (index, label) in [
        " GitHub URL · https://github.com/OWNER/REPO.git ",
        " Project slug · letters, digits, - or _ ",
        " Commit author name ",
        " Commit author email ",
    ]
    .into_iter()
    .enumerate()
    {
        text_field(
            frame,
            label,
            &app.fields[index],
            rows[index],
            app.focus == index,
        );
    }
    frame.render_widget(Paragraph::new("Tab / click selects a field · Enter saves · Esc returns\nAuthenticate outside the app: gh auth login; gh auth setup-git\nAuthor settings apply to this app. Tokens are never stored here.").style(Style::default().fg(MUTED)).wrap(Wrap { trim: false }), rows[4]);
}

fn text_field(frame: &mut Frame, label: &'static str, text: &str, area: Rect, active: bool) {
    let cursor = if active { "▏" } else { "" };
    let available = area.width.saturating_sub(4) as usize;
    let chars: Vec<_> = text.chars().collect();
    let tail: String = chars[chars.len().saturating_sub(available)..]
        .iter()
        .collect();
    frame.render_widget(
        Paragraph::new(format!("{tail}{cursor}"))
            .block(panel(label, active))
            .style(Style::default().fg(if active { GREEN } else { Color::White })),
        area,
    );
}

fn page(frame: &mut Frame, title: &'static str, text: &str, area: Rect, scroll: u16) {
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(title, true))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn splash(frame: &mut Frame, seconds: f64) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(4),
    ])
    .margin(1)
    .split(area);
    frame.render_widget(
        Paragraph::new("A E R O   A R C H I V E")
            .alignment(Alignment::Center)
            .style(Style::default().fg(GREEN).bold()),
        layout[0],
    );
    let angle = seconds * 3.2;
    let canvas = Canvas::default()
        .background_color(BG)
        .x_bounds([-110., 110.])
        .y_bounds([-105., 105.])
        .paint(move |ctx| {
            for radius in [25., 50., 75., 100.] {
                ctx.draw(&Circle {
                    x: 0.,
                    y: 0.,
                    radius,
                    color: Color::Rgb(29, 89, 69),
                });
            }
            for (x, y) in [(100., 0.), (-100., 0.), (0., 100.), (0., -100.)] {
                ctx.draw(&RadarLine {
                    x1: 0.,
                    y1: 0.,
                    x2: x,
                    y2: y,
                    color: Color::Rgb(25, 60, 55),
                });
            }
            for i in 0..18 {
                let a = angle - i as f64 * 0.035;
                let brightness = 220u8.saturating_sub(i * 10);
                ctx.draw(&RadarLine {
                    x1: 0.,
                    y1: 0.,
                    x2: 98. * a.cos(),
                    y2: 98. * a.sin(),
                    color: Color::Rgb(35, brightness, 105),
                });
            }
            for (x, y) in [(34., 47.), (-55., 23.), (22., -64.)] {
                ctx.draw(&Points {
                    coords: &[(x, y)],
                    color: GREEN,
                });
            }
            ctx.print(4., 3., Span::styled("+", Style::default().fg(CYAN)));
        });
    let width = layout[1]
        .width
        .min(layout[1].height.saturating_mul(2))
        .min(72);
    let radar_area = Rect::new(
        layout[1].x + (layout[1].width - width) / 2,
        layout[1].y,
        width,
        layout[1].height,
    );
    frame.render_widget(canvas, radar_area);
    frame.render_widget(Paragraph::new("SCANNING THE FLIGHT ENVELOPE\nTag · version · transmit\nPress any key to enter mission control").alignment(Alignment::Center).style(Style::default().fg(GREEN)), layout[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_screens_and_small_terminals_render() {
        let tmp = tempfile::tempdir().unwrap();
        let mut app = App::new(
            Paths {
                config: tmp.path().join("settings.json"),
                data: tmp.path().into(),
            },
            Config::default(),
        );
        for (width, height) in [(100, 36), (66, 24), (20, 5)] {
            let mut terminal =
                Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            for screen in [
                Screen::Home,
                Screen::Settings,
                Screen::Doctor,
                Screen::Help,
                Screen::Review,
                Screen::History,
            ] {
                app.screen = screen;
                terminal.draw(|f| draw(f, &mut app)).unwrap();
            }
            terminal.draw(|f| splash(f, 1.2)).unwrap();
        }
    }
}
