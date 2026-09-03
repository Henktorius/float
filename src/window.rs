use std::io;

use crate::pty::Pty;

pub struct Window {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    pub title: String,
    pub decorations: bool,
    pub pinned: bool,
    pub pty: Pty,
}

impl Window {
    pub fn new(
        id: usize,
        shell: &str,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
        decorations: bool,
    ) -> anyhow::Result<Self> {
        let (content_w, content_h, w, h) = if decorations {
            let content_w = if w < 6 { 2 } else { w - 2 };
            let content_h = if h < 5 { 1 } else { h.saturating_sub(4) };
            (content_w, content_h, content_w + 2, content_h + 4)
        } else {
            let content_w = if w < 4 { 2 } else { w - 2 };
            let content_h = if h < 3 { 1 } else { h - 2 };
            (content_w, content_h, content_w + 2, content_h + 2)
        };

        let pty = Pty::spawn(shell, content_h, content_w)?;
        let title = pty.process_name().unwrap_or_else(|| shell.to_string());
        Ok(Window {
            id,
            x,
            y,
            w,
            h,
            title,
            decorations,
            pinned: false,
            pty,
        })
    }

    pub fn content_w(&self) -> u16 {
        self.w.saturating_sub(2)
    }

    pub fn content_h(&self) -> u16 {
        if self.decorations {
            self.h.saturating_sub(4)
        } else {
            self.h.saturating_sub(2)
        }
    }

    pub fn content_x(&self) -> i32 {
        self.x + 1
    }

    pub fn content_y(&self) -> i32 {
        if self.decorations {
            self.y + 3
        } else {
            self.y + 1
        }
    }

    pub fn process(&mut self) -> bool {
        self.pty.process()
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.pty.write(bytes)
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.pty.screen()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<portable_pty::ExitStatus>> {
        self.pty.try_wait()
    }

    /// Returns true if (col, row) is inside the title bar area.
    /// Title bar spans the top border row (y) and the title row (y+1),
    /// full window width.
    pub fn hit_title_bar(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        r >= self.y && r <= self.y + 1 && c >= self.x && c < self.x + self.w as i32
    }

    /// Returns true if (col, row) is on the left border of the window.
    pub fn hit_left_edge(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c == self.x && r >= self.y && r < self.y + self.h as i32
    }

    /// Returns true if (col, row) is on the right border of the window.
    pub fn hit_right_edge(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c == self.x + self.w as i32 - 1 && r >= self.y && r < self.y + self.h as i32
    }

    /// Returns true if (col, row) is on the bottom border of the window.
    pub fn hit_bottom_edge(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        r == self.y + self.h as i32 - 1 && c >= self.x && c < self.x + self.w as i32
    }

    /// Returns true if (col, row) is the bottom-left corner of the window.
    pub fn hit_bottom_left_corner(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c == self.x && r == self.y + self.h as i32 - 1
    }

    /// Returns true if (col, row) is the bottom-right corner of the window.
    pub fn hit_bottom_right_corner(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c == self.x + self.w as i32 - 1 && r == self.y + self.h as i32 - 1
    }

    /// Returns true if (col, row) is inside the window's content area
    /// (everything within the border and, if present, below the title row).
    pub fn content_hit(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c >= self.content_x()
            && c < self.content_x() + self.content_w() as i32
            && r >= self.content_y()
            && r < self.content_y() + self.content_h() as i32
    }

    /// Returns true if (col, row) is anywhere inside the window's bounding box.
    pub fn contains_point(&self, col: u16, row: u16) -> bool {
        let c = col as i32;
        let r = row as i32;
        c >= self.x && c < self.x + self.w as i32 && r >= self.y && r < self.y + self.h as i32
    }
}
