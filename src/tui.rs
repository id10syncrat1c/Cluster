use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell { ch: char, style: u8 }
impl Default for Cell { fn default() -> Self { Self { ch: ' ', style: 0 } } }

struct Message { role: u8, content: String }
struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), ResetColor, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Tui {
    width: u16, height: u16,
    previous: Vec<Cell>, current: Vec<Cell>,
    messages: Vec<Message>, input: Vec<char>,
    cursor: usize, scroll: usize, status: String,
}
impl Tui {
    pub fn new(provider: &str, model: &str) -> Result<Self> {
        let (width, height) = terminal::size()?;
        let size = width as usize * height as usize;
        Ok(Self {
            width, height,
            previous: vec![Cell { ch: '\0', style: 255 }; size],
            current: vec![Cell::default(); size],
            messages: Vec::new(), input: Vec::new(), cursor: 0, scroll: 0,
            status: format!("{provider} · {model}"),
        })
    }

    pub fn input(&mut self, prompt: Option<&str>) -> Result<Option<String>> {
        let _guard = TerminalGuard::enter()?;
        self.status = prompt.unwrap_or("Enter send · Shift+Enter newline · /help").to_string();
        self.input.clear(); self.cursor = 0;
        loop {
            self.render()?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Resize(w, h) => self.resize(w, h),
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(None),
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => { self.input.insert(self.cursor, '\n'); self.cursor += 1; }
                        KeyCode::Enter => {
                            let value = self.input.iter().collect::<String>().trim().to_string();
                            self.input.clear();
                            self.cursor = 0;
                            return Ok(Some(value));
                        },
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => { self.input.insert(self.cursor, c); self.cursor += 1; }
                        KeyCode::Backspace if self.cursor > 0 => { self.cursor -= 1; self.input.remove(self.cursor); }
                        KeyCode::Delete if self.cursor < self.input.len() => { self.input.remove(self.cursor); }
                        KeyCode::Left if self.cursor > 0 => self.cursor -= 1,
                        KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
                        KeyCode::Home => self.cursor = self.input.iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1),
                        KeyCode::End => self.cursor = self.input.iter().position(|c| *c == '\n').map_or(self.input.len(), |i| i),
                        KeyCode::PageUp => self.scroll = self.scroll.saturating_add(self.height.saturating_sub(5) as usize),
                        KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(self.height.saturating_sub(5) as usize),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }

    pub fn add_message(&mut self, role: u8, content: String) { self.messages.push(Message { role, content }); self.scroll = 0; }
    pub fn set_status(&mut self, provider: &str, model: &str) { self.status = format!("{provider} · {model}"); }
    pub fn set_notice(&mut self, notice: &str) { self.status = notice.to_string(); }
    pub fn clear_messages(&mut self) { self.messages.clear(); self.scroll = 0; }

    pub fn stream(&mut self, receiver: std::sync::mpsc::Receiver<String>) -> Result<()> {
        let _guard = TerminalGuard::enter()?;
        self.messages.push(Message { role: 1, content: String::new() });
        self.status = "Generating · PgUp/PgDn scroll".into();
        loop {
            match receiver.recv_timeout(Duration::from_millis(35)) {
                Ok(chunk) => { if let Some(message) = self.messages.last_mut() { message.content.push_str(&chunk); } self.render()?; }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Resize(w, h) => self.resize(w, h),
                            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                                KeyCode::PageUp => self.scroll = self.scroll.saturating_add(self.height.saturating_sub(5) as usize),
                                KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(self.height.saturating_sub(5) as usize),
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
        self.status = "Response complete · Enter to continue".into();
        self.render()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1); self.height = height.max(1);
        let size = self.width as usize * self.height as usize;
        self.previous = vec![Cell { ch: '\0', style: 255 }; size];
        self.current = vec![Cell::default(); size];
    }
    fn put(&mut self, x: usize, y: usize, ch: char, style: u8) {
        if x < self.width as usize && y < self.height as usize {
            self.current[y * self.width as usize + x] = Cell { ch, style };
        }
    }
    fn text(&mut self, x: usize, y: usize, value: &str, style: u8, max: usize) {
        for (offset, ch) in value.chars().take(max).enumerate() { if ch == '\n' { break; } self.put(x + offset, y, ch, style); }
    }
    fn lines(&self, width: usize) -> Vec<(u8, String)> {
        let mut lines = Vec::new();
        for message in &self.messages {
            let prefix = if message.role == 0 { "you  " } else { "ai   " };
            let mut line = String::new(); let mut count = 0usize;
            for word in message.content.split_inclusive(char::is_whitespace) {
                for ch in word.chars() {
                    if ch == '\n' { lines.push((message.role, format!("{prefix}{line}"))); line.clear(); count = 0; }
                    else {
                        if count >= width.saturating_sub(prefix.len()).max(1) { lines.push((message.role, format!("{prefix}{line}"))); line.clear(); count = 0; }
                        line.push(ch); count += 1;
                    }
                }
            }
            if !line.is_empty() || message.content.is_empty() { lines.push((message.role, format!("{prefix}{line}"))); }
            lines.push((2, String::new()));
        }
        lines
    }
    fn render(&mut self) -> Result<()> {
        let width = self.width as usize; let height = self.height as usize;
        self.current.fill(Cell::default());
        self.text(1, 0, " LIBRA / CLUSTER", 1, width.saturating_sub(2));
        let status = self.status.clone();
        let sx = width.saturating_sub(status.chars().count() + 2);
        self.text(sx, 0, &status, 2, width.saturating_sub(sx));
        for x in 0..width { self.put(x, 1, '─', 3); }
        if height > 4 {
            let visible = height - 5; let lines = self.lines(width.saturating_sub(4));
            let end = lines.len().saturating_sub(self.scroll); let start = end.saturating_sub(visible);
            for (row, (role, line)) in lines[start..end].iter().enumerate() {
                self.text(2, row + 2, line, if *role == 0 { 1 } else if *role == 2 { 3 } else { 0 }, width.saturating_sub(4));
            }
        }
        let cy = height.saturating_sub(3);
        for x in 0..width { self.put(x, cy, '─', 3); }
        self.text(1, cy + 1, "› ", 1, 2);
        let input = self.input.iter().collect::<String>();
        let max = width.saturating_sub(4);
        let input_start = self.cursor.saturating_sub(max);
        let visible = input.chars().skip(input_start).take(max).collect::<String>();
        self.text(3, cy + 1, &visible, 0, max);
        self.text(1, height.saturating_sub(1), "Shift+Enter newline · PgUp/PgDn scroll · Ctrl+C exit", 2, width.saturating_sub(2));
        let mut stdout = io::stdout();
        for y in 0..height {
            let mut x = 0usize;
            while x < width {
                let i = y * width + x;
                if self.current[i] == self.previous[i] { x += 1; continue; }
                let style = self.current[i].style;
                queue!(stdout, MoveTo(x as u16, y as u16))?;
                match style {
                    1 => queue!(stdout, SetForegroundColor(Color::Rgb { r: 200, g: 255, b: 0 }))?,
                    2 | 3 => queue!(stdout, SetForegroundColor(Color::DarkGrey))?,
                    _ => queue!(stdout, SetForegroundColor(Color::White))?,
                }
                while x < width {
                    let cell = self.current[y * width + x];
                    if cell == self.previous[y * width + x] || cell.style != style { break; }
                    queue!(stdout, Print(cell.ch))?;
                    self.previous[y * width + x] = cell; x += 1;
                }
            }
        }
        let cursor_x = 3usize.saturating_add(self.cursor.saturating_sub(input_start));
        queue!(
            stdout,
            ResetColor,
            MoveTo(cursor_x.min(width.saturating_sub(1)) as u16, cy.saturating_add(1) as u16),
            Show
        )?;
        stdout.flush()?;
        Ok(())
    }
}
