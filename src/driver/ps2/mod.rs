use crate::debug;
use pc_keyboard::{layouts::Us104Key, DecodedKey, HandleControl, KeyEvent, Keyboard, ScancodeSet1};
use spin::Mutex;

//

pub fn process(scancode: u8) -> Option<char> {
    static KEYBOARD: Mutex<Keyboard<Us104Key, ScancodeSet1>> = Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        Us104Key,
        HandleControl::Ignore,
    ));

    let mut kb = KEYBOARD.lock();

    kb.add_byte(scancode)
        .ok()
        .flatten()
        .and_then(|ev: KeyEvent| kb.process_keyevent(ev))
        .and_then(|key| match key {
            DecodedKey::Unicode(ch) => Some(ch),
            DecodedKey::RawKey(key) => {
                debug!("{key:?}");
                None
            }
        })
}