use clap::Parser;
use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    time::{Duration, Instant},
};

/// A simple hex viewer for binary files with a TUI.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The path to the file to view.
    #[arg(name = "FILE")]
    path: PathBuf,

    /// Starting offset (decimal or hexadecimal, e.g., 0x1A or 26).
    /// If not provided, starts from the beginning of the file.
    #[arg(short, long, default_value_t = String::from("0"))]
    start: String,

    /// Number of bytes to display per line in the output.
    #[arg(short = 'w', long, default_value_t = 16)]
    bytes_per_line: usize,
}

/// Parses a string into a `u64`, supporting both decimal and hexadecimal (prefixed with "0x" or "0X").
fn parse_offset_or_length(s: &str) -> Result<u64, Box<dyn Error>> {
    if s.starts_with("0x") || s.starts_with("0X") {
        Ok(u64::from_str_radix(&s[2..], 16)?)
    } else {
        Ok(s.parse()?)
    }
}

/// Represents the current mode of the application.
enum AppMode {
    Normal,
    Command,
}

/// Main application state.
struct App {
    file_path: PathBuf,
    file: File,
    file_size: u64,
    current_offset: u64,
    bytes_per_line: usize,
    buffer: Vec<u8>,
    last_bytes_read: usize,
    should_quit: bool,
    mode: AppMode,
    input_buffer: String,
    status_message: String,
    last_status_time: Instant,
    default_page_size: u64, // Default bytes to read for a "page"
}

impl App {
    fn new(
        file_path: PathBuf,
        bytes_per_line: usize,
        initial_offset: u64,
    ) -> Result<App, Box<dyn Error>> {
        let file = File::open(&file_path)
            .map_err(|e| format!("Failed to open file '{}': {}", file_path.display(), e))?;
        let file_size = file
            .metadata()
            .map_err(|e| {
                format!(
                    "Failed to get metadata for '{}': {}",
                    file_path.display(),
                    e
                )
            })?
            .len();

        let default_page_size = (bytes_per_line * 16) as u64; // Display 16 lines by default

        let mut app = App {
            file_path,
            file,
            file_size,
            current_offset: initial_offset.min(file_size.saturating_sub(1)),
            bytes_per_line,
            buffer: Vec::new(),
            last_bytes_read: 0,
            should_quit: false,
            mode: AppMode::Normal,
            input_buffer: String::new(),
            status_message: String::new(),
            last_status_time: Instant::now(),
            default_page_size,
        };
        app.set_status_message(format!(
            "File: '{}' (Size: {} bytes)",
            app.file_path.display(),
            app.file_size
        ));
        Ok(app)
    }

    /// Reads a chunk of data into the buffer based on current_offset and available height.
    fn read_current_chunk(&mut self, height: u16) -> io::Result<()> {
        let lines_to_read = height.saturating_sub(4); // Account for header, footer, etc.
        let bytes_to_read = self.bytes_per_line * lines_to_read as usize;
        self.default_page_size = bytes_to_read as u64; // Update default page size based on screen height

        // Ensure we don't read beyond file size
        let actual_bytes_to_read =
            (self.file_size - self.current_offset).min(bytes_to_read as u64) as usize;

        if actual_bytes_to_read == 0 {
            self.buffer.clear();
            self.last_bytes_read = 0;
            self.set_status_message(format!(
                "End of file reached at 0x{:X}",
                self.current_offset
            ));
            return Ok(());
        }

        self.buffer.resize(actual_bytes_to_read, 0);
        self.file.seek(SeekFrom::Start(self.current_offset))?;
        self.last_bytes_read = self.file.read(&mut self.buffer)?;

        if self.last_bytes_read < actual_bytes_to_read {
            // Trim buffer if less bytes were read than requested (e.g., at EOF)
            self.buffer.truncate(self.last_bytes_read);
        }
        Ok(())
    }

    fn set_status_message(&mut self, message: String) {
        self.status_message = message;
        self.last_status_time = Instant::now();
    }

    fn handle_key_event(&mut self, key_event: event::KeyEvent) {
        match self.mode {
            AppMode::Normal => match key_event.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
                KeyCode::Char(':') => {
                    self.mode = AppMode::Command;
                    self.input_buffer.clear();
                    self.set_status_message("Command mode: enter offset (e.g., '0x100') or page command (e.g., 'page +10')".to_string());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.current_offset = self
                        .current_offset
                        .saturating_add(self.bytes_per_line as u64);
                    self.set_status_message(format!("Moved to 0x{:X}", self.current_offset));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.current_offset = self
                        .current_offset
                        .saturating_sub(self.bytes_per_line as u64);
                    self.set_status_message(format!("Moved to 0x{:X}", self.current_offset));
                }
                KeyCode::PageDown | KeyCode::Char(' ') => {
                    self.current_offset =
                        self.current_offset.saturating_add(self.default_page_size);
                    self.set_status_message(format!("Moved to 0x{:X}", self.current_offset));
                }
                KeyCode::PageUp => {
                    self.current_offset =
                        self.current_offset.saturating_sub(self.default_page_size);
                    self.set_status_message(format!("Moved to 0x{:X}", self.current_offset));
                }
                KeyCode::Home => {
                    self.current_offset = 0;
                    self.set_status_message("Moved to start of file".to_string());
                }
                KeyCode::End => {
                    self.current_offset = self.file_size.saturating_sub(self.default_page_size);
                    self.set_status_message("Moved to end of file".to_string());
                }
                _ => {}
            },
            AppMode::Command => match key_event.code {
                KeyCode::Enter => {
                    self.execute_command();
                    self.mode = AppMode::Normal;
                    self.input_buffer.clear();
                }
                KeyCode::Esc => {
                    self.mode = AppMode::Normal;
                    self.input_buffer.clear();
                    self.set_status_message("Normal mode".to_string());
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                _ => {}
            },
        }
        // Ensure offset stays within bounds
        self.current_offset = self.current_offset.min(self.file_size.saturating_sub(1));
    }

    fn execute_command(&mut self) {
        let input_str = self.input_buffer.trim();
        let parts: Vec<&str> = input_str.split_whitespace().collect();

        if parts.is_empty() {
            self.set_status_message("No command entered.".to_string());
            return;
        }

        let cmd = parts[0].to_ascii_lowercase();
        if cmd == "q" {
            self.should_quit = true;
            return;
        }

        if cmd == "page" {
            if parts.len() == 2 {
                let page_move_str = parts[1];
                if let Some(val) = page_move_str.strip_prefix('+') {
                    match val.parse::<u64>() {
                        Ok(pages) => {
                            self.current_offset = self
                                .current_offset
                                .saturating_add(pages * self.default_page_size);
                            self.set_status_message(format!(
                                "Moved +{} pages to 0x{:X}",
                                pages, self.current_offset
                            ));
                        }
                        Err(_) => self.set_status_message(
                            "Error: Invalid page count for '+' operation.".to_string(),
                        ),
                    }
                } else if let Some(val) = page_move_str.strip_prefix('-') {
                    match val.parse::<u64>() {
                        Ok(pages) => {
                            self.current_offset = self
                                .current_offset
                                .saturating_sub(pages * self.default_page_size);
                            self.set_status_message(format!(
                                "Moved -{} pages to 0x{:X}",
                                pages, self.current_offset
                            ));
                        }
                        Err(_) => self.set_status_message(
                            "Error: Invalid page count for '-' operation.".to_string(),
                        ),
                    }
                } else {
                    self.set_status_message(
                        "Error: Invalid page command. Use 'page +N' or 'page -N'.".to_string(),
                    );
                }
            } else if parts.len() == 1 {
                self.current_offset = self.current_offset.saturating_add(self.default_page_size);
                self.set_status_message(format!(
                    "Moved to next page at 0x{:X}",
                    self.current_offset
                ));
            } else {
                self.set_status_message("Error: Invalid 'page' command format.".to_string());
            }
        } else {
            // Assume it's an offset
            match parse_offset_or_length(parts[0]) {
                Ok(parsed_offset) => {
                    self.current_offset = parsed_offset;
                    self.set_status_message(format!(
                        "Jumped to offset 0x{:X}",
                        self.current_offset
                    ));
                }
                Err(e) => self.set_status_message(format!("Error parsing offset: {}", e)),
            }
        }
        // Ensure offset stays within bounds after command execution
        self.current_offset = self.current_offset.min(self.file_size.saturating_sub(1));
    }
}

// Remove generic parameter from `ui` function signature
fn ui(frame: &mut Frame, app: &mut App) {
    let size = frame.size();

    // Divide the screen into areas: Header, Hex View, Status/Command Line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header: File info
            Constraint::Min(0),    // Main content: Hex + ASCII
            Constraint::Length(3), // Footer: Status / Command input
        ])
        .split(size);

    // Header Block
    let header_block = Block::default().borders(Borders::ALL).title(format!(
        "{} - Hex Viewer - {} (Size: {} bytes)",
        env!("CARGO_PKG_NAME"),
        app.file_path.display(),
        app.file_size
    ));
    frame.render_widget(header_block, chunks[0]);

    // Read current chunk into buffer based on available height
    if let Err(e) = app.read_current_chunk(chunks[1].height) {
        app.set_status_message(format!("File read error: {}", e));
    }

    // Main content: Hex + ASCII display
    let mut hex_lines: Vec<Line> = Vec::new();
    let mut ascii_lines: Vec<Line> = Vec::new();

    for i in (0..app.last_bytes_read).step_by(app.bytes_per_line) {
        let line_offset = app.current_offset + i as u64;
        let line_end = (i + app.bytes_per_line).min(app.last_bytes_read);
        let chunk = &app.buffer[i..line_end];

        let mut hex_spans: Vec<Span> = vec![Span::raw(format!("{:08X}: ", line_offset))];
        let mut ascii_spans: Vec<Span> = Vec::new();

        // Hex part
        (0..app.bytes_per_line).for_each(|j| {
            if i + j < app.last_bytes_read {
                hex_spans.push(Span::styled(format!("{:02X} ", chunk[j]), Style::default()));
            } else {
                hex_spans.push(Span::raw("   ")); // Padding
            }
        });

        // ASCII part
        for b in chunk {
            if *b >= 32 && *b <= 126 {
                // Fixed duplicate condition
                ascii_spans.push(Span::raw(format!("{}", *b as char)));
            } else {
                ascii_spans.push(Span::raw("."));
            }
        }

        hex_lines.push(Line::from(hex_spans));
        ascii_lines.push(Line::from(ascii_spans));
    }

    // Combine hex and ASCII for display
    let mut full_content: Vec<Line> = Vec::new();
    for i in 0..hex_lines.len() {
        let mut combined_spans = hex_lines[i].spans.clone();
        combined_spans.push(Span::raw(" | ")); // Separator
        combined_spans.extend(ascii_lines[i].spans.clone());
        full_content.push(Line::from(combined_spans));
    }

    let paragraph = Paragraph::new(full_content).block(Block::default().borders(Borders::ALL));
    frame.render_widget(paragraph, chunks[1]);

    // Footer Block (Status / Command Line)
    let footer_block = Block::default().borders(Borders::ALL);
    let footer_content: Text = match app.mode {
        AppMode::Normal => {
            let msg = if app.last_status_time.elapsed() < Duration::from_secs(5)
                && !app.status_message.is_empty()
            {
                app.status_message.clone()
            } else {
                format!(
                    "Offset: 0x{:X} | Press ':' for command, 'q' to quit, '?' for help",
                    app.current_offset
                )
            };
            Text::from(Line::from(vec![Span::styled(
                msg,
                Style::default().add_modifier(Modifier::BOLD),
            )]))
        }
        AppMode::Command => {
            Text::from(Line::from(vec![
                Span::styled(":", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&app.input_buffer),
                Span::styled("_", Style::default().add_modifier(Modifier::REVERSED)), // Cursor
            ]))
        }
    };
    let footer_paragraph = Paragraph::new(footer_content).block(footer_block);
    frame.render_widget(footer_paragraph, chunks[2]);

    // Position cursor for command mode
    if let AppMode::Command = app.mode {
        frame.set_cursor(
            chunks[2].x + 1 + app.input_buffer.len() as u16,
            chunks[2].y + 1,
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let initial_offset = parse_offset_or_length(&cli.start)?;
    let mut app = App::new(cli.path, cli.bytes_per_line, initial_offset)?;

    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui(frame, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key_event(key);
                }
            } else if let CrosstermEvent::Resize(_, _) = event::read()? {
                // Reread chunk on resize to fit new height
                app.read_current_chunk(terminal.size()?.height)?;
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
