use std::{
    fs::{self, File},
    io::{self, Write},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Represents the current state of the text editor
struct Editor {
    filename: String,
    content: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
    scroll_y: usize,
    modified: bool,
    search_query: Option<String>,
}

impl Editor {
    /// Load file or start with an empty buffer
    fn open(filename: String) -> io::Result<Self> {
        let content = fs::read_to_string(&filename)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect::<Vec<_>>();

        Ok(Self {
            filename,
            content,
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            modified: false,
            search_query: None,
        })
    }

    /// Save file to disk (optionally under a new name)
    fn save(&mut self, new_name: Option<String>) -> io::Result<()> {
        if let Some(name) = new_name {
            self.filename = name;
        }
        let mut file = File::create(&self.filename)?;
        for line in &self.content {
            writeln!(file, "{}", line)?;
        }
        self.modified = false;
        Ok(())
    }

    /// Insert a character at the current cursor position
    fn insert_char(&mut self, ch: char) {
        if self.cursor_y >= self.content.len() {
            self.content.push(String::new());
        }
        self.content[self.cursor_y].insert(self.cursor_x, ch);
        self.cursor_x += 1;
        self.modified = true;
    }

    /// Handle line breaks (Enter key)
    fn insert_newline(&mut self) {
        if self.cursor_y >= self.content.len() {
            self.content.push(String::new());
        } else {
            let rest = self.content[self.cursor_y].split_off(self.cursor_x);
            self.content.insert(self.cursor_y + 1, rest);
        }
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.modified = true;
    }

    /// Delete a character (Backspace)
    fn delete_char(&mut self) {
        if self.cursor_y < self.content.len() && self.cursor_x > 0 {
            self.content[self.cursor_y].remove(self.cursor_x - 1);
            self.cursor_x -= 1;
            self.modified = true;
        } else if self.cursor_y > 0 {
            let current = self.content.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = self.content[self.cursor_y].len();
            self.content[self.cursor_y].push_str(&current);
            self.modified = true;
        }
    }

    /// Move the cursor (with basic bounds and scrolling)
    fn move_cursor(&mut self, code: KeyCode, visible_height: usize) {
        let len = self.current_line().map(|s| s.len()).unwrap_or(0);
        match code {
            KeyCode::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    if self.cursor_y < self.scroll_y {
                        self.scroll_y -= 1;
                    }
                    self.cursor_x = self.cursor_x.min(len);
                }
            }
            KeyCode::Down => {
                if self.cursor_y + 1 < self.content.len() {
                    self.cursor_y += 1;
                    if self.cursor_y >= self.scroll_y + visible_height {
                        self.scroll_y += 1;
                    }
                    self.cursor_x = self.cursor_x.min(len);
                }
            }
            KeyCode::Left => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                } else if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.cursor_x = len;
                }
            }
            KeyCode::Right => {
                if self.cursor_x < len {
                    self.cursor_x += 1;
                } else if self.cursor_y + 1 < self.content.len() {
                    self.cursor_y += 1;
                    self.cursor_x = 0;
                }
            }
            _ => {}
        }
    }

    fn current_line(&self) -> Option<&String> {
        self.content.get(self.cursor_y)
    }

    /// Search for a term in the file and move cursor
    fn search(&mut self, query: String) {
        self.search_query = Some(query.clone());
        if let Some((y, _)) = self
            .content
            .iter()
            .enumerate()
            .find(|(_, line)| line.contains(&query))
        {
            self.cursor_y = y;
            self.cursor_x = self.content[y].find(&query).unwrap_or(0);
        }
    }
}

/// Prompt user for input text (used for save or search dialogs)
fn prompt_input(
    term: &mut Terminal<CrosstermBackend<io::Stdout>>,
    message: &str,
) -> io::Result<String> {
    let mut input = String::new();
    loop {
        term.draw(|f| {
            let area = centered_rect(60, 20, f.size());
            let block = Block::default()
                .title(message)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));
            let paragraph = Paragraph::new(input.clone()).block(block);
            f.render_widget(paragraph, area);
        })?;

        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Enter => break,
                KeyCode::Char(c) => input.push(c),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Esc => return Ok(String::new()),
                _ => {}
            }
        }
    }
    Ok(input)
}

/// Creates a centered rectangle for popups
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

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut editor = Editor::open("untitled.txt".into())?;
    let mut last_blink = Instant::now();
    let mut show_cursor = true;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
                .split(size);

            let visible_lines = (layout[0].height - 2) as usize;
            let content_to_show = editor
                .content
                .iter()
                .skip(editor.scroll_y)
                .take(visible_lines)
                .map(|l| Line::from(Span::raw(l.clone())))
                .collect::<Vec<_>>();

            let main_block = Block::default().borders(Borders::ALL).title(Span::styled(
                "Rano — Text Editor",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

            let paragraph = Paragraph::new(content_to_show)
                .block(main_block)
                .style(Style::default().fg(Color::White));

            f.render_widget(paragraph, layout[0]);

            let status = format!(
                "File: {} | Line: {} | Col: {} | {}",
                editor.filename,
                editor.cursor_y + 1,
                editor.cursor_x + 1,
                if editor.modified { "Modified" } else { "Saved" }
            );
            let status_bar = Paragraph::new(Line::from(Span::styled(
                status,
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            f.render_widget(status_bar, layout[1]);

            if show_cursor {
                let x = editor.cursor_x as u16 + 1;
                let y = (editor.cursor_y - editor.scroll_y) as u16 + 1;
                f.set_cursor(layout[0].x + x, layout[0].y + y);
            }
        })?;

        if last_blink.elapsed() >= Duration::from_millis(500) {
            show_cursor = !show_cursor;
            last_blink = Instant::now();
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                match (code, modifiers) {
                    (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                        if editor.modified {
                            let save = prompt_input(
                                &mut terminal,
                                "Unsaved changes. Save before exit? (y/n)",
                            )?;
                            if save.trim().eq_ignore_ascii_case("y") {
                                editor.save(None)?;
                            }
                        }
                        break;
                    }
                    (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                        let new_name = prompt_input(&mut terminal, "Save as:")?;
                        if !new_name.is_empty() {
                            editor.save(Some(new_name))?;
                        }
                    }
                    (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                        let query = prompt_input(&mut terminal, "Search for:")?;
                        if !query.is_empty() {
                            editor.search(query);
                        }
                    }
                    (KeyCode::Enter, _) => editor.insert_newline(),
                    (KeyCode::Backspace, _) => editor.delete_char(),
                    (KeyCode::Char(c), _) => editor.insert_char(c),
                    (kc, _) => editor.move_cursor(kc, (terminal.size()?.height - 2) as usize),
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)?;
    Ok(())
}
