//! Find literal text across retained terminal rows, including soft line wraps.

#[derive(Debug, Clone)]
pub(crate) enum Action {
    Open,
    Query(String),
    Move(bool),
    Close,
}

#[derive(Default)]
pub(crate) struct Search {
    pub open: bool,
    pub query: String,
    hits: Vec<Hit>,
    selected: usize,
    history: u16,
    scrolled: usize,
}

type Position = (u16, u16);

#[derive(Clone, Copy)]
struct Hit {
    first: Position,
    last: Position,
}

impl Search {
    pub fn count(&self) -> (usize, usize) {
        (
            if self.hits.is_empty() {
                0
            } else {
                self.selected + 1
            },
            self.hits.len(),
        )
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hits = Vec::new();
        self.scrolled = 0;
    }

    pub fn open(&mut self, screen: &mut vt100::Screen) {
        self.open = true;
        self.refresh(screen, screen.scrollback());
        self.reveal(screen);
    }

    pub fn set_query(&mut self, query: String, screen: &mut vt100::Screen) {
        self.query = query;
        self.selected = 0;
        self.hits.clear();
        self.refresh(screen, screen.scrollback());
        self.reveal(screen);
    }

    pub fn navigate(&mut self, forward: bool, screen: &mut vt100::Screen) {
        let count = self.hits.len();
        if count > 0 {
            self.selected = (self.selected + if forward { 1 } else { count - 1 }) % count;
            self.reveal(screen);
        }
    }

    fn reveal(&self, screen: &mut vt100::Screen) {
        if let Some(hit) = self.hits.get(self.selected) {
            let top = self.history.saturating_sub(screen.scrollback() as u16);
            let rows = screen.size().0;
            if hit.first.0 < top || hit.first.0 >= top + rows || hit.last.0 >= top + rows {
                screen.set_scrollback(usize::from(self.history.saturating_sub(hit.first.0)));
            }
        }
    }

    pub fn process(&mut self, parser: &mut vt100::Parser<impl vt100::Callbacks>, bytes: &[u8]) {
        if self.hits.is_empty() {
            parser.process(bytes);
            return;
        }
        // vt100 advances any nonzero scrollback offset when it adds history rows.
        // Small chunks keep the marker below the 2,000-row limit, including explicit scroll commands.
        for chunk in bytes.chunks(32) {
            let screen = parser.screen_mut();
            let alternate = screen.alternate_screen();
            let viewport = screen.scrollback();
            screen.set_scrollback(1);
            let marker = screen.scrollback();
            parser.process(chunk);
            let screen = parser.screen_mut();
            if alternate != screen.alternate_screen() || screen.scrollback() < marker {
                // Switching screens or resetting the terminal invalidates the previous selection.
                self.hits.clear();
                self.scrolled = 0;
                continue;
            }
            if marker == 0 {
                screen.set_scrollback(usize::MAX);
            }
            let added = screen.scrollback() - marker;
            self.scrolled += added;
            screen.set_scrollback(if viewport == 0 { 0 } else { viewport + added });
        }
    }

    pub fn refresh(&mut self, screen: &mut vt100::Screen, previous_offset: usize) {
        if !self.open || self.query.is_empty() {
            return;
        }
        let viewport = screen.scrollback();
        screen.set_scrollback(usize::MAX);
        let history = screen.scrollback() as u16;
        let added = std::mem::take(&mut self.scrolled);
        let evicted = (usize::from(self.history) + added).saturating_sub(usize::from(history));
        let anchor = self.hits.get(self.selected).map(|hit| hit.first);
        let old_history = self.history;
        self.history = history;
        self.hits.clear();
        let rows = screen.size().0;
        let previous_top = old_history.saturating_sub(previous_offset as u16);
        let was_visible =
            anchor.is_some_and(|(row, _)| (previous_top..previous_top + rows).contains(&row));
        let mut line = LogicalLine::default();
        let mut first = 0;
        while first < history + rows {
            let page_start = first.min(history);
            screen.set_scrollback(usize::from(history - page_start));
            let skip = first - page_start;
            for (row, contents) in
                (skip..rows).zip(screen.rows(0, u16::MAX).skip(usize::from(skip)))
            {
                line.push(screen, row, first, &contents);
                first += 1;
                if !screen.row_wrapped(row) || first == history + rows {
                    line.find(&self.query, &mut self.hits);
                }
            }
        }
        screen.set_scrollback(viewport);
        self.selected = anchor.map_or(0, |(row, col)| {
            let Some(row) = usize::from(row).checked_sub(evicted) else {
                return 0;
            };
            let pos = (row as u16, col);
            self.hits
                .partition_point(|hit| hit.first < pos)
                .min(self.hits.len().saturating_sub(1))
        });
        if was_visible {
            self.reveal(screen);
        }
    }

    /// Return whether the cell belongs to the current match or another match.
    pub fn highlight(&self, screen: &vt100::Screen, row: u16, col: u16) -> Option<bool> {
        if !self.open || self.hits.is_empty() {
            return None;
        }
        let pos = (
            self.history.saturating_sub(screen.scrollback() as u16) + row,
            col,
        );
        let index = self.hits.partition_point(|hit| hit.last < pos);
        self.hits
            .get(index)
            .filter(|hit| hit.first <= pos)
            .map(|_| index == self.selected)
    }

    /// A retained row can still have the terminal's former width after a resize.
    pub fn first_column(&self, screen: &vt100::Screen) -> u16 {
        let Some(hit) = self.hits.get(self.selected) else {
            return 0;
        };
        let (rows, cols) = screen.size();
        let top = self.history.saturating_sub(screen.scrollback() as u16);
        if hit.first.0 < top || hit.first.0 >= top + rows {
            return 0;
        }
        let end = if hit.first.0 == hit.last.0 {
            hit.last.1
        } else {
            hit.first.1
        };
        end.saturating_sub(cols - 1).min(hit.first.1)
    }
}

#[derive(Default)]
struct LogicalLine {
    text: String,
    rows: Vec<Row>,
}

struct Row {
    start: usize,
    number: u16,
    // Only Unicode rows need a byte-to-cell map. The terminal supplies actual cell widths.
    cells: Vec<(usize, u16, u16)>,
}

impl LogicalLine {
    fn push(&mut self, screen: &vt100::Screen, row: u16, number: u16, contents: &str) {
        let mut cells = Vec::new();
        if !contents.is_ascii() {
            let mut byte = 0;
            let mut col = 0;
            while byte < contents.len() {
                let Some(cell) = screen.cell(row, col) else {
                    break;
                };
                let width = 1 + u16::from(cell.is_wide());
                cells.push((byte, col, col + width - 1));
                col += width;
                byte += cell.contents().len().max(1);
            }
        }
        self.rows.push(Row {
            start: self.text.len(),
            number,
            cells,
        });
        self.text.push_str(contents);
    }

    fn position(&self, byte: usize, end: bool) -> Position {
        let row = &self.rows[self.rows.partition_point(|row| row.start <= byte) - 1];
        let local = byte - row.start;
        let col = if row.cells.is_empty() {
            local as u16
        } else {
            let (_, start, last) = row.cells[row.cells.partition_point(|cell| cell.0 <= local) - 1];
            if end { last } else { start }
        };
        (row.number, col)
    }

    fn find(&mut self, query: &str, hits: &mut Vec<Hit>) {
        for (byte, found) in self.text.match_indices(query) {
            hits.push(Hit {
                first: self.position(byte, false),
                last: self.position(byte + found.len() - 1, true),
            });
        }
        self.text.clear();
        self.rows.clear();
    }
}

#[cfg(test)]
mod tests;
