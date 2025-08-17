use std::env;
use std::fs::File;
use std::io::{self};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{execute, terminal};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use ratatui::Frame as TuiFrame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

#[derive(Default)]
struct App {
    file_path: PathBuf,
    data: Vec<Vec<String>>, // rows x cols
    row: usize,
    col: usize,
    editing: bool,
    editor_buf: String,
    dirty: bool,
}

impl App {
    fn max_cols(&self) -> usize {
        self.data.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    fn ensure_cell_exists(&mut self, r: usize, c: usize) {
        if r >= self.data.len() {
            self.data.resize(r + 1, Vec::new());
        }
        if c >= self.data[r].len() {
            self.data[r].resize(c + 1, String::new());
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("enable raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen).context("enter alt screen")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn usage(program: &str) {
    eprintln!("Usage: {program} <path/to/file.csv>");
}

fn load_csv(path: &PathBuf) -> Result<Vec<Vec<String>>> {
    let file = File::open(path).with_context(|| format!("open {path:?}"))?;
    let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(file);
    let mut out = Vec::new();
    for rec in rdr.records() {
        let rec: StringRecord = rec?;
        out.push(rec.iter().map(|s| s.to_string()).collect());
    }
    Ok(out)
}

fn save_csv(path: &PathBuf, data: &[Vec<String>]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {path:?}"))?;
    let mut wtr = WriterBuilder::new().has_headers(false).from_writer(file);
    for row in data {
        wtr.write_record(row)?;
    }
    wtr.flush()?;
    Ok(())
}

fn draw_ui<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &App) -> Result<()> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // table
                Constraint::Length(3), // status/help
                Constraint::Length(3), // editor / message line
            ])
            .split(f.area());

        draw_table(f, chunks[0], app);
        draw_status(f, chunks[1], app);
        draw_editor(f, chunks[2], app);
    })?;
    Ok(())
}

fn draw_table(f: &mut TuiFrame, area: Rect, app: &App) {
    let rows_len = app.data.len();
    let cols_len = app.max_cols();
    let cols = cols_len.max(1);

    let mut rows = Vec::with_capacity(rows_len.max(1));
    for (r_idx, row) in app.data.iter().enumerate() {
        let mut cells = Vec::with_capacity(cols);
        for c_idx in 0..cols {
            let txt = row.get(c_idx).map(String::as_str).unwrap_or("");
            let mut cell = Cell::from(txt.to_string());
            if r_idx == app.row && c_idx == app.col {
                cell = cell.style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
            cells.push(cell);
        }
        rows.push(Row::new(cells));
    }

    // Construct basic constraints: at least 5 chars per column.
    let constraints: Vec<Constraint> = (0..cols).map(|_| Constraint::Min(5)).collect();

    let table = Table::new(rows, constraints)
        .block(Block::default().title("CSV Viewer").borders(Borders::ALL))
        .column_spacing(1);
    f.render_widget(table, area);
}

fn draw_status(f: &mut TuiFrame, area: Rect, app: &App) {
    let status = format!(
        "File: {} | Pos: (row {}, col {}) | Dirty: {}",
        app.file_path.display(),
        app.row + 1,
        app.col + 1,
        if app.dirty { "yes" } else { "no" }
    );
    let help = "Arrows: move  e: edit  Enter: save cell  Esc: cancel  w: write  q: quit";
    let text = vec![Line::raw(status), Line::raw(help)];
    let p = Paragraph::new(text)
        .block(Block::default().title("Status").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_editor(f: &mut TuiFrame, area: Rect, app: &App) {
    let (title, content) = if app.editing {
        (
            "Editor",
            format!(
                "Editing (r{}, c{}): {}",
                app.row + 1,
                app.col + 1,
                app.editor_buf
            ),
        )
    } else {
        ("Info", "Press 'e' to edit selected cell".to_string())
    };
    let p = Paragraph::new(Line::from(Span::raw(content)))
        .block(Block::default().title(title).borders(Borders::ALL));
    f.render_widget(p, area);
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Returns Ok(true) to request exit
    if app.editing {
        match key.code {
            KeyCode::Enter => {
                app.ensure_cell_exists(app.row, app.col);
                app.data[app.row][app.col] = app.editor_buf.clone();
                app.editor_buf.clear();
                app.editing = false;
                app.dirty = true;
            }
            KeyCode::Esc => {
                app.editor_buf.clear();
                app.editing = false;
            }
            KeyCode::Backspace => {
                app.editor_buf.pop();
            }
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    app.editor_buf.push(c);
                }
            }
            KeyCode::Left => {}
            KeyCode::Right => {}
            KeyCode::Up => {}
            KeyCode::Down => {}
            _ => {}
        }
        return Ok(false);
    }

    match key.code {
        KeyCode::Char('q') => {
            // Auto-save on quit if dirty
            if app.dirty {
                save_csv(&app.file_path, &app.data)?;
            }
            return Ok(true);
        }
        KeyCode::Char('w') => {
            save_csv(&app.file_path, &app.data)?;
            app.dirty = false;
        }
        KeyCode::Char('e') => {
            app.ensure_cell_exists(app.row, app.col);
            app.editor_buf = app.data[app.row][app.col].clone();
            app.editing = true;
        }
        KeyCode::Left => {
            if app.col > 0 {
                app.col -= 1;
            }
        }
        KeyCode::Right => {
            let cols = app.max_cols();
            if app.col + 1 < cols {
                app.col += 1;
            }
        }
        KeyCode::Up => {
            if app.row > 0 {
                app.row -= 1;
                app.col = app.col.min(app.data[app.row].len().saturating_sub(1));
            }
        }
        KeyCode::Down => {
            if app.row + 1 < app.data.len() {
                app.row += 1;
                app.col = app.col.min(app.data[app.row].len().saturating_sub(1));
            }
        }
        _ => {}
    }
    Ok(false)
}

fn main() -> Result<()> {
    let mut args = env::args().collect::<Vec<_>>();
    let program = args.remove(0);
    if args.is_empty() {
        usage(&program);
        return Err(anyhow!("missing CSV file path"));
    }
    let file_path = PathBuf::from(&args[0]);
    let data = load_csv(&file_path).with_context(|| "failed to load CSV")?;

    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;

    let mut app = App {
        file_path,
        data,
        row: 0,
        col: 0,
        editing: false,
        editor_buf: String::new(),
        dirty: false,
    };

    loop {
        draw_ui(&mut terminal, &app)?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                let exit = handle_key(&mut app, key)?;
                if exit {
                    break;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_load_and_save_csv_roundtrip() -> Result<()> {
        let dir = env::temp_dir();
        let path = dir.join(format!("tui_csv_viewer_test_{}.csv", std::process::id()));
        fs::write(&path, b"a,b\nc,d\n")?;

        let data = load_csv(&path)?;
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], vec!["a".to_string(), "b".to_string()]);
        assert_eq!(data[1], vec!["c".to_string(), "d".to_string()]);

        let mut new_data = data.clone();
        new_data[1][1] = "dd".into();
        save_csv(&path, &new_data)?;

        let reread = fs::read_to_string(&path)?;
        assert!(reread.trim_end().ends_with("c,dd"));
        let round = load_csv(&path)?;
        assert_eq!(round[1][1], "dd");
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn test_edit_flow_and_write_key() -> Result<()> {
        let dir = env::temp_dir();
        let path = dir.join(format!("tui_csv_viewer_flow_{}.csv", std::process::id()));
        std::fs::write(&path, b"a,b\nc,d\n")?;

        let data = load_csv(&path)?;
        let mut app = App {
            file_path: path.clone(),
            data,
            row: 0,
            col: 0,
            editing: false,
            editor_buf: String::new(),
            dirty: false,
        };

        handle_key(&mut app, key(KeyCode::Char('e')))?;
        assert!(app.editing);
        assert_eq!(app.editor_buf, "a");

        handle_key(&mut app, key(KeyCode::Char('X')))?;
        handle_key(&mut app, key(KeyCode::Enter))?;
        assert!(!app.editing);
        assert_eq!(app.data[0][0], "aX");
        assert!(app.dirty);

        handle_key(&mut app, key(KeyCode::Char('w')))?;
        assert!(!app.dirty);
        let reread = std::fs::read_to_string(&app.file_path)?;
        assert!(reread.contains("aX,b"));
        let _ = std::fs::remove_file(&app.file_path);
        Ok(())
    }
}
