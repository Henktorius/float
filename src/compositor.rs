use std::io::{self, Write};

use crossterm::{
    cursor, queue,
    style::{Print, SetBackgroundColor, SetForegroundColor},
};

use crate::cell::{self, Cell};
use crate::frame;
use crate::wm::WindowManager;

impl WindowManager {
    /// Compose all windows bottom-to-top, diff against previous frame, emit only changes.
    pub fn composite(&mut self) -> io::Result<()> {
        let rows = self.term_rows;
        let cols = self.term_cols;

        for row in &mut self.back_buf {
            row.fill(Cell::default());
        }

        if self.windows.is_empty() {
            frame::draw_hint(
                &mut self.back_buf,
                rows,
                cols,
                "Press Alt+c to open a window",
                self.config.theme.hint_text.to_crossterm(),
            );
        } else {
            for i in 0..self.windows.len() {
                let window = &self.windows[i];
                let focused = i == self.focused;
                let border_fg = if focused {
                    self.config.theme.focused_border.to_crossterm()
                } else {
                    self.config.theme.unfocused_border.to_crossterm()
                };

                frame::draw_frame(
                    &mut self.back_buf,
                    window.x,
                    window.y,
                    window.w,
                    window.h,
                    if self.config.disable_mouse {
                        None
                    } else {
                        Some(&window.title)
                    },
                    border_fg,
                    rows,
                    cols,
                );

                let screen = window.screen();
                let cx = window.content_x();
                let cy = window.content_y();
                let cw = window.content_w();
                let ch = window.content_h();

                for rel_row in 0..ch {
                    let abs_row = cy + rel_row as i32;
                    if abs_row < 0 || abs_row >= rows as i32 {
                        continue;
                    }
                    for rel_col in 0..cw {
                        let abs_col = cx + rel_col as i32;
                        if abs_col < 0 || abs_col >= cols as i32 {
                            continue;
                        }
                        let cell = if let Some(vt_cell) = screen.cell(rel_row, rel_col) {
                            let ch = if vt_cell.contents().is_empty() {
                                ' '
                            } else {
                                vt_cell.contents().chars().next().unwrap_or(' ')
                            };
                            Cell {
                                ch,
                                fg: cell::vt100_color_to_crossterm(vt_cell.fgcolor()),
                                bg: cell::vt100_color_to_crossterm(vt_cell.bgcolor()),
                            }
                        } else {
                            Cell::default()
                        };

                        self.back_buf[abs_row as usize][abs_col as usize] = cell;
                    }
                }
            }
        }

        let mut stdout = io::stdout();

        if self.force_full {
            self.force_full = false;
            let mut current_fg: Option<crossterm::style::Color> = None;
            let mut current_bg: Option<crossterm::style::Color> = None;
            let mut tracked_pos: Option<(u16, u16)> = None;

            for row in 0..rows as usize {
                let back_row = &self.back_buf[row];
                let mut col = 0;
                // Always MoveTo the start of each row (cursor tracking handles adjacency)
                while col < cols as usize {
                    let target_col = col as u16;
                    let target_row = row as u16;
                    if tracked_pos != Some((target_col, target_row)) {
                        queue!(stdout, cursor::MoveTo(target_col, target_row))?;
                        tracked_pos = Some((target_col, target_row));
                    }
                    let cell = back_row[col];
                    let run_fg = cell.fg;
                    let run_bg = cell.bg;
                    if Some(run_fg) != current_fg {
                        queue!(stdout, SetForegroundColor(run_fg))?;
                        current_fg = Some(run_fg);
                    }
                    if Some(run_bg) != current_bg {
                        queue!(stdout, SetBackgroundColor(run_bg))?;
                        current_bg = Some(run_bg);
                    }
                    let mut chars = String::new();
                    while col < cols as usize
                        && back_row[col].fg == run_fg
                        && back_row[col].bg == run_bg
                    {
                        chars.push(back_row[col].ch);
                        col += 1;
                    }
                    queue!(stdout, Print(&chars))?;
                    if let Some((ref mut tc, _)) = tracked_pos {
                        *tc += chars.chars().count() as u16;
                    }
                }
            }
        } else {
            // Only emit cells that changed from the previous frame.
            let mut current_fg: Option<crossterm::style::Color> = None;
            let mut current_bg: Option<crossterm::style::Color> = None;
            let mut tracked_pos: Option<(u16, u16)> = None;

            for row in 0..rows as usize {
                let front_row = &self.front_buf[row];
                let back_row = &self.back_buf[row];
                let mut col = 0;
                while col < cols as usize {
                    if back_row[col] != front_row[col] {
                        let target_col = col as u16;
                        let target_row = row as u16;
                        if tracked_pos != Some((target_col, target_row)) {
                            queue!(stdout, cursor::MoveTo(target_col, target_row))?;
                            tracked_pos = Some((target_col, target_row));
                        }
                        while col < cols as usize && back_row[col] != front_row[col] {
                            let cell = back_row[col];
                            let run_fg = cell.fg;
                            let run_bg = cell.bg;
                            if Some(run_fg) != current_fg {
                                queue!(stdout, SetForegroundColor(run_fg))?;
                                current_fg = Some(run_fg);
                            }
                            if Some(run_bg) != current_bg {
                                queue!(stdout, SetBackgroundColor(run_bg))?;
                                current_bg = Some(run_bg);
                            }
                            let mut chars = String::new();
                            while col < cols as usize
                                && back_row[col] != front_row[col]
                                && back_row[col].fg == run_fg
                                && back_row[col].bg == run_bg
                            {
                                chars.push(back_row[col].ch);
                                col += 1;
                            }
                            queue!(stdout, Print(&chars))?;
                            if let Some((ref mut tc, _)) = tracked_pos {
                                *tc += chars.chars().count() as u16;
                            }
                        }
                    } else {
                        col += 1;
                    }
                }
            }
        }

        if let Some(focused) = self.windows.get(self.focused) {
            let screen = focused.screen();
            if screen.hide_cursor() {
                queue!(stdout, cursor::Hide)?;
            } else {
                let (curs_row, curs_col) = screen.cursor_position();
                let abs_row =
                    (focused.content_y() + curs_row as i32).clamp(0, rows as i32 - 1) as u16;
                let abs_col =
                    (focused.content_x() + curs_col as i32).clamp(0, cols as i32 - 1) as u16;
                queue!(stdout, cursor::MoveTo(abs_col, abs_row), cursor::Show)?;
            }
        } else {
            queue!(stdout, cursor::Hide)?;
        }
        stdout.flush()?;

        std::mem::swap(&mut self.front_buf, &mut self.back_buf);

        Ok(())
    }
}
