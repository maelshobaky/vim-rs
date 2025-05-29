use std::fs::File;
use std::io;

use ropey::iter::{Bytes, Chars, Chunks, Lines};
use ropey::{Rope, RopeSlice};

use crate::log;

pub struct Buffer {
    pub path: String,
    pub text: Rope,
    pub dirty: bool,
}

impl Buffer {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let text = Rope::from_reader(&mut io::BufReader::new(File::open(&path)?))?;

        Ok(Self {
            path: path.to_string(),
            text,
            dirty: false,
        })
    }

    pub fn get(&self, line: usize) -> Option<RopeSlice> {
        if self.text.len_lines() > line {
            return Some(self.text.line(line));
        }
        None
    }

    pub fn len(&self) -> usize {
        self.text.len_lines()
    }

    pub fn lines(&self) -> Lines {
        self.text.lines()
    }

    pub fn line_len(&self, line_i: usize) -> usize {
        self.text.line(line_i).len_chars()
    }

    pub fn insert_char(&mut self, line_i: usize, x: usize, c: char) {
        let line_start = self.text.line_to_char(line_i);
        self.text.insert_char(x + line_start, c);
    }

    pub fn insert_text(&mut self, line_i: usize, x: usize, text: &str) {
        let curs_index = self.text.line_to_char(line_i) + x;

        if !text.is_empty() {
            self.text.insert(curs_index, text);
        }
        self.dirty = true;
    }

    pub fn remove_char(&mut self, line_i: usize, x: usize) {
        let line_start = self.text.line_to_char(line_i);
        let char_index = line_start + x;
        self.text.remove(char_index..(char_index + 1));
    }
}
