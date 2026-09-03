mod cell;
mod compositor;
mod config;
mod escape;
mod frame;
mod gpm;
mod input;
mod pty;
mod window;
mod wm;

use std::io;
use std::time::Duration;

use crossterm::{
    cursor, event, execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, enable_raw_mode},
};

use wm::WindowManager;

fn main() -> anyhow::Result<()> {
    let config = config::load();
    let (cols, rows) = terminal::size()?;
    let mut wm = WindowManager::new(config, cols, rows)?;
    let mut stdout = io::stdout();

    // Connect to gpm before touching terminal state: on a bare Linux virtual
    // console crossterm never sees the mouse (the console emits no xterm mouse
    // sequences), so talk to the gpm daemon directly. Anywhere else, fall back
    // to crossterm's mouse capture.
    let disable_mouse = wm.config.disable_mouse;
    let gpm = if disable_mouse {
        None
    } else {
        gpm::Gpm::open()
    };
    let mouse_capture = !disable_mouse && gpm.is_none();
    if gpm.is_some() {
        // The console draws no pointer once gpm hands the mouse to us.
        wm.enable_software_cursor();
    }

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide,)?;
    if mouse_capture {
        execute!(stdout, event::EnableMouseCapture)?;
    }

    let restore = || {
        let mut out = io::stdout();
        let _ = execute!(out, cursor::Show, LeaveAlternateScreen,);
        if mouse_capture {
            let _ = execute!(out, event::DisableMouseCapture);
        }
        let _ = terminal::disable_raw_mode();
    };

    let result = run_event_loop(&mut wm, gpm);

    restore();
    result
}

fn run_event_loop(wm: &mut WindowManager, mut gpm: Option<gpm::Gpm>) -> anyhow::Result<()> {
    loop {
        // Flush stale Esc if the timeout elapsed without a follow-up key
        wm.expire_esc();

        wm.process_all();
        if wm.is_quit_requested() {
            return Ok(());
        }

        if wm.is_dirty() {
            wm.composite()?;
            wm.clear_dirty();
        }

        wm.reap_dead_windows()?;

        if let Some(g) = gpm.as_mut() {
            for mouse in g.drain_events() {
                wm.handle_mouse(mouse)?;
            }
        }

        if event::poll(Duration::from_millis(wm.config.poll_interval_ms))? {
            match event::read()? {
                event::Event::Key(key) => wm.handle_key(key)?,
                event::Event::Mouse(mouse) => wm.handle_mouse(mouse)?,
                event::Event::Resize(new_cols, new_rows) => {
                    wm.resize_screen(new_rows, new_cols);
                }
                _ => {}
            }
        }
    }
}
