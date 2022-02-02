use crossterm::cursor::{
    MoveDown, MoveLeft, MoveRight, MoveTo, MoveToColumn, MoveToNextLine, MoveUp,
};
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue, style::Print};
use std::cmp;
use std::env;
use std::fs;
use std::io;
use std::io::{stdout, Write};
use term_size;

const TITLE: &str = "Freja's Editor for Editing Text";

fn add_to_buffer(b: &mut Vec<String>, line: usize, line_offset: usize, col: usize, s: &str) {
    let mut line_to_change = b[line - 2 + line_offset].to_string();
    line_to_change.insert_str(col - 7, s);
    b[line - 2 + line_offset] = line_to_change;
}

fn add_line(b: &mut Vec<String>, line: usize, line_offset: usize) {
    let _ = &b.insert(line + line_offset - 1, "".to_string());
}

fn backspace(b: &mut Vec<String>, line: usize, line_offset: usize, col: usize) {
    let mut line_to_change = b[line - 2 + line_offset].to_string();
    if col == 7 {
        // let _ = &b.remove(line + line_offset - 1);
        let _ = &b.splice(
            line + line_offset - 3..line + line_offset - 2,
            // b[line + line_offset].chars(),
            [],
        );
    } else {
        line_to_change.drain(col - 8..col - 8 + 1);
        b[line - 2 + line_offset] = line_to_change;
    }
}

fn show_buffer(
    stdout: &mut io::Stdout,
    buffer: &Vec<String>,
    line_offset: usize,
    text_w: usize,
    term_w: usize,
    term_h: usize,
) -> Result<(), io::Error> {
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    for _ in 0..3 {
        queue!(stdout, Print("/".to_string()))?;
    }
    queue!(stdout, Print(" ".to_string()))?;
    queue!(stdout, Print(TITLE.to_string()))?;
    queue!(stdout, Print(" ".to_string()))?;
    for _ in TITLE.len() + 4..term_w - 1 {
        queue!(stdout, Print("/".to_string()))?;
    }
    for _ in 1..term_h - 1 {
        queue!(
            stdout,
            Print("|".to_string()),
            MoveRight(term_w as u16 - 1),
            Print("|".to_string()),
            MoveToNextLine(1)
        )?;
    }
    for _ in 0..term_w {
        queue!(stdout, Print("/".to_string()))?;
    }
    queue!(stdout, MoveTo(5, 2))?;
    for n in line_offset..buffer.len() {
        let line = &buffer[n];
        // for (n, line) in buffer.iter().enumerate() {
        // let mut line_parts = vec![];
        // let line_len = line.chars().count();
        // let num_line_parts = line_len / text_w;
        // if line_len > text_w {
        // for n in 0..num_line_parts {
        // line_parts.push(line[n as usize * text_w..(n + 1) as usize * text_w].to_string());
        // }
        // }
        // line_parts.push(line[line_len - (line_len % text_w)..].to_string());

        queue!(
            stdout,
            MoveToColumn(3) /*, Print((n + 1).to_string() + "    ")*/
        )?;
        print!("{:>4} ", (n + 1).to_string());
        print!("{}", &line[..cmp::min(line.chars().count(), text_w + 1)]);
        // for (pn, part) in line_parts.iter().enumerate() {
        // if pn > 0 {
        // queue!(stdout, MoveDown(1))?;
        // }
        // queue!(stdout, MoveToColumn(9), Print(part))?;
        // }
        queue!(stdout, MoveDown(1), MoveToColumn(8))?;
    }

    stdout.flush()?;
    Ok(())
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let filename = &args[1];
    let contents = fs::read_to_string(filename)?;
    // let lines = contents.lines();
    let mut buffer: Vec<String> = contents
        .lines()
        .into_iter()
        .map(|x| x.to_string())
        .collect();

    let (term_w, term_h) = term_size::dimensions().unwrap();
    let text_w = term_w - 10; // Margins + line numbers

    let mut line_offset = 0; // How many lines are we scrolled down?

    let mut stdout = stdout();

    execute!(stdout, EnterAlternateScreen)?;
    // queue!(stdout, MoveTo(0, 0))?;
    // for _ in 0..3 {
    //     queue!(stdout, Print("/".to_string()))?;
    // }
    // queue!(stdout, Print(" ".to_string()))?;
    // queue!(stdout, Print(TITLE.to_string()))?;
    // queue!(stdout, Print(" ".to_string()))?;
    // for _ in TITLE.len() + 4..term_w - 1 {
    //     queue!(stdout, Print("/".to_string()))?;
    // }
    // for _ in 1..term_h - 1 {
    //     queue!(
    //         stdout,
    //         Print("|".to_string()),
    //         MoveRight(term_w as u16 - 1),
    //         Print("|".to_string()),
    //         MoveToNextLine(1)
    //     )?;
    // }
    // for _ in 0..term_w {
    //     queue!(stdout, Print("/".to_string()))?;
    // }

    show_buffer(&mut stdout, &buffer, line_offset, text_w, term_w, term_h)?;

    enable_raw_mode()?;

    execute!(stdout, MoveTo(7, 2))?;
    let mut cursor_pos = (7, 2);
    loop {
        if let Event::Key(KeyEvent { code, modifiers: m }) = read()? {
            if code == KeyCode::Esc.into() {
                break;
            }

            match code {
                KeyCode::Esc => break,
                KeyCode::Up => {
                    if cursor_pos.1 > 2 {
                        queue!(stdout, MoveUp(1))?;
                        cursor_pos.1 -= 1;
                    } else if cursor_pos.1 == 2 {
                        if line_offset > 0 {
                            line_offset -= 1;
                        }
                        // line_offset = cmp::max(line_offset - 1, 2);
                    }
                }
                KeyCode::Down => {
                    if cursor_pos.1 < term_h - 3 {
                        queue!(stdout, MoveDown(1))?;
                        cursor_pos.1 += 1;
                    } else if cursor_pos.1 == term_h - 3 {
                        // line_offset = cmp::min(line_offset + 1, term_h - 3);
                        line_offset += 1;
                    }
                }
                KeyCode::Left => {
                    if cursor_pos.0 > 7 {
                        queue!(stdout, MoveLeft(1))?;
                        cursor_pos.0 -= 1;
                    }
                }
                KeyCode::Right => {
                    if cursor_pos.0 < term_w - 3 {
                        queue!(stdout, MoveRight(1))?;
                        cursor_pos.0 += 1;
                    }
                }
                KeyCode::Backspace => {
                    if cursor_pos.0 >= 7 {
                        backspace(&mut buffer, cursor_pos.1, line_offset, cursor_pos.0);
                        if cursor_pos.0 > 7 {
                            cursor_pos.0 -= 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    add_line(&mut buffer, cursor_pos.1, line_offset);
                    cursor_pos.1 += 1;
                }
                KeyCode::Char(c) => {
                    if m == KeyModifiers::CONTROL {
                        if c == 's' {
                            let mut b = vec![];
                            for line in buffer {
                                b.push([line, "\n".to_string()].concat());
                            }
                            fs::write(filename, b.concat())?;
                            break;
                        }
                        continue;
                    }

                    add_to_buffer(
                        &mut buffer,
                        cursor_pos.1,
                        line_offset,
                        cursor_pos.0,
                        &c.to_string(),
                        // &line_offset.to_string(),
                    );
                    // show_buffer(&mut stdout, &buffer, line_offset, text_w, term_w, term_h)?;
                    cursor_pos.0 += 1;
                    // execute!(stdout, MoveTo(cursor_pos.0 as u16, cursor_pos.1 as u16))?;
                }
                _ => break,
            }
        }
        show_buffer(&mut stdout, &buffer, line_offset, text_w, term_w, term_h)?;
        execute!(stdout, MoveTo(cursor_pos.0 as u16, cursor_pos.1 as u16))?;
        stdout.flush()?;
    }

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;

    Ok(())
}
