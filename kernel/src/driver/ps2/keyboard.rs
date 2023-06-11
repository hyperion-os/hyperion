use core::sync::atomic::{AtomicBool, Ordering};

use hyperion_keyboard::provide_keyboard_event;
use pc_keyboard::{
    layouts::{AnyLayout, DVP104Key, De105Key, Dvorak104Key, Uk105Key, Us104Key},
    DecodedKey, HandleControl, KeyCode, KeyEvent, Keyboard, ScancodeSet1,
};
use spin::Mutex;

//

pub fn process(scancode: u8) -> Option<char> {
    let mut kb = KEYBOARD.lock();

    DEBUG_KEY.store(false, Ordering::SeqCst);

    kb.add_byte(scancode)
        .ok()
        .flatten()
        .and_then(|ev: KeyEvent| kb.process_keyevent(ev))
        .and_then(|key| match key {
            DecodedKey::Unicode(ch) => Some(ch),
            DecodedKey::RawKey(KeyCode::Home) => {
                DEBUG_KEY.store(true, Ordering::SeqCst);
                None
            }
            DecodedKey::RawKey(_key) => None,
        })
        .map(|c| {
            provide_keyboard_event(c);
            c
        })
}

pub fn set_layout(name: &str) -> Option<()> {
    let layout = match name {
        "us" => AnyLayout::Us104Key(Us104Key),
        "uk" => AnyLayout::Uk105Key(Uk105Key),
        "de" => AnyLayout::De105Key(De105Key),
        "dvorak" => AnyLayout::Dvorak104Key(Dvorak104Key),
        "dvp" => AnyLayout::DVP104Key(DVP104Key),
        _ => return None,
    };

    *KEYBOARD.lock() = Keyboard::new(ScancodeSet1::new(), layout, HandleControl::Ignore);
    Some(())
}

pub fn debug_key() -> bool {
    DEBUG_KEY.load(Ordering::SeqCst)
}

static DEBUG_KEY: AtomicBool = AtomicBool::new(false);

static KEYBOARD: Mutex<Keyboard<AnyLayout, ScancodeSet1>> = Mutex::new(Keyboard::new(
    ScancodeSet1::new(),
    AnyLayout::Us104Key(Us104Key),
    HandleControl::Ignore,
));
