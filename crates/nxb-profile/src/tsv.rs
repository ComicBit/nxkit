//! The engine's own profile files: `game.tsv` and the other per-title tables (`dm/games/<title id>/*.tsv`).
//!
//! A file is kept as its physical lines, so reading and writing it again gives the same bytes (comments, blank lines,
//! trailing white space, a missing final newline and CR bytes included). A row that is not changed is written from its
//! original text, a row that is changed or new is written as `key<TAB>value<TAB>...`. The reading rules are those of
//! `dm/games/game.py`: a blank line or a line whose first non-blank character is `#` is not a row, the first cell is
//! the key, every cell is stripped.

use std::fmt;

/// One physical line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A blank line (its text, for white space).
    Blank(String),
    /// A `#` line (its text).
    Comment(String),
    /// A row.
    Row(Row),
}

/// A row: a key and its cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The first cell.
    pub key: String,
    /// The other cells.
    pub values: Vec<String>,
    /// The original text while the row is as read.
    raw: Option<String>,
}

impl Row {
    /// A new row.
    pub fn new(key: &str, values: &[&str]) -> Row {
        Row {
            key: key.to_string(),
            values: values.iter().map(|v| v.to_string()).collect(),
            raw: None,
        }
    }

    /// Replaces the cells (the row is then written in its normal form).
    pub fn set(&mut self, values: Vec<String>) {
        self.values = values;
        self.raw = None;
    }

    /// The cells joined with tabs, as `game.py get` gives them.
    pub fn joined(&self) -> String {
        self.values.join("\t")
    }

    fn text(&self) -> String {
        match &self.raw {
            Some(r) => r.clone(),
            None => {
                let mut s = self.key.clone();
                for v in &self.values {
                    s.push('\t');
                    s.push_str(v);
                }
                s
            }
        }
    }
}

/// A parse error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TsvError(pub String);

impl fmt::Display for TsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TsvError {}

/// A profile table, line by line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TsvFile {
    lines: Vec<Line>,
    /// Whether the text ended with a newline (a file with no line is as the empty text).
    final_newline: bool,
}

impl TsvFile {
    /// An empty file.
    pub fn new() -> TsvFile {
        TsvFile {
            lines: Vec::new(),
            final_newline: true,
        }
    }

    /// Reads the text of a file.
    pub fn parse(text: &str) -> TsvFile {
        let final_newline = text.is_empty() || text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let mut lines = Vec::new();
        if !text.is_empty() {
            for l in body.split('\n') {
                lines.push(line_of(l));
            }
        }
        TsvFile {
            lines,
            final_newline,
        }
    }

    /// Reads a file from disk.
    pub fn load(path: &std::path::Path) -> Result<TsvFile, TsvError> {
        let b = std::fs::read(path).map_err(|e| TsvError(format!("{}: {e}", path.display())))?;
        let t = String::from_utf8(b)
            .map_err(|_| TsvError(format!("{}: not UTF-8 text", path.display())))?;
        Ok(TsvFile::parse(&t))
    }

    /// The text, byte for byte what was read when nothing changed.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            match l {
                Line::Blank(t) | Line::Comment(t) => s.push_str(t),
                Line::Row(r) => s.push_str(&r.text()),
            }
        }
        if !self.lines.is_empty() && self.final_newline {
            s.push('\n');
        }
        s
    }

    /// Writes the file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), TsvError> {
        std::fs::write(path, self.to_text())
            .map_err(|e| TsvError(format!("{}: {e}", path.display())))
    }

    /// Every line.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Every row, in file order.
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.lines.iter().filter_map(|l| match l {
            Line::Row(r) => Some(r),
            _ => None,
        })
    }

    /// Every row of a key.
    pub fn rows_of<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Row> {
        self.rows().filter(move |r| r.key == key)
    }

    /// `g.get(key)`: the first row's cells joined by tabs.
    pub fn get(&self, key: &str) -> Option<String> {
        self.rows_of(key).next().map(Row::joined)
    }

    /// `g.get(key, default)`.
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    /// `g.words(key)`: the first row's value split at white space.
    pub fn words(&self, key: &str) -> Vec<String> {
        self.get(key)
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Sets the first row of a key (a new key goes at the end).
    pub fn set(&mut self, key: &str, values: &[&str]) {
        for l in &mut self.lines {
            if let Line::Row(r) = l
                && r.key == key
            {
                r.set(values.iter().map(|v| v.to_string()).collect());
                return;
            }
        }
        self.push_row(key, values);
    }

    /// Adds a row at the end.
    pub fn push_row(&mut self, key: &str, values: &[&str]) {
        self.lines.push(Line::Row(Row::new(key, values)));
    }

    /// Adds a comment line (`text` without the `# `).
    pub fn push_comment(&mut self, text: &str) {
        self.lines.push(Line::Comment(format!("# {text}")));
    }

    /// Adds a blank line.
    pub fn push_blank(&mut self) {
        self.lines.push(Line::Blank(String::new()));
    }

    /// Removes every row of a key, returns how many.
    pub fn remove(&mut self, key: &str) -> usize {
        let n = self.lines.len();
        self.lines
            .retain(|l| !matches!(l, Line::Row(r) if r.key == key));
        n - self.lines.len()
    }
}

fn line_of(l: &str) -> Line {
    let t = l.trim();
    if t.is_empty() {
        return Line::Blank(l.to_string());
    }
    if t.starts_with('#') {
        return Line::Comment(l.to_string());
    }
    let mut c = l.split('\t');
    let key = c.next().unwrap_or("").trim().to_string();
    Line::Row(Row {
        key,
        values: c.map(|v| v.trim().to_string()).collect(),
        raw: Some(l.to_string()),
    })
}
