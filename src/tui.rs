use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    execute, queue,
    event::{DisableMouseCapture, EnableMouseCapture},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    ch: char,
    style: u8,
}
impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', style: 0 }
    }
}

struct Message {
    role: u8,
    content: String,
}
struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), ResetColor, Show, DisableMouseCapture, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Tui {
    width: u16,
    height: u16,
    previous: Vec<Cell>,
    current: Vec<Cell>,
    messages: Vec<Message>,
    input: Vec<char>,
    cursor: usize,
    scroll: usize,
    status: String,
    placeholder: Option<String>,
    home_mode: bool,
    mask_input: bool,
    resume_prompt: Option<String>,
    home_placeholders: Vec<String>,
    placeholder_index: usize,
    last_placeholder_rotation: Instant,
    version: String,
}
impl Tui {
    pub fn new(provider: &str, model: &str) -> Result<Self> {
        let (width, height) = terminal::size()?;
        let size = width as usize * height as usize;
        Ok(Self {
            width,
            height,
            previous: vec![
                Cell {
                    ch: '\0',
                    style: 255
                };
                size
            ],
            current: vec![Cell::default(); size],
            messages: Vec::new(),
            input: Vec::new(),
            cursor: 0,
            scroll: 0,
            status: format!("{provider} · {model}"),
            placeholder: None,
            home_mode: false,
            mask_input: false,
            resume_prompt: None,
            home_placeholders: Vec::new(),
            placeholder_index: 0,
            last_placeholder_rotation: Instant::now(),
            version: format!("v{}", env!("CARGO_PKG_VERSION")),
        })
    }

    fn credits_text() -> String {
        let encoded = [0x17u8, 0x3b, 0x3e, 0x3f, 0x7a, 0x2d, 0x33, 0x2e, 0x32, 0x7a, 0xb8, 0xc7, 0xfe, 0xb5, 0xe2, 0xd5, 0x7a, 0x38, 0x23, 0x7a, 0x29, 0x3f, 0x36, 0x23, 0x35, 0x28, 0x2f, 0x29];
        String::from_utf8(encoded.iter().map(|byte| *byte ^ 0x5a).collect()).unwrap()
    }

    pub fn menu(&mut self, title: &str, items: &[(String, bool)]) -> Result<Option<usize>> {
        let _guard = TerminalGuard::enter()?;
        let selectable = items.iter().position(|item| item.1);
        let Some(mut selected) = selectable else {
            return Ok(None);
        };
        loop {
            let mut stdout = io::stdout();
            let (screen_width, screen_height) = terminal::size()?;
            let title_rows = title.lines().count();
            let first_item_row = title_rows + 2;
            let visible_rows = screen_height.saturating_sub(first_item_row as u16 + 2).max(1) as usize;
            let start = selected.saturating_sub(visible_rows.saturating_sub(1));
            let end = (start + visible_rows).min(items.len());
            execute!(stdout, terminal::Clear(ClearType::All), MoveTo(0, 0))?;
            queue!(stdout, SetForegroundColor(Color::Rgb { r: 200, g: 255, b: 0 }), Print(format!("  {title}\n")))?;
            queue!(stdout, SetForegroundColor(Color::DarkGrey), Print("  Use ↑/↓ to navigate · Enter to select · Esc to go back\n\n"))?;
            for index in start..end {
                let (label, enabled) = &items[index];
                if !enabled {
                    queue!(stdout, SetForegroundColor(Color::DarkGrey), Print(format!("  {label}\n")))?;
                } else if index == selected {
                    queue!(stdout, SetForegroundColor(Color::Rgb { r: 200, g: 255, b: 0 }), Print(format!("  › {label}\n")))?;
                } else {
                    queue!(stdout, SetForegroundColor(Color::White), Print(format!("    {label}\n")))?;
                }
            }
            queue!(stdout, ResetColor)?;
            stdout.flush()?;
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                        let (width, height) = (screen_width, screen_height);
                        if title.contains("Release Libra") && mouse.row == height.saturating_sub(1) && mouse.column < 14 {
                            if let Some(index) = items.iter().position(|item| item.0 == "Changelogs" && item.1) {
                                return Ok(Some(index));
                            }
                        }
                        if mouse.row >= first_item_row as u16 {
                            let index = start + (mouse.row - first_item_row as u16) as usize;
                            if index < items.len() && items[index].1 {
                                return Ok(Some(index));
                            }
                        }
                    }
                    Event::Key(key) => {
                    if key.kind != KeyEventKind::Press { continue; }
                    match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Enter => return Ok(Some(selected)),
                        KeyCode::Up | KeyCode::Char('k') => {
                            let mut index = selected;
                            for _ in 0..items.len() {
                                index = if index == 0 { items.len() - 1 } else { index - 1 };
                                if items[index].1 { selected = index; break; }
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let mut index = selected;
                            for _ in 0..items.len() {
                                index = (index + 1) % items.len();
                                if items[index].1 { selected = index; break; }
                            }
                        }
                        _ => {}
                    }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn input(&mut self, prompt: Option<&str>) -> Result<Option<String>> {
        self.input_mode(prompt, false, false)
    }

    pub fn home_input(&mut self, prompt: Option<&str>) -> Result<Option<String>> {
        self.input_mode(prompt, true, false)
    }

    pub fn secret_input(&mut self, prompt: &str) -> Result<Option<String>> {
        self.input_mode(Some(prompt), false, true)
    }

    pub fn set_home_suggestions(&mut self, suggestions: Vec<String>) {
        self.home_placeholders = suggestions;
    }

    pub fn set_resume_prompt(&mut self, preview: Option<String>) {
        self.resume_prompt = preview;
    }

    fn input_mode(
        &mut self,
        prompt: Option<&str>,
        home_mode: bool,
        mask_input: bool,
    ) -> Result<Option<String>> {
        let _guard = TerminalGuard::enter()?;
        self.home_mode = home_mode;
        self.mask_input = mask_input;
        if prompt.is_none() {
            self.status = "Enter send · Ctrl+N newline · /help".to_string();
        }
        self.placeholder = prompt.map(str::to_owned);
        if self.home_mode {
            if let Some(index) = self.home_placeholders.iter().position(|item| Some(item) == self.placeholder.as_ref()) {
                self.placeholder_index = index;
            }
        }
        self.last_placeholder_rotation = Instant::now();
        self.input.clear();
        self.cursor = 0;
        loop {
            if self.home_mode
                && self.input.is_empty()
                && self.home_placeholders.len() > 1
                && self.last_placeholder_rotation.elapsed() >= Duration::from_secs(4)
            {
                self.placeholder_index = (self.placeholder_index + 1) % self.home_placeholders.len();
                self.placeholder = Some(self.home_placeholders[self.placeholder_index].clone());
                self.last_placeholder_rotation = Instant::now();
            }
            self.render()?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Resize(w, h) => self.resize(w, h),
                    Event::Mouse(mouse) if self.home_mode && mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                        let logo_y = self.height.saturating_sub(20) / 2;
                        let box_y = logo_y + 7;
                        let panel_y = box_y + 4;
                        let buttons_y = panel_y + 3;
                        let panel_left = self.width.saturating_sub(self.width.saturating_sub(8).min(78).max(24)) / 2;
                        if self.resume_prompt.is_some() && mouse.row == buttons_y {
                            if mouse.column < self.width / 2 && mouse.column >= panel_left {
                                return Ok(Some("/resume".into()));
                            }
                            if mouse.column >= self.width / 2 {
                                self.resume_prompt = None;
                            }
                        } else if mouse.row == self.height.saturating_sub(1) && (1..13).contains(&mouse.column) {
                            return Ok(Some("/changelog".into()));
                        }
                    }
                    Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::ALT) => return Ok(Some("/settings".into())),
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::ALT) => return Ok(Some("/sessions".into())),
                        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(Some("/commands".into())),
                        KeyCode::Tab => return Ok(Some("/sessions".into())),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(None),
                        KeyCode::Char(c) if self.home_mode && self.input.is_empty() && self.resume_prompt.is_some() && c.to_ascii_lowercase() == 'y' => {
                            return Ok(Some("/resume".into()));
                        }
                        KeyCode::Char(c) if self.home_mode && self.input.is_empty() && self.resume_prompt.is_some() && c.to_ascii_lowercase() == 'n' => {
                            self.resume_prompt = None;
                        }
                        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.input.insert(self.cursor, '\n');
                            self.cursor += 1;
                        }
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            self.input.insert(self.cursor, '\n');
                            self.cursor += 1;
                        }
                        KeyCode::Enter => {
                            let value = self.input.iter().collect::<String>().trim().to_string();
                            self.input.clear();
                            self.cursor = 0;
                            self.placeholder = None;
                            self.home_mode = false;
                            self.mask_input = false;
                            return Ok(Some(value));
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.input.insert(self.cursor, c);
                            self.cursor += 1;
                        }
                        KeyCode::Backspace if self.cursor > 0 => {
                            self.cursor -= 1;
                            self.input.remove(self.cursor);
                        }
                        KeyCode::Delete if self.cursor < self.input.len() => {
                            self.input.remove(self.cursor);
                        }
                        KeyCode::Left if self.cursor > 0 => self.cursor -= 1,
                        KeyCode::Right if self.cursor < self.input.len() => self.cursor += 1,
                        KeyCode::Home => {
                            self.cursor = self.input[..self.cursor]
                                .iter()
                                .rposition(|c| *c == '\n')
                                .map_or(0, |i| i + 1);
                        }
                        KeyCode::End => {
                            self.cursor += self.input[self.cursor..]
                                .iter()
                                .position(|c| *c == '\n')
                                .unwrap_or(self.input.len() - self.cursor);
                        }
                        KeyCode::PageUp => self.scroll = self.scroll.saturating_add(self.page_size()),
                        KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(self.page_size()),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }

    pub fn add_message(&mut self, role: u8, content: String) {
        self.add_message_with_preview(role, content, true);
    }

    pub fn add_message_with_preview(&mut self, role: u8, content: String, code_depth_preview: bool) {
        let content = if role == 1 && !code_depth_preview {
            Self::compact_code_preview(&content)
        } else {
            content
        };
        self.messages.push(Message { role, content });
        self.scroll = 0;
    }
    pub fn set_status(&mut self, provider: &str, model: &str) {
        self.status = format!("{provider} · {model}");
    }
    pub fn set_notice(&mut self, notice: &str) {
        self.status = notice.to_string();
    }
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll = 0;
    }

    fn is_code_preview_line(line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with('+')
            || line.starts_with('-')
            || line.starts_with("import ")
            || line.starts_with("from ")
            || line.starts_with("def ")
            || line.starts_with("class ")
            || line.starts_with("const ")
            || line.starts_with("let ")
            || line.starts_with("fn ")
            || line.starts_with("pub ")
            || line.starts_with("use ")
    }

    fn compact_code_preview(content: &str) -> String {
        let mut output = Vec::new();
        let mut skip_code = false;
        for line in content.lines() {
            if line.starts_with("Edited ") {
                output.push(line);
                skip_code = true;
            } else if skip_code && (line.trim().is_empty() || Self::is_code_preview_line(line)) {
                if line.trim().is_empty() {
                    skip_code = false;
                    output.push("");
                }
            } else {
                skip_code = false;
                output.push(line);
            }
        }
        output.join("\n")
    }

    pub fn stream(&mut self, receiver: std::sync::mpsc::Receiver<String>, code_depth_preview: bool, show_thought_process: bool) -> Result<()> {
        let _guard = TerminalGuard::enter()?;
        self.messages.push(Message {
            role: 1,
            content: String::new(),
        });
        self.status = if show_thought_process {
            "Thinking · streaming response · PgUp/PgDn to scroll".into()
        } else {
            "Generating response".into()
        };
        let mut raw_output = String::new();
        loop {
            match receiver.recv_timeout(Duration::from_millis(35)) {
                Ok(chunk) => {
                    raw_output.push_str(&chunk);
                    if let Some(message) = self.messages.last_mut() {
                        if code_depth_preview {
                            message.content.push_str(&chunk);
                        } else {
                            message.content = Self::compact_code_preview(&raw_output);
                        }
                    }
                    self.render()?;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Resize(w, h) => self.resize(w, h),
                            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                                KeyCode::PageUp => {
                                    self.scroll = self.scroll.saturating_add(self.page_size())
                                }
                                KeyCode::PageDown => {
                                    self.scroll = self.scroll.saturating_sub(self.page_size())
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
        if !code_depth_preview {
            if let Some(message) = self.messages.last_mut() {
                message.content = Self::compact_code_preview(&raw_output);
            }
        }
        self.status = if show_thought_process { "Thought process complete".into() } else { "Response complete".into() };
        self.render()
    }

    fn page_size(&self) -> usize {
        self.height.saturating_sub(7).max(1) as usize
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
        let size = self.width as usize * self.height as usize;
        self.previous = vec![
            Cell {
                ch: '\0',
                style: 255
            };
            size
        ];
        self.current = vec![Cell::default(); size];
    }

    fn put(&mut self, x: usize, y: usize, ch: char, style: u8) {
        if x < self.width as usize && y < self.height as usize {
            self.current[y * self.width as usize + x] = Cell { ch, style };
        }
    }

    fn text(&mut self, x: usize, y: usize, value: &str, style: u8, max: usize) {
        for (offset, ch) in value.chars().take(max).enumerate() {
            if ch == '\n' {
                break;
            }
            self.put(x + offset, y, ch, style);
        }
    }

    fn clean_markdown(line: &str) -> String {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let mut value = trimmed.replace("**", "").replace("__", "").replace('`', "");
        if let Some(rest) = value.strip_prefix("## ") {
            value = format!("▌ {rest}");
        } else if let Some(rest) = value.strip_prefix("# ") {
            value = format!("▌ {rest}");
        } else if let Some(rest) = value.strip_prefix("- ") {
            value = format!("• {rest}");
        } else if let Some(rest) = value.strip_prefix("* ") {
            value = format!("• {rest}");
        }
        format!("{}{}", " ".repeat(indent.min(4)), value)
    }

    fn lines(&self, width: usize) -> Vec<(u8, String)> {
        let mut lines = Vec::new();
        for (index, message) in self.messages.iter().enumerate() {
            let label = if message.role == 0 { "you  " } else { "ai   " };
            let continuation = "     ";
            let mut first = true;
            for raw in message.content.lines() {
                let cleaned = Self::clean_markdown(raw);
                if cleaned.is_empty() {
                    lines.push((2, String::new()));
                    first = false;
                    continue;
                }
                let words = cleaned.split_whitespace();
                let mut line = String::new();
                let mut prefix = label;
                for word in words {
                    let available = width.saturating_sub(prefix.chars().count()).max(1);
                    let mut remaining = word.chars().collect::<Vec<_>>();
                    while !remaining.is_empty() {
                        let room = available
                            .saturating_sub(line.chars().count() + usize::from(!line.is_empty()));
                        if room == 0 {
                            lines.push((message.role, format!("{prefix}{line}")));
                            prefix = continuation;
                            line.clear();
                            continue;
                        }
                        if remaining.len() > room {
                            if !line.is_empty() {
                                lines.push((message.role, format!("{prefix}{line}")));
                                prefix = continuation;
                                line.clear();
                            } else {
                                let chunk = remaining.drain(..room).collect::<String>();
                                lines.push((message.role, format!("{prefix}{chunk}")));
                                prefix = continuation;
                            }
                        } else {
                            if !line.is_empty() {
                                line.push(' ');
                            }
                            line.push_str(&remaining.drain(..).collect::<String>());
                        }
                    }
                    if !line.is_empty() && line.chars().count() >= available {
                        lines.push((message.role, format!("{prefix}{line}")));
                        prefix = continuation;
                        line.clear();
                    }
                }
                if !line.is_empty() {
                    lines.push((message.role, format!("{prefix}{line}")));
                } else if first && message.content.is_empty() {
                    lines.push((message.role, label.trim_end().to_string()));
                }
                first = false;
            }
            if index + 1 < self.messages.len() {
                lines.push((2, String::new()));
            }
        }
        lines
    }

    fn render(&mut self) -> Result<()> {
        let width = self.width as usize;
        let height = self.height as usize;
        self.current.fill(Cell::default());
        let input = if self.mask_input {
            "*".repeat(self.input.len())
        } else {
            self.input.iter().collect::<String>()
        };
        let input_lines = input.split('\n').collect::<Vec<_>>();
        let (cursor_x, cursor_y);
        if self.home_mode {
            let logo = [
                "█▀▀█ █▀▀█ █▀▀█ █▀▀▄ █▀▀▀ █▀▀█ █▀▀█ █▀▀█",
                "█  █ █  █ █▀▀▀ █  █ █    █  █ █  █ █▀▀▀",
                "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
            ];
            let logo_width = logo[0].chars().count();
            let logo_x = width.saturating_sub(logo_width) / 2;
            let logo_y = height.saturating_sub(20) / 2;
            let brand = "Cluster";
            self.text(width.saturating_sub(brand.chars().count()) / 2, logo_y, brand, 1, brand.chars().count());
            for (row, line) in logo.iter().enumerate() {
                self.text(logo_x, logo_y + row + 1, line, 1, width.saturating_sub(logo_x));
            }
            let subtitle = format!("Release Libra {}", self.version);
            self.text(width.saturating_sub(subtitle.chars().count()) / 2, logo_y + 5, &subtitle, 2, subtitle.chars().count());
            let box_width = width.saturating_sub(8).min(78).max(24);
            let box_x = width.saturating_sub(box_width) / 2;
            let box_y = logo_y + 7;
            self.text(box_x, box_y, &format!("┏{}┓", "━".repeat(box_width.saturating_sub(2))), 3, box_width);
            self.text(box_x, box_y + 1, "┃", 1, 1);
            if input.is_empty() {
                if let Some(placeholder) = self.placeholder.clone() {
                    self.text(box_x + 2, box_y + 1, &placeholder, 3, box_width.saturating_sub(4));
                }
            } else {
                self.text(box_x + 2, box_y + 1, &input, 0, box_width.saturating_sub(4));
            }
            self.text(box_x + box_width.saturating_sub(1), box_y + 1, "┃", 1, 1);
            self.text(box_x, box_y + 2, &format!("┗{}┛", "━".repeat(box_width.saturating_sub(2))), 3, box_width);
            if let Some(preview) = self.resume_prompt.clone() {
                let panel_y = box_y + 4;
                self.text(box_x, panel_y, &format!("┏{}┓", "━".repeat(box_width.saturating_sub(2))), 3, box_width);
                self.text(box_x + 2, panel_y + 1, "Resume your last session?", 1, box_width.saturating_sub(4));
                self.text(box_x + 2, panel_y + 2, &format!("Last message: {preview}"), 2, box_width.saturating_sub(4));
                let buttons = "[ Yes ( Y ) ]       [ No ( N ) ]";
                self.text(box_x + box_width.saturating_sub(buttons.chars().count()) / 2, panel_y + 3, buttons, 1, buttons.chars().count());
                self.text(box_x, panel_y + 4, &format!("┗{}┛", "━".repeat(box_width.saturating_sub(2))), 3, box_width);
            }
            let shortcuts = "Alt+E settings   Alt+T switch session   Ctrl+P commands";
            self.text(width.saturating_sub(shortcuts.chars().count() + 2), height.saturating_sub(2), shortcuts, 2, width.saturating_sub(2));
            self.text(1, height.saturating_sub(1), "Changelogs", 2, 12);
            let credit = Self::credits_text();
            self.text(width.saturating_sub(credit.chars().count() + 2), height.saturating_sub(1), &credit, 2, width.saturating_sub(2));
            let cursor_col = self.input.iter().take(self.cursor).count();
            cursor_x = (box_x + 2 + cursor_col).min(width.saturating_sub(1));
            cursor_y = box_y + 1;
        }
                self.text(box_x + 2, box_y + 1, &input, 0, box_width.saturating_sub(4));
            }
            self.text(box_x + box_width.saturating_sub(1), box_y + 1, "┃", 1, 1);
            self.text(box_x, box_y + 2, &format!("┗{}┛", "━".repeat(box_width.saturating_sub(2))), 3, box_width);
            let status = format!("{} · {}", self.status, self.version);
            let status_x = width.saturating_sub(status.chars().count() + 2);
            self.text(status_x, height.saturating_sub(1), &status, 2, width.saturating_sub(status_x));
            let tip = self.tip.clone();
            self.text(2, height.saturating_sub(4), &tip, 2, width.saturating_sub(4));
            let shortcuts = "Tab sessions   Ctrl+P commands";
            self.text(width.saturating_sub(shortcuts.chars().count() + 2), height.saturating_sub(2), shortcuts, 2, width.saturating_sub(2));
            let cursor_col = self.input.iter().take(self.cursor).count();
            cursor_x = (box_x + 2 + cursor_col).min(width.saturating_sub(1));
            cursor_y = box_y + 1;
        } else {
            self.text(1, 0, " LIBRA / CLUSTER", 1, width.saturating_sub(2));
            let status = self.status.clone();
            let status_len = status.chars().count().min(width.saturating_sub(20));
            let sx = width.saturating_sub(status_len + 2);
            self.text(sx, 0, &status, 2, width.saturating_sub(sx));
            for x in 0..width { self.put(x, 1, '─', 3); }
            let input_top = height.saturating_sub(5);
            if input_top > 2 {
                let visible = input_top - 2;
                let lines = self.lines(width.saturating_sub(4));
                let end = lines.len().saturating_sub(self.scroll);
                let start = end.saturating_sub(visible);
                for (row, (role, line)) in lines[start..end].iter().enumerate() {
                    self.text(2, row + 2, line, if *role == 0 { 1 } else if *role == 2 { 3 } else { 0 }, width.saturating_sub(4));
                }
            }
            for x in 0..width { self.put(x, input_top, '─', 3); }
            let visible_rows = height.saturating_sub(input_top + 2).max(1);
            let first_line = input_lines.len().saturating_sub(visible_rows);
            if input.is_empty() {
                if let Some(placeholder) = self.placeholder.clone() {
                    self.text(2, input_top + 1, &placeholder, 3, width.saturating_sub(4));
                }
            } else {
                for (row, line) in input_lines.iter().skip(first_line).enumerate() {
                    self.text(2, input_top + 1 + row, line, 0, width.saturating_sub(4));
                }
            }
            self.text(1, height.saturating_sub(1), "Ctrl+N newline · Enter send · PgUp/PgDn scroll · Ctrl+C exit", 2, width.saturating_sub(2));
            let cursor_line = self.input[..self.cursor].iter().filter(|c| **c == '\n').count();
            let cursor_col = self.input[..self.cursor].iter().rev().take_while(|c| **c != '\n').count();
            let visible_cursor_line = cursor_line.saturating_sub(first_line);
            cursor_x = 2 + cursor_col.min(width.saturating_sub(3));
            cursor_y = input_top + 1 + visible_cursor_line;
        }
        let mut stdout = io::stdout();
        for y in 0..height {
            let mut x = 0usize;
            while x < width {
                let i = y * width + x;
                if self.current[i] == self.previous[i] {
                    x += 1;
                    continue;
                }
                let style = self.current[i].style;
                queue!(stdout, MoveTo(x as u16, y as u16))?;
                match style {
                    1 => queue!(
                        stdout,
                        SetForegroundColor(Color::Rgb {
                            r: 200,
                            g: 255,
                            b: 0
                        })
                    )?,
                    2 | 3 => queue!(stdout, SetForegroundColor(Color::DarkGrey))?,
                    _ => queue!(stdout, SetForegroundColor(Color::White))?,
                }
                while x < width {
                    let cell = self.current[y * width + x];
                    if cell == self.previous[y * width + x] || cell.style != style {
                        break;
                    }
                    queue!(stdout, Print(cell.ch))?;
                    self.previous[y * width + x] = cell;
                    x += 1;
                }
            }
        }
        queue!(
            stdout,
            ResetColor,
            MoveTo(
                cursor_x.min(width.saturating_sub(1)) as u16,
                cursor_y.min(height.saturating_sub(2)) as u16
            ),
            Show
        )?;
        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_tui() -> Tui {
        Tui {
            width: 40,
            height: 12,
            previous: Vec::new(),
            current: Vec::new(),
            messages: Vec::new(),
            input: Vec::new(),
            cursor: 0,
            scroll: 0,
            status: String::new(),
            placeholder: None,
            home_mode: false,
            mask_input: false,
            resume_prompt: None,
            home_placeholders: Vec::new(),
            placeholder_index: 0,
            last_placeholder_rotation: Instant::now(),
            version: String::new(),
        }
    }

    #[test]
    fn compact_code_preview_keeps_edit_summary_and_hides_diff_lines() {
        let preview = Tui::compact_code_preview("Edited python.html\nimport os\n- improt frint\n+ import print\nDone");
        assert_eq!(preview, "Edited python.html\nDone");
    }

    #[test]
    fn wraps_messages_and_indents_continuation_lines() {
        let mut tui = test_tui();
        tui.add_message(0, "abcdefghijk".into());
        let lines = tui.lines(9);
        assert_eq!(lines[0], (0, "you  abcd".into()));
        assert_eq!(lines[1], (0, "     efgh".into()));
        assert_eq!(lines[2], (0, "     ijk".into()));
    }

    #[test]
    fn markdown_markers_are_cleaned_for_terminal_display() {
        assert_eq!(Tui::clean_markdown("**A heading**"), "A heading");
        assert_eq!(Tui::clean_markdown("- A list item"), "• A list item");
        assert_eq!(Tui::clean_markdown("## Section"), "▌ Section");
    }

    #[test]
    fn cell_comparison_marks_changed_content_and_style() {
        let blank = Cell::default();
        assert_ne!(blank, Cell { ch: 'x', style: 0 });
        assert_ne!(blank, Cell { ch: ' ', style: 1 });
    }
}
