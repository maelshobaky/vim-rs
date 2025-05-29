use std::io::{stdout, Stdout, Write};

use anyhow::Ok;

use crossterm::{
    cursor,
    event::{self, read, Event, KeyModifiers},
    style::{self, Stylize},
    terminal::{self, Clear, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand, QueueableCommand,
};
use ropey::RopeSlice;

use crate::buffer::Buffer;

enum Action {
    Quit,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    EnterMode(Mode),
    InsertChar(char),
    NewLine,
    PageDown,
    PageUp,
    EndOfLine,
    StartOfLine,
    DelCharBefore,
    DelCharAtCursor,
}

#[derive(Debug)]
enum Mode {
    Normal,
    Insert,
}

pub struct Editor {
    buffer: Buffer,
    stdout: Stdout,
    size: (u16, u16),
    vtop: u16,
    vleft: u16,
    cx: u16,
    cy: u16,
    mode: Mode,
}

impl Drop for Editor {
    fn drop(&mut self) {
        _ = self.stdout.flush();
        _ = self.stdout.execute(LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
    }
}

impl Editor {
    pub fn new(buffer: Buffer) -> anyhow::Result<Self> {
        let mut stdout = stdout();
        terminal::enable_raw_mode()?;

        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(Clear(terminal::ClearType::All))?;

        Ok(Editor {
            buffer,
            stdout,
            size: terminal::size()?,
            vtop: 0,
            vleft: 0,
            cx: 0,
            cy: 0,
            mode: Mode::Normal,
        })
    }

    fn draw(&mut self) -> anyhow::Result<()> {
        self.stdout.execute(Clear(terminal::ClearType::All))?;
        self.draw_viewport()?;
        self.draw_statusline()?;
        self.stdout.queue(cursor::MoveTo(self.cx, self.cy))?;
        self.stdout.flush()?;
        Ok(())
    }

    fn vheight(&self) -> u16 {
        self.size.1 - 2
    }

    fn vwidth(&self) -> u16 {
        self.size.0
    }

    fn line_length(&self) -> u16 {
        if let Some(line) = self.viewport_line(self.cy) {
            return line.len_chars() as u16;
        }
        0
    }

    fn buffer_line(&self) -> usize {
        (self.vtop + self.cy) as usize
    }

    fn viewport_line(&self, n: u16) -> Option<RopeSlice> {
        let buffer_line = self.vtop + n;
        self.buffer.get(buffer_line as usize)
    }

    fn draw_viewport(&mut self) -> anyhow::Result<()> {
        let vwidth = self.vwidth() as usize;

        for i in 0..self.vheight() {
            let line = match self.viewport_line(i) {
                None => String::new(),
                Some(s) => s.to_string(),
            };

            self.stdout.queue(cursor::MoveTo(0, i))?;
            self.stdout
                .queue(style::Print(format!("{line:<width$}", width = vwidth)))?;
        }

        Ok(())
    }

    fn draw_statusline(&mut self) -> anyhow::Result<()> {
        let separator = "\u{e0b0}";
        let separator_rev = "\u{e0b2}";
        let file = format!(" [{}]", self.buffer.path);
        let mode = format!(" {:?} ", self.mode).to_uppercase();
        let pos = format!(" {}:{} ", self.cx, self.cy);
        let file_width = self.size.0 - mode.len() as u16 - pos.len() as u16 - 2;
        self.stdout.queue(cursor::MoveTo(0, self.size.1 - 2))?;
        self.stdout.queue(style::PrintStyledContent(
            mode.with(style::Color::Black)
                .on(style::Color::Rgb {
                    r: 184,
                    g: 144,
                    b: 243,
                })
                .bold(),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            separator
                .on(style::Color::Rgb {
                    r: 67,
                    g: 70,
                    b: 89,
                })
                .with(style::Color::Rgb {
                    r: 184,
                    g: 144,
                    b: 243,
                }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            format!("{:<width$}", file, width = file_width as usize)
                .on(style::Color::Rgb {
                    r: 67,
                    g: 70,
                    b: 89,
                })
                .with(style::Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            separator_rev
                .on(style::Color::Rgb {
                    r: 67,
                    g: 70,
                    b: 89,
                })
                .with(style::Color::Rgb {
                    r: 184,
                    g: 144,
                    b: 243,
                }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            pos.with(style::Color::Black)
                .on(style::Color::Rgb {
                    r: 184,
                    g: 144,
                    b: 243,
                })
                .bold(),
        ))?;

        Ok(())
    }

    fn assert_cursor_boundaries(&mut self, mut cx_history: u16) -> u16 {
        let bottom_scroll_limit = self.vtop + self.vheight();
        let cursor_below_vp = self.cy > self.vheight() - 1;

        if (bottom_scroll_limit) > self.buffer.len() as u16 {
            self.vtop = self.buffer.len() as u16 - self.vheight();
        }
        if cursor_below_vp && (bottom_scroll_limit) < self.buffer.len() as u16 {
            self.vtop += 1;
        }
        if cursor_below_vp {
            self.cy = self.vheight() - 1;
        }

        if self.cx > self.vwidth() || self.cx >= self.line_length() {
            if self.cy < self.vheight() - 1 {
                self.cx = 0;
                self.cy += 1;
            }
            if (bottom_scroll_limit) < self.buffer.len() as u16 {
                self.cx = 0;
                self.vtop += 1;
            } else if (bottom_scroll_limit) > self.buffer.len() as u16 || self.line_length() == 0 {
                self.cx = 0;
            }
            cx_history = self.cx;
        }

        cx_history
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut cx_history = self.cx;
        loop {
            cx_history = self.assert_cursor_boundaries(cx_history);
            self.draw()?;

            if let Some(action) = self.handle_event(read()?)? {
                match action {
                    Action::Quit => break,
                    Action::MoveUp => {
                        self.cy = self.cy.saturating_sub(1);
                        if self.cy == 0 && self.vtop > 0 {
                            self.vtop -= 1;
                        }
                        if cx_history <= self.line_length() {
                            self.cx = cx_history;
                        } else if cx_history > self.line_length() {
                            self.cx = self.line_length();
                        }
                    }
                    Action::MoveDown => {
                        self.cy += 1;

                        if cx_history <= self.line_length() {
                            self.cx = cx_history;
                        } else if cx_history > self.line_length() {
                            self.cx = self.line_length();
                        }
                    }
                    Action::MoveLeft => {
                        if self.cx == self.vleft {
                            if self.cy == 0 && self.vtop > 0 {
                                self.vtop -= 1;
                                self.cx = self.line_length();
                            } else if self.cy > 0 {
                                self.cy = self.cy.saturating_sub(1);
                                self.cx = self.line_length();
                            }
                        }
                        self.cx = self.cx.saturating_sub(1);
                        cx_history = self.cx;
                    }
                    Action::MoveRight => {
                        self.cx += 1;
                        cx_history = self.cx;
                    }
                    Action::EnterMode(new_mode) => {
                        self.mode = new_mode;
                        self.stdout.execute(Clear(terminal::ClearType::Purge))?;
                    }
                    Action::InsertChar(c) => {
                        self.buffer
                            .insert_char(self.buffer_line(), self.cx as usize, c);
                        //self.cx += 1;
                    }
                    Action::DelCharBefore => {
                        if self.cx > self.vleft {
                            self.buffer
                                .remove_char(self.buffer_line(), self.cx as usize - 1);
                            self.cx = self.cx.saturating_sub(1);
                            cx_history = self.cx;
                        }
                    }
                    Action::DelCharAtCursor => {
                        if self.cx < self.line_length() && self.line_length() > 0 {
                            self.buffer
                                .remove_char(self.buffer_line(), self.cx as usize);
                        }
                        if (self.vtop + self.vheight()) > self.buffer.len() as u16 {
                            self.cy += 1;
                        }
                    }
                    Action::NewLine => {
                        self.buffer
                            .insert_text(self.buffer_line(), self.cx as usize, "\u{000a}");
                        self.cx = 0;
                        self.cy += 1;
                    }
                    Action::PageDown => {
                        self.vtop += self.vheight();
                        if (self.vtop + self.vheight()) > self.buffer.len() as u16 {
                            self.cy = self.vheight() - 1;
                        }

                        if cx_history <= self.line_length() {
                            self.cx = cx_history;
                        } else if cx_history > self.line_length() {
                            self.cx = self.line_length();
                        }
                    }
                    Action::PageUp => {
                        if self.vtop >= self.vheight() {
                            self.vtop -= self.vheight();
                        } else {
                            self.vtop = 0;
                            self.cy = 0;
                        }
                        if cx_history <= self.line_length() {
                            self.cx = cx_history;
                        } else if cx_history > self.line_length() {
                            self.cx = self.line_length();
                        }
                    }
                    Action::EndOfLine => {
                        self.cx = self.line_length();
                        cx_history = self.cx;
                    }
                    Action::StartOfLine => {
                        self.cx = self.vleft;
                        cx_history = self.cx;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, ev: Event) -> anyhow::Result<Option<Action>> {
        if matches!(ev, Event::Resize(_, _)) {
            self.size = terminal::size()?;
        }
        match self.mode {
            Mode::Normal => self.handle_normal_event(ev),
            Mode::Insert => self.handle_insert_event(ev),
        }
    }

    fn handle_insert_event(&self, ev: Event) -> anyhow::Result<Option<Action>> {
        let action = match ev {
            Event::Key(key_event) => match key_event.kind {
                event::KeyEventKind::Press => match key_event.code {
                    event::KeyCode::Esc => Some(Action::EnterMode(Mode::Normal)),
                    event::KeyCode::Up => Some(Action::MoveUp),
                    event::KeyCode::Down => Some(Action::MoveDown),
                    event::KeyCode::Left => Some(Action::MoveLeft),
                    event::KeyCode::Right => Some(Action::MoveRight),
                    event::KeyCode::Enter => Some(Action::NewLine),
                    event::KeyCode::Char(c) => Some(Action::InsertChar(c)),
                    event::KeyCode::PageDown => Some(Action::PageDown),
                    event::KeyCode::PageUp => Some(Action::PageUp),
                    event::KeyCode::End => Some(Action::EndOfLine),
                    event::KeyCode::Home => Some(Action::StartOfLine),
                    event::KeyCode::Backspace => Some(Action::DelCharBefore),
                    event::KeyCode::Delete => Some(Action::DelCharAtCursor),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        Ok(action)
    }

    fn handle_normal_event(&self, ev: Event) -> anyhow::Result<Option<Action>> {
        let action = match ev {
            Event::Key(key_event) => match key_event.kind {
                event::KeyEventKind::Press => match key_event.code {
                    event::KeyCode::Char('q') => Some(Action::Quit),
                    event::KeyCode::Up | event::KeyCode::Char('k') => Some(Action::MoveUp),
                    event::KeyCode::Down | event::KeyCode::Char('l') => Some(Action::MoveDown),
                    event::KeyCode::Left | event::KeyCode::Char('j') => Some(Action::MoveLeft),
                    event::KeyCode::Right | event::KeyCode::Char(';') => Some(Action::MoveRight),
                    event::KeyCode::Char('i') => Some(Action::EnterMode(Mode::Insert)),
                    event::KeyCode::PageDown => Some(Action::PageDown),
                    event::KeyCode::PageUp => Some(Action::PageUp),
                    event::KeyCode::Char('f') if key_event.modifiers == KeyModifiers::CONTROL => {
                        Some(Action::PageDown)
                    }
                    event::KeyCode::Char('b') if key_event.modifiers == KeyModifiers::CONTROL => {
                        Some(Action::PageUp)
                    }
                    event::KeyCode::Char('$') | event::KeyCode::End => Some(Action::EndOfLine),
                    event::KeyCode::Char('0') | event::KeyCode::Home => Some(Action::StartOfLine),
                    event::KeyCode::Char('x') => Some(Action::DelCharAtCursor),

                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        Ok(action)
    }
}
