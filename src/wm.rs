use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::cell::Cell;
use crate::config::Config;
use crate::window::Window;

/// What kind of mouse-driven operation is in progress.
enum MouseOp {
    Move {
        window_id: usize,
        offset_x: i32,
        offset_y: i32,
    },
    ResizeLeft {
        window_id: usize,
        anchor_right: i32,
    },
    ResizeRight {
        window_id: usize,
    },
    ResizeBottom {
        window_id: usize,
    },
    ResizeBottomLeft {
        window_id: usize,
        anchor_right: i32,
    },
    ResizeBottomRight {
        window_id: usize,
    },
}

pub struct WindowManager {
    pub(crate) windows: Vec<Window>,
    pub(crate) focused: usize,
    next_id: usize,
    drag: Option<MouseOp>,
    esc_pending: Option<std::time::Instant>,
    dirty: bool,
    pub(crate) config: Config,
    quit: bool,
    pub(crate) force_full: bool,
    pub(crate) term_cols: u16,
    pub(crate) term_rows: u16,
    pub(crate) front_buf: Vec<Vec<Cell>>,
    pub(crate) back_buf: Vec<Vec<Cell>>,
}

impl WindowManager {
    pub fn new(config: Config, term_cols: u16, term_rows: u16) -> anyhow::Result<Self> {
        let front_buf = vec![vec![Cell::default(); term_cols as usize]; term_rows as usize];
        let back_buf = vec![vec![Cell::default(); term_cols as usize]; term_rows as usize];
        Ok(Self {
            windows: vec![],
            focused: 0,
            next_id: 1,
            esc_pending: None,
            dirty: true,
            config,
            quit: false,
            force_full: false,
            drag: None,
            term_cols,
            term_rows,
            front_buf,
            back_buf,
        })
    }

    /// If an `Esc` was pressed and the timeout hasn't expired, forward it
    /// to the shell and clear the pending flag. Call this each iteration.
    pub fn expire_esc(&mut self) {
        if let Some(deadline) = self.esc_pending
            && deadline < std::time::Instant::now()
        {
            self.esc_pending = None;
            if !self.windows.is_empty() {
                let _ = self.windows[self.focused].write(b"\x1b");
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.kind == KeyEventKind::Release {
            return Ok(());
        }

        if self.esc_pending.take().is_some() {
            return self.dispatch_alt(key);
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            return self.dispatch_alt(key);
        }

        if key.code == KeyCode::Esc {
            self.esc_pending = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_millis(self.config.alt_timeout_ms),
            );
            return Ok(());
        }
        if !self.windows.is_empty()
            && let Some(bytes) = crate::input::encode_key(key)
        {
            self.windows[self.focused].write(&bytes)?;
        }
        Ok(())
    }

    fn dispatch_alt(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let shifted = key.modifiers.contains(KeyModifiers::SHIFT);
        match (&key.code, shifted) {
            (KeyCode::Char(c), _) if *c == self.config.keys.new_window => self.new_window(),
            (KeyCode::Char(c), _) if *c == self.config.keys.focus_next => {
                self.focus_next();
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.focus_prev => {
                self.focus_prev();
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.quit => {
                self.quit = true;
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.close_window => self.close_focused(),

            (KeyCode::Char(c), false) if *c == self.config.keys.move_left => {
                self.move_focused(-1, 0);
                Ok(())
            }
            (KeyCode::Left, false) => {
                self.move_focused(-1, 0);
                Ok(())
            }
            (KeyCode::Char(c), false) if *c == self.config.keys.move_down => {
                self.move_focused(0, 1);
                Ok(())
            }
            (KeyCode::Down, false) => {
                self.move_focused(0, 1);
                Ok(())
            }
            (KeyCode::Char(c), false) if *c == self.config.keys.move_up => {
                self.move_focused(0, -1);
                Ok(())
            }
            (KeyCode::Up, false) => {
                self.move_focused(0, -1);
                Ok(())
            }
            (KeyCode::Char(c), false) if *c == self.config.keys.move_right => {
                self.move_focused(1, 0);
                Ok(())
            }
            (KeyCode::Right, false) => {
                self.move_focused(1, 0);
                Ok(())
            }

            (KeyCode::Char(c), _) if *c == self.config.keys.resize_left => {
                self.resize_focused(-1, 0);
                Ok(())
            }
            (KeyCode::Left, true) => {
                self.resize_focused(-1, 0);
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.resize_down => {
                self.resize_focused(0, 1);
                Ok(())
            }
            (KeyCode::Down, true) => {
                self.resize_focused(0, 1);
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.resize_up => {
                self.resize_focused(0, -1);
                Ok(())
            }
            (KeyCode::Up, true) => {
                self.resize_focused(0, -1);
                Ok(())
            }
            (KeyCode::Char(c), _) if *c == self.config.keys.resize_right => {
                self.resize_focused(1, 0);
                Ok(())
            }
            (KeyCode::Right, true) => {
                self.resize_focused(1, 0);
                Ok(())
            }

            (KeyCode::Char(c), _) if *c == self.config.keys.pin_window => {
                self.toggle_pin_focused();
                Ok(())
            }

            (KeyCode::Char(c), _) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as usize;
                self.focus_window(n.wrapping_sub(1));
                Ok(())
            }

            _ => Ok(()),
        }
    }

    fn new_window(&mut self) -> anyhow::Result<()> {
        let new_w = ((self.term_cols as f64 * self.config.layout.new_window_width_ratio) as u16)
            .max(self.config.layout.min_window_cols);
        let new_h = ((self.term_rows as f64 * self.config.layout.new_window_height_ratio) as u16)
            .max(self.config.layout.min_window_rows);

        let mut new_x = ((self.term_cols / 2) - (new_w / 2)) as i32;
        let mut new_y = ((self.term_rows / 2) - (new_h / 2)) as i32;

        if !self.windows.is_empty() {
            let f = &self.windows[self.focused];
            new_x = f.x + self.config.layout.cascade_offset_x;
            new_y = f.y + self.config.layout.cascade_offset_y;
        }

        if new_x + new_w as i32 > self.term_cols as i32 {
            new_x = 0;
        }
        if new_y + new_h as i32 > self.term_rows as i32 {
            new_y = 0;
        }

        let id = self.next_id;
        self.next_id += 1;
        let shell = if !self.config.shell.is_empty() {
            self.config.shell.clone()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
        };
        let win = Window::new(
            id,
            &shell,
            new_x,
            new_y,
            new_w,
            new_h,
            !self.config.disable_mouse,
        )?;
        let new_idx = self.windows.len();
        self.windows.push(win);
        self.focused = bring_to_front(&mut self.windows, new_idx);
        self.force_full = true;
        self.dirty = true;
        Ok(())
    }

    fn focus_next(&mut self) {
        if self.windows.len() < 2 {
            return;
        }
        let new_focus = (self.focused + 1) % self.windows.len();
        self.focused = bring_to_front(&mut self.windows, new_focus);
        self.dirty = true;
    }

    fn focus_prev(&mut self) {
        if self.windows.len() < 2 {
            return;
        }
        let new_focus = if self.focused == 0 {
            self.windows.len() - 1
        } else {
            self.focused - 1
        };
        self.focused = bring_to_front(&mut self.windows, new_focus);
        self.dirty = true;
    }
    fn move_focused(&mut self, dx: i32, dy: i32) {
        if self.windows.is_empty() {
            return;
        }
        let w = &mut self.windows[self.focused];
        w.x = (w.x + dx).clamp(-(w.w as i32 - 1), self.term_cols as i32 - 1);
        w.y = (w.y + dy).clamp(-(w.h as i32 - 1), self.term_rows as i32 - 1);
        self.force_full = true;
        self.dirty = true;
    }
    fn resize_focused(&mut self, dw: i32, dh: i32) {
        if self.windows.is_empty() {
            return;
        }
        let w = &mut self.windows[self.focused];
        let new_w = (w.w as i32 + dw).max(self.config.layout.min_window_cols as i32) as u16;
        let new_h = (w.h as i32 + dh).max(self.config.layout.min_window_rows as i32) as u16;
        w.w = new_w;
        w.h = new_h;
        w.pty.resize(w.content_h(), w.content_w());
        self.force_full = true;
        self.dirty = true;
    }
    fn close_focused(&mut self) -> anyhow::Result<()> {
        if self.windows.is_empty() {
            return Ok(());
        }
        self.windows.remove(self.focused);
        if self.focused >= self.windows.len() {
            self.focused = self.windows.len().saturating_sub(1);
        }
        if !self.windows.is_empty() {
            self.focused = bring_to_front(&mut self.windows, self.focused);
        }
        self.force_full = true;
        self.dirty = true;
        Ok(())
    }

    fn focus_window(&mut self, n: usize) {
        if n < self.windows.len() && n != self.focused {
            self.focused = bring_to_front(&mut self.windows, n);
            self.dirty = true;
        }
    }

    pub fn resize_screen(&mut self, term_rows: u16, term_cols: u16) {
        self.term_rows = term_rows;
        self.term_cols = term_cols;
        self.front_buf = vec![vec![Cell::default(); term_cols as usize]; term_rows as usize];
        self.back_buf = vec![vec![Cell::default(); term_cols as usize]; term_rows as usize];
        self.dirty = true;
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) -> anyhow::Result<()> {
        if self.config.disable_mouse {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                for i in (0..self.windows.len()).rev() {
                    let w = &self.windows[i];
                    if w.hit_title_bar(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        let offset_x = event.column as i32 - w.x;
                        let offset_y = event.row as i32 - w.y;
                        self.drag = Some(MouseOp::Move {
                            window_id: w.id,
                            offset_x,
                            offset_y,
                        });
                        self.dirty = true;
                        break;
                    }
                    if w.hit_bottom_left_corner(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        let anchor_right = w.x + w.w as i32;
                        self.drag = Some(MouseOp::ResizeBottomLeft {
                            window_id: w.id,
                            anchor_right,
                        });
                        self.dirty = true;
                        break;
                    }
                    if w.hit_bottom_right_corner(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        self.drag = Some(MouseOp::ResizeBottomRight { window_id: w.id });
                        self.dirty = true;
                        break;
                    }
                    if w.hit_left_edge(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        let anchor_right = w.x + w.w as i32;
                        self.drag = Some(MouseOp::ResizeLeft {
                            window_id: w.id,
                            anchor_right,
                        });
                        self.dirty = true;
                        break;
                    }
                    if w.hit_right_edge(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        self.drag = Some(MouseOp::ResizeRight { window_id: w.id });
                        self.dirty = true;
                        break;
                    }
                    if w.hit_bottom_edge(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        let w = &self.windows[self.focused];
                        self.drag = Some(MouseOp::ResizeBottom { window_id: w.id });
                        self.dirty = true;
                        break;
                    }
                    if w.contains_point(event.column, event.row) {
                        self.focused = bring_to_front(&mut self.windows, i);
                        self.dirty = true;
                        break;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.drag {
                Some(MouseOp::Move {
                    window_id,
                    offset_x,
                    offset_y,
                }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_x = event.column as i32 - offset_x;
                        let new_y = event.row as i32 - offset_y;
                        w.x = new_x.clamp(-(w.w as i32 - 1), self.term_cols as i32 - 1);
                        w.y = new_y.clamp(-(w.h as i32 - 1), self.term_rows as i32 - 1);
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                Some(MouseOp::ResizeLeft {
                    window_id,
                    anchor_right,
                }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_x = (event.column as i32)
                            .min(anchor_right - self.config.layout.min_window_cols as i32);
                        let new_w = (anchor_right - new_x)
                            .max(self.config.layout.min_window_cols as i32)
                            as u16;
                        w.x = new_x;
                        w.w = new_w;
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                Some(MouseOp::ResizeRight { window_id }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_w = (event.column as i32 - w.x + 1)
                            .max(self.config.layout.min_window_cols as i32)
                            as u16;
                        w.w = new_w;
                        w.pty.resize(w.content_h(), w.content_w());
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                Some(MouseOp::ResizeBottom { window_id }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_h = (event.row as i32 - w.y + 1)
                            .max(self.config.layout.min_window_rows as i32)
                            as u16;
                        w.h = new_h;
                        w.pty.resize(w.content_h(), w.content_w());
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                Some(MouseOp::ResizeBottomLeft {
                    window_id,
                    anchor_right,
                }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_x = (event.column as i32)
                            .min(anchor_right - self.config.layout.min_window_cols as i32);
                        let new_w = (anchor_right - new_x)
                            .max(self.config.layout.min_window_cols as i32)
                            as u16;
                        let new_h = (event.row as i32 - w.y + 1)
                            .max(self.config.layout.min_window_rows as i32)
                            as u16;
                        w.x = new_x;
                        w.w = new_w;
                        w.h = new_h;
                        w.pty.resize(w.content_h(), w.content_w());
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                Some(MouseOp::ResizeBottomRight { window_id }) => {
                    if let Some(idx) = self.windows.iter().position(|w| w.id == window_id) {
                        let w = &mut self.windows[idx];
                        let new_w = (event.column as i32 - w.x + 1)
                            .max(self.config.layout.min_window_cols as i32)
                            as u16;
                        let new_h = (event.row as i32 - w.y + 1)
                            .max(self.config.layout.min_window_rows as i32)
                            as u16;
                        w.w = new_w;
                        w.h = new_h;
                        w.pty.resize(w.content_h(), w.content_w());
                        self.force_full = true;
                        self.dirty = true;
                    }
                }
                None => {}
            },
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Process PTY output for all windows, and refresh window titles
    /// from the foreground process. Sets dirty if any data or title changed.
    pub fn process_all(&mut self) {
        for w in &mut self.windows {
            if w.process() {
                self.dirty = true;
            }
            if let Some(name) = w.pty.foreground_process_name()
                && name != w.title
            {
                w.title = name;
                self.dirty = true;
            }
        }
    }

    pub fn reap_dead_windows(&mut self) -> anyhow::Result<()> {
        let mut i = 0;
        let mut removed = false;
        while i < self.windows.len() {
            if let Ok(Some(_)) = self.windows[i].try_wait() {
                self.windows.remove(i);
                if i <= self.focused && self.focused > 0 {
                    self.focused -= 1;
                }
                removed = true;
            } else {
                i += 1;
            }
        }

        if removed {
            self.dirty = true;
        }

        if !self.windows.is_empty() && self.focused >= self.windows.len() {
            self.focused = 0;
            self.dirty = true;
        }

        Ok(())
    }

    fn toggle_pin_focused(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        self.windows[self.focused].pinned = !self.windows[self.focused].pinned;
        self.focused = bring_to_front(&mut self.windows, self.focused);
        self.dirty = true;
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn is_quit_requested(&self) -> bool {
        self.quit
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Move the window at `idx` to its correct z-order position and return the new index.
/// Pinned windows go to the very end (on top of all others).
/// Unpinned windows go just before the first pinned window, or to the end if none are pinned.
fn bring_to_front(windows: &mut Vec<Window>, idx: usize) -> usize {
    // Already at the end — only correct for pinned windows.
    if idx >= windows.len() - 1 && windows[idx].pinned {
        return idx;
    }
    let is_pinned = windows[idx].pinned;
    let w = windows.remove(idx);
    if is_pinned {
        windows.push(w);
        windows.len() - 1
    } else {
        let first_pinned = windows.iter().position(|win| win.pinned);
        match first_pinned {
            Some(pos) => {
                windows.insert(pos, w);
                pos
            }
            None => {
                windows.push(w);
                windows.len() - 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Pure z-order logic extracted from bring_to_front for testing.
    /// Takes a slice of pinned flags, the index to bring to front, and returns the new index.
    fn bring_to_front_test(pinned: &mut Vec<bool>, idx: usize) -> usize {
        // Already at the end — only correct for pinned.
        if idx >= pinned.len() - 1 && pinned[idx] {
            return idx;
        }
        let is_pinned = pinned[idx];
        pinned.remove(idx);
        if is_pinned {
            pinned.push(true);
            pinned.len() - 1
        } else {
            let first_pinned = pinned.iter().position(|&p| p);
            match first_pinned {
                Some(pos) => {
                    pinned.insert(pos, false);
                    pos
                }
                None => {
                    pinned.push(false);
                    pinned.len() - 1
                }
            }
        }
    }

    #[test]
    fn unpinned_no_pinned_goes_to_end() {
        // [false, false, false*] -> bring idx 1 to front -> [false, false, false] idx 2
        let mut v = vec![false, false, false];
        let new_idx = bring_to_front_test(&mut v, 1);
        assert_eq!(new_idx, 2);
        assert_eq!(v, vec![false, false, false]);
    }

    #[test]
    fn pinned_goes_to_very_end() {
        // [false, true*, false] -> bring idx 1 to front -> [false, false, true] idx 2
        let mut v = vec![false, true, false];
        let new_idx = bring_to_front_test(&mut v, 1);
        assert_eq!(new_idx, 2);
        assert_eq!(v, vec![false, false, true]);
    }

    #[test]
    fn unpinned_stops_before_first_pinned() {
        // [true, false*, false] — unpinned at idx 1 above pinned; bring to front
        // Remove idx 1 → [true, false]; first_pinned at 0; insert at 0 → [false, true, false]
        let mut v = vec![true, false, false];
        let new_idx = bring_to_front_test(&mut v, 1);
        assert_eq!(new_idx, 0);
        assert_eq!(v, vec![false, true, false]);
    }

    #[test]
    fn pinned_already_at_end_stays() {
        // [false, false, true*] -> idx 2 already last -> stays
        let mut v = vec![false, false, true];
        let new_idx = bring_to_front_test(&mut v, 2);
        assert_eq!(new_idx, 2);
        assert_eq!(v, vec![false, false, true]);
    }

    #[test]
    fn multiple_pinned_lifo_order() {
        // [true(A), false, true(B)*] -> bring B to front -> [true(A), false, true(B)]
        // B already at end
        let mut v = vec![true, false, true];
        let new_idx = bring_to_front_test(&mut v, 2);
        assert_eq!(new_idx, 2);
        assert_eq!(v, vec![true, false, true]);

        // Now bring A (idx 0) to front -> [false, true(B), true(A)] idx 2
        let new_idx = bring_to_front_test(&mut v, 0);
        assert_eq!(new_idx, 2);
        assert_eq!(v, vec![false, true, true]);
    }

    #[test]
    fn unpinned_stops_before_multiple_pinned() {
        // [false*, true, true] -> bring idx 0 -> [true, true, false] idx 2? No:
        // first_pinned is idx 0 (after removing false), so insert at 0 -> [false, true, true], pos 0
        let mut v = vec![false, true, true];
        let new_idx = bring_to_front_test(&mut v, 0);
        assert_eq!(new_idx, 0);
        assert_eq!(v, vec![false, true, true]);
    }

    #[test]
    fn new_unpinned_goes_below_pinned() {
        // Simulates: window is pinned, then a new window spawns.
        // [true(A)] — push false(B), then bring_to_front on the new window (idx 1)
        let mut v = vec![true];
        v.push(false);
        let new_idx = bring_to_front_test(&mut v, 1);
        assert_eq!(new_idx, 0); // new unpinned window goes below pinned
        assert_eq!(v, vec![false, true]);
    }
}
