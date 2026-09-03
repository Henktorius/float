//! Native Linux virtual-console mouse support via `libgpm`.
//!
//! Crossterm delivers mouse input by parsing xterm escape sequences, which a
//! bare Linux console (`TERM=linux`) never emits. On such a console Float
//! connects to the running `gpm` daemon instead. `libgpm` is loaded with
//! `dlopen` at runtime so the crate keeps no build- or link-time dependency on
//! it: if the shared library or the daemon is missing, [`Gpm::open`] returns
//! `None` and Float falls back to crossterm mouse capture.

use std::ffi::{CStr, c_void};
use std::os::raw::c_int;

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// `struct Gpm_Connect` from `gpm.h` (size 16, checked against the header).
#[repr(C)]
struct GpmConnect {
    event_mask: u16,
    default_mask: u16,
    min_mod: u16,
    max_mod: u16,
    pid: c_int,
    vc: c_int,
}

/// `struct Gpm_Event` from `gpm.h` (size 28, checked against the header).
#[repr(C)]
#[derive(Default)]
struct GpmEvent {
    buttons: u8,
    modifiers: u8,
    vc: u16,
    dx: i16,
    dy: i16,
    x: i16,
    y: i16,
    etype: c_int,
    clicks: c_int,
    margin: c_int,
    wdx: i16,
    wdy: i16,
}

type GpmOpenFn = unsafe extern "C" fn(*mut GpmConnect, c_int) -> c_int;
type GpmCloseFn = unsafe extern "C" fn() -> c_int;
type GpmGetEventFn = unsafe extern "C" fn(*mut GpmEvent) -> c_int;

// enum Gpm_Etype
const GPM_MOVE: c_int = 1;
const GPM_DRAG: c_int = 2;
const GPM_DOWN: c_int = 4;
const GPM_UP: c_int = 8;

// button bits in Gpm_Event.buttons
const GPM_B_RIGHT: u8 = 1;
const GPM_B_MIDDLE: u8 = 2;
const GPM_B_LEFT: u8 = 4;

/// A dynamically loaded `libgpm` plus the entry points we use.
struct Lib {
    handle: *mut c_void,
    open: GpmOpenFn,
    get_event: GpmGetEventFn,
    close: GpmCloseFn,
}

impl Lib {
    /// `dlopen` libgpm and resolve the symbols. `None` if the library is not
    /// installed or is missing an expected symbol.
    fn load() -> Option<Lib> {
        unsafe {
            let handle = [c"libgpm.so.2".as_ptr(), c"libgpm.so".as_ptr()]
                .into_iter()
                .map(|name| libc::dlopen(name, libc::RTLD_NOW))
                .find(|h| !h.is_null())?;

            let sym = |name: &CStr| {
                let p = libc::dlsym(handle, name.as_ptr());
                (!p.is_null()).then_some(p)
            };
            match (sym(c"Gpm_Open"), sym(c"Gpm_GetEvent"), sym(c"Gpm_Close")) {
                (Some(o), Some(g), Some(c)) => Some(Lib {
                    handle,
                    open: std::mem::transmute::<*mut c_void, GpmOpenFn>(o),
                    get_event: std::mem::transmute::<*mut c_void, GpmGetEventFn>(g),
                    close: std::mem::transmute::<*mut c_void, GpmCloseFn>(c),
                }),
                _ => {
                    libc::dlclose(handle);
                    None
                }
            }
        }
    }
}

/// The gpm connect parameters Midnight Commander uses on the console: forward
/// every button and motion event to us, but let gpm keep doing its own
/// low-level processing for anything we do not claim.
fn connect_params() -> GpmConnect {
    GpmConnect {
        event_mask: 0xffff,
        default_mask: GPM_MOVE as u16,
        min_mod: 0,
        max_mod: 0xffff,
        pid: 0,
        vc: 0, // 0 => the console backing our stdin
    }
}

pub struct Gpm {
    lib: Lib,
    fd: c_int,
    /// Button currently held down, so `Drag`/`Up` events (where gpm may not
    /// repeat the button bits) still name the right button.
    held: Option<MouseButton>,
}

impl Gpm {
    /// Connect to `gpm` when running on a Linux virtual console and the daemon
    /// is reachable. Returns `None` in every other case, including a missing
    /// `libgpm`, so the caller can fall back to crossterm mouse capture.
    pub fn open() -> Option<Self> {
        let term = std::env::var("TERM").unwrap_or_default();
        // A real virtual console reports `linux` (or `linux-16color`, etc.).
        // Anything else is an emulator where crossterm handles the mouse and
        // where `Gpm_Open` would only scribble xterm sequences onto stdout.
        let is_console = term == "linux" || term.starts_with("linux-") || term.is_empty();
        if !is_console || !std::path::Path::new("/dev/gpmctl").exists() {
            return None;
        }

        let lib = Lib::load()?;
        let mut conn = connect_params();
        let fd = unsafe { (lib.open)(&mut conn, 0) };
        if fd < 0 {
            unsafe { libc::dlclose(lib.handle) };
            return None;
        }

        // The fd stays blocking, like mc: `ready()` polls before every read, so
        // `Gpm_GetEvent` only runs with a whole event already waiting.
        Some(Self {
            lib,
            fd,
            held: None,
        })
    }

    /// Drain every queued gpm event, converted to crossterm's representation so
    /// the window manager handles it exactly like emulator mouse input.
    pub fn drain_events(&mut self) -> Vec<MouseEvent> {
        let mut out = Vec::new();
        while self.ready() {
            let mut ev = GpmEvent::default();
            if unsafe { (self.lib.get_event)(&mut ev) } <= 0 {
                break;
            }
            if let Some(m) = translate(&ev, &mut self.held) {
                out.push(m);
            }
        }
        out
    }

    /// Non-blocking check for pending data on the gpm socket.
    fn ready(&self) -> bool {
        let mut pfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let r = unsafe { libc::poll(&mut pfd, 1, 0) };
        r > 0 && pfd.revents & libc::POLLIN != 0
    }
}

impl Drop for Gpm {
    fn drop(&mut self) {
        unsafe {
            (self.lib.close)();
            libc::dlclose(self.lib.handle);
        }
    }
}

fn translate(ev: &GpmEvent, held: &mut Option<MouseButton>) -> Option<MouseEvent> {
    // gpm reports 1-based console coordinates; crossterm wants 0-based.
    let column = ev.x.max(1).wrapping_sub(1) as u16;
    let row = ev.y.max(1).wrapping_sub(1) as u16;
    let modifiers = mods(ev.modifiers);

    if ev.wdy != 0 {
        let kind = if ev.wdy > 0 {
            MouseEventKind::ScrollUp
        } else {
            MouseEventKind::ScrollDown
        };
        return Some(MouseEvent {
            kind,
            column,
            row,
            modifiers,
        });
    }

    let button = |bits: u8| {
        if bits & GPM_B_LEFT != 0 {
            Some(MouseButton::Left)
        } else if bits & GPM_B_MIDDLE != 0 {
            Some(MouseButton::Middle)
        } else if bits & GPM_B_RIGHT != 0 {
            Some(MouseButton::Right)
        } else {
            None
        }
    };

    let kind = if ev.etype & GPM_DOWN != 0 {
        let b = button(ev.buttons)?;
        *held = Some(b);
        MouseEventKind::Down(b)
    } else if ev.etype & GPM_UP != 0 {
        let b = button(ev.buttons)
            .or(held.take())
            .unwrap_or(MouseButton::Left);
        MouseEventKind::Up(b)
    } else if ev.etype & GPM_DRAG != 0 {
        match button(ev.buttons).or(*held) {
            Some(b) => MouseEventKind::Drag(b),
            None => MouseEventKind::Moved,
        }
    } else if ev.etype & GPM_MOVE != 0 {
        MouseEventKind::Moved
    } else {
        return None;
    };

    Some(MouseEvent {
        kind,
        column,
        row,
        modifiers,
    })
}

/// Map the Linux keyboard shift bitmask to crossterm modifiers
/// (shift = `1<<0`, ctrl = `1<<2`, alt = `1<<3`).
fn mods(bits: u8) -> KeyModifiers {
    let mut m = KeyModifiers::empty();
    if bits & 0b0001 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if bits & 0b0100 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    if bits & 0b1000 != 0 {
        m |= KeyModifiers::ALT;
    }
    m
}
