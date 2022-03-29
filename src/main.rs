use crossterm::{
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::cmp;
use std::env;
use std::fs;
use std::io;
use tui::{
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Modifier, Style},
    // text::{Span, Spans},
    widgets::{Block, Borders, Row, Table},
    Terminal,
};
use unicode_segmentation::UnicodeSegmentation;

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut backend = CrosstermBackend::new(io::stdout());
    execute!(backend, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(backend)?;

    let args: Vec<String> = env::args().collect();
    let buffer = fs::read_to_string(&args[1])?;
    let mut buf: Vec<String> = buffer.lines().map(|x| x.to_string()).collect();

    let mut cursor_row: usize = 0;
    let mut cursor_col: usize = 0;

    let mut v_scroll = 0;
    let v_scroll_max = terminal.size()?.height as usize - 2; // Should be in loop in case of resizing
    let width = terminal.size()?.width;

    loop {
        terminal.draw(|f| {
            let size = f.size();

            let block = Block::default()
                .title("Freja's Editor for Editing Text")
                .borders(Borders::ALL);
            let block_inner = block.inner(size);
            f.render_widget(block, size);

            let line_numbers_width = (buffer.lines().count() as f32).log10() as usize + 2;
            let mut lines_with_nums: Vec<Vec<String>> = vec![];
            for i in v_scroll..buf.len() {
                let overflow_indicator =
                    if buf[i].len() > width as usize - 2 - line_numbers_width - 2 - 2 {
                        ">>".to_string()
                    } else {
                        "".to_string()
                    };
                lines_with_nums.push(vec![
                    format!("{:>line_numbers_width$}", i + 1),
                    buf[i].clone(),
                    overflow_indicator,
                ]);
            }
            let rows = lines_with_nums.into_iter().map(|x| Row::new(x));

            f.set_cursor(
                (2 + line_numbers_width + cursor_col) as u16,
                1 + cursor_row as u16,
            );

            f.render_widget(
                Table::new(rows)
                    .widths(&[
                        Constraint::Length(line_numbers_width as u16),
                        Constraint::Length(width - 2 - line_numbers_width as u16 - 2 - 2),
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
                    if cursor_row > 0 {
                        cursor_row -= 1;
                        cursor_col = cmp::min(cursor_col, buf[cursor_row + v_scroll].len());
                    } else if v_scroll > 0 {
                        cursor_col = cmp::min(cursor_col, buf[cursor_row + v_scroll].len());
                        v_scroll -= 1;
                    }
                }
                KeyCode::Down => {
                    if cursor_row < v_scroll_max - 1 && cursor_row < buf.len() - 1 {
                        cursor_row += 1;
                        cursor_col = cmp::min(cursor_col, buf[cursor_row].len());
                    } else if v_scroll <= buf.len() - 2
                        && cursor_row == v_scroll_max - 1
                        && cursor_row < buf.len() - 1
                    {
                        cursor_col = cmp::min(cursor_col, buf[cursor_row].len());
                        v_scroll += 1;
                    }
                }
                KeyCode::Left => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    }
                }
                KeyCode::Right => {
                    if cursor_col < buf[cursor_row + v_scroll].len() {
                        cursor_col += 1;
                    }
                }
                KeyCode::Backspace => {
                    if cursor_col > 0 {
                        let line: Vec<&str> =
                            UnicodeSegmentation::graphemes(&buf[cursor_row + v_scroll][..], true)
                                .collect();
                        let p1 = &line[..cursor_col - 1].join("");
                        let p2 = &line[cursor_col..].join("");
                        buf[cursor_row + v_scroll] = p1.to_string() + p2;

                        cursor_col -= 1;
                    } else if cursor_col == 0
                        && cursor_row + v_scroll > 0
                        && buf[cursor_row + v_scroll].len() == 0
                    {
                        buf.remove(cursor_row + v_scroll);
                        cursor_row -= 1;
                        cursor_col = buf[cursor_row + v_scroll].len();
                    } else if cursor_col == 0 && cursor_row + v_scroll > 0 {
                        let p = &buf[cursor_row + v_scroll].clone();
                        cursor_col = UnicodeSegmentation::graphemes(
                            &buf[cursor_row + v_scroll - 1][..],
                            true,
                        )
                        .collect::<Vec<&str>>()
                        .len();
                        buf[cursor_row + v_scroll - 1].push_str(p);
                        buf.remove(cursor_row + v_scroll);
                        cursor_row -= 1;
                    }
                }
                KeyCode::Enter => {
                    if cursor_col == buf[cursor_row + v_scroll].len() {
                        buf.insert(cursor_row + 1 + v_scroll, "".to_string());
                        if cursor_row < v_scroll_max - 1 {
                            cursor_row += 1;
                        } else {
                            v_scroll += 1;
                        }
                        cursor_col = 0;
                    } else {
                        let line: Vec<&str> =
                            UnicodeSegmentation::graphemes(&buf[cursor_row + v_scroll][..], true)
                                .collect();
                        let p1 = &line[..cursor_col].join("");
                        let p2 = &line[cursor_col..].join("");

                        buf.insert(cursor_row + 1 + v_scroll, p2.to_string());
                        buf.insert(cursor_row + 1 + v_scroll, p1.to_string());
                        buf.remove(cursor_row + v_scroll);
                        if cursor_row < v_scroll_max - 1 {
                            cursor_row += 1;
                        } else {
                            v_scroll += 1;
                        }
                        cursor_col = 0;
                    }
                }
                KeyCode::Char(c) => {
                    if _m == KeyModifiers::CONTROL && c == 's' {
                        break;
                    };

                    let line: Vec<&str> =
                        UnicodeSegmentation::graphemes(&buf[cursor_row + v_scroll][..], true)
                            .collect();
                    let p1 = &line[..cursor_col].join("");
                    let p2 = &line[cursor_col..].join("");
                    let p: String = p1.to_string() + &c.to_string() + p2;
                    buf[cursor_row + v_scroll] = p;
                    cursor_col += 1;
                }
                _ => continue,
            }
        }
    }

    fs::write(&args[1], buf.join("\n"))?;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
