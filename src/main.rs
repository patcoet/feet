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

struct UndoBufferLine(usize, Option<String>);
struct UndoBufferCurPos(usize, usize, usize);
struct UndoBufferEntry(
    UndoBufferLine,
    UndoBufferLine,
    UndoBufferLine,
    UndoBufferLine,
    UndoBufferCurPos,
    UndoBufferCurPos,
);

struct BufState {
    buf: Vec<String>,
    c_row: usize, // Cursor row
    c_col: usize,
    scrolled: usize,
    c_row_max: usize,
    un_buf: Vec<UndoBufferEntry>,
    un_buf_i: usize,
}

impl BufState {
    fn adjust_col(&mut self) {
        self.c_col = cmp::min(self.c_col, self.buf[self.c_row + self.scrolled].len());
    }

    fn add_to_undo_buffer(
        &mut self,
        l1o: UndoBufferLine,
        l1n: UndoBufferLine,
        l2o: UndoBufferLine,
        l2n: UndoBufferLine,
        cpo: UndoBufferCurPos,
        cpn: UndoBufferCurPos,
    ) {
        self.un_buf.truncate(self.un_buf_i);
        self.un_buf
            .push(UndoBufferEntry(l1o, l1n, l2o, l2n, cpo, cpn));
        self.un_buf_i += 1;
    }

    fn undo(&mut self) {
        if self.un_buf_i < 1 {
            return;
        }

        self.un_buf_i -= 1;
        let UndoBufferEntry(l1o, _, l2o, l2n, cpo, _) = &self.un_buf[self.un_buf_i];

        self.buf[l1o.0] = l1o.1.as_ref().unwrap().to_string();
        self.c_row = cpo.0;
        self.c_col = cpo.1;
        self.scrolled = cpo.2;

        if l1o.0 != l2o.0 {
            match (&l2o.1, &l2n.1) {
                // We only get here if we're adding or removing a line, so we only have Some, None
                // and None, Some to deal with
                (Some(x), None) => self.buf.insert(l2o.0, x.to_string()),
                (None, Some(_)) => {
                    self.buf.remove(l2o.0);
                }
                _ => (),
            }
        }
    }

    fn redo(&mut self) {
        if self.un_buf_i >= self.un_buf.len() {
            return;
        }

        let UndoBufferEntry(l1o, l1n, l2o, l2n, cpo, cpn) = &self.un_buf[self.un_buf_i];

        self.buf[l1n.0] = l1n.1.as_ref().unwrap().to_string();
        self.c_row = cpn.0;
        self.c_col = cpn.1;
        self.scrolled = cpn.2;

        if l1n.0 != l2n.0 {
            match (&l2o.1, &l2n.1) {
                (Some(x), None) => {
                    self.buf.remove(l2n.0);
                }
                (None, Some(x)) => (self.buf.insert(l2n.0, x.to_string())),
                _ => (),
            }
        }

        self.un_buf_i += 1;
    }

    fn move_cursor_up(&mut self, amount: usize) {
        if self.c_row >= amount {
            self.c_row -= amount;
        } else if self.scrolled + self.c_row >= amount {
            self.scrolled -= cmp::min(amount - self.c_row, self.scrolled);
            self.c_row = 0;
        } else {
            self.scrolled = 0;
            self.c_row = 0;
        }
        self.adjust_col();
    }

    fn move_cursor_down(&mut self, amount: usize) {
        let curr_line = self.scrolled + self.c_row + 1;
        let target_line = curr_line + amount;
        let last_visible_line = self.scrolled + self.c_row_max;
        let remaining_lines_in_file = self.buf.len() - curr_line;
        if target_line <= last_visible_line {
            // No need to scroll
            self.c_row += amount;
        } else {
            // Scroll and move cursor down if needed
            // Cases:
            // 1. Just scroll amount lines
            // 2. Just scroll less than amount lines, because EOF
            // 3. Scroll amount lines and move cursor, because cursor not at bottom when paging
            //    down
            // 4. Scroll less than amount lines and move cursor
            if curr_line < last_visible_line {
                // Cursor needs to be moved
                let c_row_change = last_visible_line - curr_line;
                let lines_to_scroll = cmp::min(remaining_lines_in_file, amount - c_row_change);
                self.c_row += c_row_change;
                if last_visible_line < self.buf.len() {
                    self.scrolled += lines_to_scroll;
                }
            } else {
                // Just scroll
                let lines_to_scroll = cmp::min(remaining_lines_in_file, amount);
                self.scrolled += lines_to_scroll;
            }
        }
        self.adjust_col();
    }

    fn move_cursor_left(&mut self, amount: usize) {
        if amount <= self.c_col {
            self.c_col -= amount;
        } else {
            self.c_col = 0;
        }
    }

    fn move_cursor_right(&mut self, amount: usize) {
        self.c_col = cmp::min(
            self.c_col + amount,
            self.buf[self.c_row + self.scrolled].len(),
        );
    }

    fn backspace(&mut self) {
        let cpo = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);

        if self.c_col > 0 {
            let l1o = UndoBufferLine(
                self.scrolled + self.c_row,
                Some(self.buf[self.scrolled + self.c_row].clone()),
            );
            let l2o = UndoBufferLine(self.scrolled + self.c_row, None);

            let line: Vec<&str> =
                UnicodeSegmentation::graphemes(&self.buf[(self.c_row + self.scrolled)][..], true)
                    .collect();

            let p1 = &line[..(self.c_col - 1)].join("");
            let p2 = &line[self.c_col..].join("");

            self.buf[(self.c_row + self.scrolled)] = p1.to_string() + p2;
            self.c_col -= 1;

            let l1n = UndoBufferLine(
                self.scrolled + self.c_row,
                Some(self.buf[self.scrolled + self.c_row].clone()),
            );
            let l2n = UndoBufferLine(self.scrolled + self.c_row, None);
            let cpn = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);
            self.add_to_undo_buffer(l1o, l1n, l2o, l2n, cpo, cpn);
        } else if self.c_col == 0
            && self.c_row + self.scrolled > 0
            && self.buf[(self.c_row + self.scrolled)].is_empty()
        {
            let l1o = UndoBufferLine(
                self.scrolled + self.c_row - 1,
                Some(self.buf[self.scrolled + self.c_row - 1].clone()),
            );
            let l2o = UndoBufferLine(self.scrolled + self.c_row, Some("".to_string()));

            self.buf.remove(self.c_row + self.scrolled);
            self.c_row -= 1;
            self.c_col = self.buf[self.c_row + self.scrolled].len();

            let l1n = UndoBufferLine(
                self.scrolled + self.c_row,
                Some(self.buf[self.scrolled + self.c_row].clone()),
            );
            let l2n = UndoBufferLine(self.scrolled + self.c_row + 1, None);
            let cpn = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);
            self.add_to_undo_buffer(l1o, l1n, l2o, l2n, cpo, cpn);
        } else if self.c_col == 0 && self.c_row + self.scrolled > 0 {
            let l1o = UndoBufferLine(
                self.scrolled + self.c_row - 1,
                Some(self.buf[self.scrolled + self.c_row - 1].clone()),
            );
            let l2o = UndoBufferLine(
                self.scrolled + self.c_row,
                Some(self.buf[self.scrolled + self.c_row].clone()),
            );

            let p = &self.buf[(self.c_row + self.scrolled)].clone();
            self.c_col =
                UnicodeSegmentation::graphemes(&self.buf[self.c_row + self.scrolled - 1][..], true)
                    .count();
            self.buf[(self.c_row + self.scrolled - 1)].push_str(p);
            self.buf.remove(self.c_row + self.scrolled);
            self.c_row -= 1;

            let l1n = UndoBufferLine(
                self.scrolled + self.c_row,
                Some(self.buf[self.scrolled + self.c_row].clone()),
            );
            let l2n = UndoBufferLine(self.scrolled + self.c_row + 1, None);
            let cpn = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);
            self.add_to_undo_buffer(l1o, l1n, l2o, l2n, cpo, cpn);
        }
    }

    fn enter(&mut self) {
        let l1o = UndoBufferLine(
            self.scrolled + self.c_row,
            Some(self.buf[self.scrolled + self.c_row].clone()),
        );
        let l2o = UndoBufferLine(self.scrolled + self.c_row + 1, None);
        let cpo = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);

        if self.c_col == self.buf[self.c_row + self.scrolled].len() {
            self.buf
                .insert(self.c_row + 1 + self.scrolled, "".to_string());
            if self.c_row < self.c_row_max - 1 {
                self.c_row += 1;
            } else {
                self.scrolled += 1;
            }
            self.c_col = 0;
        } else {
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

        let l1n = UndoBufferLine(
            self.scrolled + self.c_row - 1,
            Some(self.buf[self.scrolled + self.c_row - 1].clone()),
        );
        let l2n = UndoBufferLine(
            self.scrolled + self.c_row + 0,
            Some(self.buf[self.scrolled + self.c_row + 0].clone()),
        );
        let cpn = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);
        self.add_to_undo_buffer(l1o, l1n, l2o, l2n, cpo, cpn);
    }

    fn insert_char(&mut self, c: char) {
        let l1o = UndoBufferLine(
            self.scrolled + self.c_row,
            Some(self.buf[self.scrolled + self.c_row].clone()),
        );
        let l2o = UndoBufferLine(self.scrolled + self.c_row, None);
        let cpo = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);

        let line: Vec<&str> =
            UnicodeSegmentation::graphemes(&self.buf[self.c_row + self.scrolled][..], true)
                .collect();
        let p1 = &line[..self.c_col].join("");
        let p2 = &line[self.c_col..].join("");
        let p: String = p1.to_string() + &c.to_string() + p2;
        self.buf[self.c_row + self.scrolled] = p;
        self.c_col += 1;

        let l1n = UndoBufferLine(
            self.scrolled + self.c_row,
            Some(self.buf[self.c_row].clone()),
        );
        let l2n = UndoBufferLine(self.scrolled + self.c_row, None);
        let cpn = UndoBufferCurPos(self.c_row, self.c_col, self.scrolled);

        self.add_to_undo_buffer(l1o, l1n, l2o, l2n, cpo, cpn);
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
        c_row: 0,
        c_col: 0,
        scrolled: 0,
        c_row_max: terminal.size()?.height as usize - 2,
        un_buf: vec![],
        un_buf_i: 0,
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

        if let Event::Key(KeyEvent { code, modifiers: m }) = read()? {
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
                    if m == KeyModifiers::CONTROL && c == 's' {
                        should_write_to_file = true;
                        break;
                    } else if m == KeyModifiers::CONTROL && c == 'z' {
                        buf_state.undo();
                    } else if m == KeyModifiers::CONTROL && c == 'y' {
                        buf_state.redo();
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
