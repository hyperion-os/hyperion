use pc_keyboard::{
    layouts::{AnyLayout, DVP104Key, De105Key, Dvorak104Key, Uk105Key, Us104Key},
    DecodedKey, HandleControl, KeyCode, KeyState, Keyboard, ScancodeSet1,
};
use spin::Mutex;

use crate::event::{ElementState, KeyboardEvent};

//

pub(super) fn process(ps2_byte: u8) -> Option<KeyboardEvent> {
    let mut kb = KEYBOARD.lock();

    let mut event = kb.add_byte(ps2_byte).ok().flatten()?;
    if event.code == KeyCode::Oem7 {
        event.code = KeyCode::Oem5; // idk, '\' / '|' key isn't working
    };

    let state = match event.state {
        KeyState::Up => ElementState::Release,
        KeyState::Down => ElementState::PressHold,
        KeyState::SingleShot => ElementState::PressRelease,
    };
    let keycode = event.code;

    let key = kb.process_keyevent(event);

    let unicode = match key {
        Some(DecodedKey::Unicode(c)) => Some(c),
        _ => None,
    };

    Some(KeyboardEvent {
        state,
        keycode,
        unicode,
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

pub fn layouts() -> &'static [&'static str] {
    &["us", "uk", "de", "dvorak", "dvp"]
}

static KEYBOARD: Mutex<Keyboard<AnyLayout, ScancodeSet1>> = Mutex::new(Keyboard::new(
    ScancodeSet1::new(),
    // AnyLayout::Uk105Key(Uk105Key),
    AnyLayout::Us104Key(Us104Key),
    HandleControl::Ignore,
));
