use pc_keyboard::{
    layouts::{AnyLayout, DVP104Key, De105Key, Dvorak104Key, FiSe105Key, Uk105Key, Us104Key},
    DecodedKey, HandleControl, KeyCode, KeyState, Keyboard, ScancodeSet1,
};
use spin::Mutex;

use super::event::{ElementState, KeyboardEvent};

//

pub(crate) fn process(ps2_byte: u8) -> impl Iterator<Item = KeyboardEvent> {
    let mut kb = KEYBOARD.lock();

    let Some(mut event) = kb.add_byte(ps2_byte).ok().flatten() else {
        return [KeyboardEvent::empty(); 2].into_iter().take(0);
    };

    if event.code == KeyCode::Oem7 {
        event.code = KeyCode::Oem5; // idk, '\' / '|' key isn't working
    };

    let keycode = event.code;
    let state = event.state;
    let key = kb.process_keyevent(event);

    let unicode = match key {
        Some(DecodedKey::Unicode(c)) => Some(c),
        _ => None,
    };

    match state {
        KeyState::Up => [
            KeyboardEvent {
                state: ElementState::Released,
                keycode,
                unicode,
            },
            KeyboardEvent::empty(),
        ]
        .into_iter()
        .take(1),
        KeyState::Down => [
            KeyboardEvent {
                state: ElementState::Pressed,
                keycode,
                unicode,
            },
            KeyboardEvent::empty(),
        ]
        .into_iter()
        .take(1),
        KeyState::SingleShot => [
            KeyboardEvent {
                state: ElementState::Pressed,
                keycode,
                unicode,
            },
            KeyboardEvent {
                state: ElementState::Released,
                keycode,
                unicode: None,
            },
        ]
        .into_iter()
        .take(2),
    }
}

pub fn set_layout(name: &str) -> Option<()> {
    let layout = match name {
        "us" => AnyLayout::Us104Key(Us104Key),
        "uk" => AnyLayout::Uk105Key(Uk105Key),
        "de" => AnyLayout::De105Key(De105Key),
        "fi" | "se" => AnyLayout::FiSe105Key(FiSe105Key),
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
