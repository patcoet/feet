use crossterm::{
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{cmp, env, error::Error, fs, io};
use tui::{
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Modifier, Style},
    // text::{Span, Spans},
    widgets::{Block, Borders, Row, Table},
    Terminal,
};
use unicode_segmentation::UnicodeSegmentation;

struct BufState {
    buf: Vec<String>,
    buf_hist: Vec<Vec<String>>,
    c_row: usize, // Cursor row
    c_col: usize,
    scrolled: usize,
    c_hist: Vec<(usize, usize, usize)>,
    c_row_max: usize,
}

impl BufState {
    fn adjust_col(&mut self) {
        self.c_col = cmp::min(self.c_col, self.buf[self.c_row + self.scrolled].len());
    }

    fn push_hists(&mut self) {
        self.buf_hist.push(self.buf.clone());
        self.c_hist.push((self.c_col, self.c_row, self.scrolled));
    }

    fn move_cursor_up(&mut self, amount: usize) {
        if self.c_row >= amount {
            self.c_row -= amount;
            self.adjust_col();
        } else if self.scrolled + self.c_row >= amount {
            self.scrolled -= cmp::min(amount - self.c_row, self.scrolled);
            self.c_row = 0;
            self.adjust_col();
        } else {
            self.scrolled = 0;
            self.c_row = 0;
            self.adjust_col();
        }
    }

    fn move_cursor_down(&mut self, amount: usize) {
        if self.c_row + amount < self.c_row_max {
            // No need to scroll
            self.c_row += cmp::min(amount, self.buf.len() - (self.scrolled + self.c_row));
            self.adjust_col();
        } else {
            let remaining_lines_in_file = self.buf.len() - (self.scrolled + self.c_row) - 1;
            let lines_wanting_to_scroll = self.c_row + amount + 1 - self.c_row_max;
            let lines_to_scroll = cmp::min(remaining_lines_in_file, lines_wanting_to_scroll);
            if self.scrolled + self.c_row_max < self.buf.len() {
                self.scrolled += lines_to_scroll;
                let remaining_lines_in_file = self.buf.len() - (self.scrolled + self.c_row) - 1;
                let lines_to_scroll = cmp::min(remaining_lines_in_file, lines_wanting_to_scroll);
                self.c_row += cmp::min(amount - lines_to_scroll, remaining_lines_in_file);
            } else {
                self.c_row += cmp::min(amount - lines_to_scroll, remaining_lines_in_file);
            }
            self.adjust_col();
        }
    }

    fn move_cursor_left(&mut self, amount: usize) {
        self.c_col = cmp::max(self.c_col - amount, 0);
    }

    fn move_cursor_right(&mut self, amount: usize) {
        self.c_col = cmp::min(
            self.c_col + amount,
            self.buf[self.c_row + self.scrolled].len(),
        );
    }

    fn backspace(&mut self) {
        if self.c_col > 0 {
            self.push_hists();

            let line: Vec<&str> =
                UnicodeSegmentation::graphemes(&self.buf[(self.c_row + self.scrolled)][..], true)
                    .collect();

            let p1 = &line[..(self.c_col - 1)].join("");
            let p2 = &line[self.c_col..].join("");

            self.buf[(self.c_row + self.scrolled)] = p1.to_string() + p2;
            self.c_col -= 1;
        } else if self.c_col == 0
            && self.c_row + self.scrolled > 0
            && self.buf[(self.c_row + self.scrolled)].is_empty()
        {
            self.push_hists();

            self.buf.remove(self.c_row + self.scrolled);
            self.c_row -= 1;
            self.c_col = self.buf[self.c_row + self.scrolled].len();
        } else if self.c_col == 0 && self.c_row + self.scrolled > 0 {
            self.push_hists();

            let p = &self.buf[(self.c_row + self.scrolled)].clone();
            self.c_col =
                UnicodeSegmentation::graphemes(&self.buf[self.c_row + self.scrolled - 1][..], true)
                    .count();
            self.buf[(self.c_row + self.scrolled - 1)].push_str(p);
            self.buf.remove(self.c_row + self.scrolled);
            self.c_row -= 1;
        }
    }

    fn enter(&mut self) {
        if self.c_col == self.buf[self.c_row + self.scrolled].len() {
            self.push_hists();

            self.buf
                .insert(self.c_row + 1 + self.scrolled, "".to_string());
            if self.c_row < self.c_row_max - 1 {
                self.c_row += 1;
            } else {
                self.scrolled += 1;
            }
            self.c_col = 0;
        } else {
            self.push_hists();

            let line: Vec<&str> =
                UnicodeSegmentation::graphemes(&self.buf[self.c_row + self.scrolled][..], true)
                    .collect();
            let p1 = &line[..self.c_col].join("");
            let p2 = &line[self.c_col..].join("");

            self.buf
                .insert(self.c_row + 1 + self.scrolled, p2.to_string());
            self.buf
                .insert(self.c_row + 1 + self.scrolled, p1.to_string());
            self.buf.remove(self.c_row + self.scrolled);
            if self.c_row < self.c_row_max - 1 {
                self.c_row += 1;
            } else {
                self.scrolled += 1;
            }
            self.c_col = 0;
        }
    }

    fn undo(&mut self) {
        match self.buf_hist.pop() {
            Some(x) => {
                self.buf = x;
                (self.c_col, self.c_row, self.scrolled) = self.c_hist.pop().unwrap();
                // Safe because c_hist is added to whenever buf_hist is
            }
            None => (),
        }
    }

    fn insert_char(&mut self, c: char) {
        self.push_hists();

        let line: Vec<&str> =
            UnicodeSegmentation::graphemes(&self.buf[self.c_row + self.scrolled][..], true)
                .collect();
        let p1 = &line[..self.c_col].join("");
        let p2 = &line[self.c_col..].join("");
        let p: String = p1.to_string() + &c.to_string() + p2;
        self.buf[self.c_row + self.scrolled] = p;
        self.c_col += 1;
    }
}

fn run(filename: &str) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut backend = CrosstermBackend::new(io::stdout());
    execute!(backend, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(backend)?;

    let term_width = terminal.size()?.width;

    let mut buf_state = BufState {
        buf: fs::read_to_string(filename)?
            .lines()
            .map(|x| x.to_string())
            .collect(),
        buf_hist: vec![],
        c_row: 0,
        c_col: 0,
        scrolled: 0,
        c_hist: vec![],
        c_row_max: terminal.size()?.height as usize - 2,
    };

    let mut should_write_to_file = false;

    loop {
        terminal.draw(|f| {
            let size = f.size();

            let block = Block::default()
                .title("Freja's Editor for Editing Text")
                .borders(Borders::ALL);
            let block_inner = block.inner(size);
            f.render_widget(block, size);

            let line_numbers_width = (buf_state.buf.len() as f32).log10() as usize + 2;
            let mut lines_with_nums: Vec<Vec<String>> = vec![];
            for (i, item) in buf_state.buf.iter().enumerate().skip(buf_state.scrolled) {
                let overflow_indicator =
                    if item.len() > term_width as usize - 2 - line_numbers_width - 2 - 2 {
                        ">>".to_string()
                    } else {
                        "".to_string()
                    };
                lines_with_nums.push(vec![
                    format!("{:>line_numbers_width$}", i + 1),
                    item.clone(),
                    overflow_indicator,
                ]);
            }
            let rows = lines_with_nums.into_iter().map(Row::new);

            f.set_cursor(
                (2 + line_numbers_width + buf_state.c_col) as u16,
                1 + buf_state.c_row as u16,
            );

            f.render_widget(
                Table::new(rows)
                    .widths(&[
                        Constraint::Length(line_numbers_width as u16),
                        Constraint::Length(term_width - 2 - line_numbers_width as u16 - 2 - 2),
                        Constraint::Min(2),
                    ])
                    .column_spacing(1)
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD)),
                block_inner,
            );
        })?;

        if let Event::Key(KeyEvent {
            code,
            modifiers: _m,
        }) = read()?
        {
            match code {
                KeyCode::Esc => break,
                KeyCode::Up => {
                    buf_state.move_cursor_up(1);
                }
                KeyCode::Down => {
                    buf_state.move_cursor_down(1);
                }
                KeyCode::Left => {
                    buf_state.move_cursor_left(1);
                }
                KeyCode::Right => {
                    buf_state.move_cursor_right(1);
                }
                KeyCode::PageUp => {
                    buf_state.move_cursor_up(10);
                }
                KeyCode::PageDown => {
                    buf_state.move_cursor_down(10);
                }
                KeyCode::Backspace => {
                    buf_state.backspace();
                }
                KeyCode::Enter => {
                    buf_state.enter();
                }
                KeyCode::Char(c) => {
                    if _m == KeyModifiers::CONTROL && c == 's' {
                        should_write_to_file = true;
                        break;
                    } else if _m == KeyModifiers::CONTROL && c == 'z' {
                        buf_state.undo();
                    } else {
                        buf_state.insert_char(c);
                    }
                }
                _ => continue,
            }
        }
    }

    if should_write_to_file {
        fs::write(filename, buf_state.buf.join("\n"))?;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

fn main() {
    let filename = &env::args().collect::<Vec<String>>()[1];

    run(filename).expect("Something went wrong!");
}
