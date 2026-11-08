use crate::{Block, BlockKind};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const MAX_ROWS: usize = 200;
pub const MAX_COLUMNS: usize = 30;
pub const MAX_CELL_BYTES: usize = 16_384;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SimpleTable {
    #[serde(default = "table_version")]
    pub version: u8,
    pub rows: Vec<Vec<String>>,
    pub header_row: bool,
    pub header_column: bool,
}
fn table_version() -> u8 {
    1
}
impl Default for SimpleTable {
    fn default() -> Self {
        Self {
            version: 1,
            rows: vec![vec![String::new(); 3]; 3],
            header_row: false,
            header_column: false,
        }
    }
}
impl SimpleTable {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported table version.".into());
        }
        if self.rows.is_empty() || self.rows.len() > MAX_ROWS {
            return Err("Table must have 1 to 200 rows.".into());
        }
        let width = self.rows[0].len();
        if width == 0 || width > MAX_COLUMNS || self.rows.iter().any(|row| row.len() != width) {
            return Err("Table must have 1 to 30 columns of equal length.".into());
        }
        if self
            .rows
            .iter()
            .flatten()
            .any(|cell| cell.len() > MAX_CELL_BYTES)
        {
            return Err("A table cell is too large.".into());
        }
        if self.rows.iter().flatten().map(String::len).sum::<usize>() > 1_048_576 {
            return Err("Table text is too large.".into());
        }
        Ok(())
    }
    pub fn read(block: &Block) -> Result<Self, String> {
        if block.kind != BlockKind::SimpleTable {
            return Err("Block is not a simple table.".into());
        }
        let table = match block.config.get("simple_table") {
            None => Self::default(),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|_| "Table data is invalid.".to_owned())?,
        };
        table.validate()?;
        Ok(table)
    }
    pub fn write(&self, block: &mut Block) -> Result<(), String> {
        self.validate()?;
        if block.kind != BlockKind::SimpleTable {
            return Err("Block is not a simple table.".into());
        }
        if block.config.is_null() {
            block.config = json!({});
        }
        block
            .config
            .as_object_mut()
            .ok_or("Block settings are invalid.")?
            .insert(
                "simple_table".into(),
                serde_json::to_value(self).map_err(|_| "Table data is invalid.")?,
            );
        Ok(())
    }
    pub fn insert_row(&mut self, after: usize) -> Result<(), String> {
        self.validate()?;
        if self.rows.len() >= MAX_ROWS || after >= self.rows.len() {
            return Err("Cannot add another row.".into());
        }
        self.rows
            .insert(after + 1, vec![String::new(); self.rows[0].len()]);
        Ok(())
    }
    pub fn delete_row(&mut self, index: usize) -> Result<(), String> {
        self.validate()?;
        if self.rows.len() <= 1 || index >= self.rows.len() {
            return Err("Keep at least one row.".into());
        }
        self.rows.remove(index);
        Ok(())
    }
    pub fn insert_column(&mut self, after: usize) -> Result<(), String> {
        self.validate()?;
        if self.rows[0].len() >= MAX_COLUMNS || after >= self.rows[0].len() {
            return Err("Cannot add another column.".into());
        }
        for row in &mut self.rows {
            row.insert(after + 1, String::new());
        }
        Ok(())
    }
    pub fn delete_column(&mut self, index: usize) -> Result<(), String> {
        self.validate()?;
        if self.rows[0].len() <= 1 || index >= self.rows[0].len() {
            return Err("Keep at least one column.".into());
        }
        for row in &mut self.rows {
            row.remove(index);
        }
        Ok(())
    }
    pub fn to_markdown(&self) -> Result<String, String> {
        self.validate()?;
        let mut out = format!(
            "<!-- moss:header-row={};header-column={} -->\n",
            u8::from(self.header_row),
            u8::from(self.header_column)
        );
        let encode = |value: &str| {
            let mut encoded = String::new();
            for ch in value.chars() {
                match ch {
                    '\n' => encoded.push_str("<br>"),
                    ch if ch.is_whitespace() => encoded.push_str(&format!("&#x{:X};", ch as u32)),
                    '&' => encoded.push_str("&amp;"),
                    '<' => encoded.push_str("&lt;"),
                    '>' => encoded.push_str("&gt;"),
                    '\\' => encoded.push_str("\\\\"),
                    '|' => encoded.push_str("\\|"),
                    _ => encoded.push(ch),
                }
            }
            encoded
        };
        for (index, row) in self.rows.iter().enumerate() {
            out.push_str("| ");
            out.push_str(
                &row.iter()
                    .map(|cell| encode(cell))
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            out.push_str(" |\n");
            if index == 0 {
                out.push_str("| ");
                out.push_str(&vec!["---"; row.len()].join(" | "));
                out.push_str(" |\n");
            }
        }
        Ok(out)
    }
    pub fn from_markdown(text: &str) -> Result<Self, String> {
        if text.len() > 4_194_304 {
            return Err("Markdown table is too large.".into());
        }
        let mut lines = text.lines();
        let first = lines.next().ok_or("Table is empty.")?;
        let (header_row, header_column, first_row) = if let Some(flags) = first
            .strip_prefix("<!-- moss:header-row=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            let (row, col) = flags
                .split_once(";header-column=")
                .ok_or("Invalid table metadata.")?;
            let flag = |s| match s {
                "0" => Ok(false),
                "1" => Ok(true),
                _ => Err("Invalid table metadata."),
            };
            (
                flag(row)?,
                flag(col)?,
                lines.next().ok_or("Table is empty.")?,
            )
        } else {
            (true, false, first)
        };
        let header = parse_row(first_row)?;
        let separator = parse_row(lines.next().ok_or("Table separator is missing.")?)?;
        if separator.len() != header.len()
            || separator.iter().any(|s| {
                let d = s.trim_matches(':');
                d.len() < 3 || !d.chars().all(|c| c == '-')
            })
        {
            return Err("Invalid Markdown table separator.".into());
        }
        let mut rows = vec![header];
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            if rows.len() == MAX_ROWS {
                return Err("Table has too many rows.".into());
            }
            rows.push(parse_row(line)?);
        }
        let table = Self {
            version: 1,
            rows,
            header_row,
            header_column,
        };
        table.validate()?;
        Ok(table)
    }
}
fn parse_row(line: &str) -> Result<Vec<String>, String> {
    let line = line.trim();
    if !line.contains('|') {
        return Err("Markdown table row needs a pipe.".into());
    }
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = if line.ends_with('|')
        && line[..line.len() - 1]
            .bytes()
            .rev()
            .take_while(|&b| b == b'\\')
            .count()
            % 2
            == 0
    {
        &line[..line.len() - 1]
    } else {
        line
    };
    let mut cells = vec![String::new()];
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            if ch != '|' && ch != '\\' {
                cells.last_mut().unwrap().push('\\');
            }
            cells.last_mut().unwrap().push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '|' {
            if cells.len() == MAX_COLUMNS {
                return Err("Table has too many columns.".into());
            }
            cells.push(String::new());
        } else {
            cells.last_mut().unwrap().push(ch);
        }
    }
    if escaped {
        cells.last_mut().unwrap().push('\\');
    }
    Ok(cells
        .into_iter()
        .map(|cell| decode_cell(cell.trim()))
        .collect())
}
fn decode_cell(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("&#x") {
            if let Some((number, tail)) = after.split_once(';') {
                if number.len() <= 6 && number.chars().all(|c| c.is_ascii_hexdigit()) {
                    if let Some(ch) = u32::from_str_radix(number, 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        if ch.is_whitespace() {
                            out.push(ch);
                            rest = tail;
                            continue;
                        }
                    }
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out.replace("<br>", "\n")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
#[cfg(test)]
#[path = "simple_table_tests.rs"]
mod tests;
