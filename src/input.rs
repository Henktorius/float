use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Encode a key event as the bytes a child pty program expects (xterm style).
///
/// `app_cursor` picks the SS3 form (`ESC O x`) for the cursor keys and
/// Home/End when the program has enabled DECCKM (application cursor keys).
pub fn encode_key(key: KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let m = key.modifiers;

    // xterm modifier parameter: 1 + shift + alt*2 + ctrl*4.
    let mod_param = 1
        + u8::from(m.contains(KeyModifiers::SHIFT))
        + u8::from(m.contains(KeyModifiers::ALT)) * 2
        + u8::from(m.contains(KeyModifiers::CONTROL)) * 4;

    // `ESC [ <p> ; <mod> <fin>`, omitting the parameters that default.
    let seq = |p: &str, fin: char| -> Vec<u8> {
        match (p, mod_param) {
            ("", 1) => format!("\x1b[{fin}"),
            ("", _) => format!("\x1b[1;{mod_param}{fin}"),
            (p, 1) => format!("\x1b[{p}{fin}"),
            (p, _) => format!("\x1b[{p};{mod_param}{fin}"),
        }
        .into_bytes()
    };
    // Cursor / Home / End: SS3 form only when unmodified and in app-cursor mode.
    let cursor = |fin: char| -> Vec<u8> {
        if app_cursor && mod_param == 1 {
            format!("\x1bO{fin}").into_bytes()
        } else {
            seq("", fin)
        }
    };
    // F1-F4: SS3 when unmodified, else `CSI 1 ; mod P..S`.
    let pf = |fin: char| -> Vec<u8> {
        if mod_param == 1 {
            format!("\x1bO{fin}").into_bytes()
        } else {
            format!("\x1b[1;{mod_param}{fin}").into_bytes()
        }
    };

    let bytes = match key.code {
        KeyCode::Char(c) => {
            if m.contains(KeyModifiers::CONTROL) {
                return ctrl_byte(c).map(|b| vec![b]);
            }
            let mut buf = [0u8; 4];
            c.encode_utf8(&mut buf).as_bytes().to_vec()
        }
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => {
            if m.contains(KeyModifiers::CONTROL) {
                vec![0x08]
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Esc => vec![0x1b],

        KeyCode::Up => cursor('A'),
        KeyCode::Down => cursor('B'),
        KeyCode::Right => cursor('C'),
        KeyCode::Left => cursor('D'),
        KeyCode::Home => cursor('H'),
        KeyCode::End => cursor('F'),

        KeyCode::Insert => seq("2", '~'),
        KeyCode::Delete => seq("3", '~'),
        KeyCode::PageUp => seq("5", '~'),
        KeyCode::PageDown => seq("6", '~'),

        KeyCode::F(1) => pf('P'),
        KeyCode::F(2) => pf('Q'),
        KeyCode::F(3) => pf('R'),
        KeyCode::F(4) => pf('S'),
        KeyCode::F(5) => seq("15", '~'),
        KeyCode::F(6) => seq("17", '~'),
        KeyCode::F(7) => seq("18", '~'),
        KeyCode::F(8) => seq("19", '~'),
        KeyCode::F(9) => seq("20", '~'),
        KeyCode::F(10) => seq("21", '~'),
        KeyCode::F(11) => seq("23", '~'),
        KeyCode::F(12) => seq("24", '~'),

        _ => return None,
    };
    Some(bytes)
}

/// The control byte for `Ctrl` + `c`, or `None` if the combination has none.
fn ctrl_byte(c: char) -> Option<u8> {
    let u = c.to_ascii_uppercase();
    match u {
        'A'..='Z' => Some(u as u8 - b'A' + 1),
        ' ' | '@' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' | '/' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}
