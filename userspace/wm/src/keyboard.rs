use std::{fs::File, io::Read};

use hyperion_windowing::shared::{ElementState, Event};
use pc_keyboard::{
    layouts::{AnyLayout, Us104Key},
    DecodedKey, HandleControl, KeyState, Keyboard, ScancodeSet1,
};

use crate::EVENTS;

//

pub fn keyboard() {
    let mut kb_dev = File::open("/dev/keyboard").unwrap();

    let mut buf = [0u8; 64];

    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        AnyLayout::Us104Key(Us104Key),
        HandleControl::Ignore,
    );

    loop {
        let n = kb_dev.read(&mut buf).unwrap();

        // let windows = LazyCell::new(|| windows.lock().unwrap());
        // let windows = LazyCell::new(|| windows.last());

        for byte in &buf[..n] {
            if let Ok(Some(ev)) = keyboard.add_byte(*byte) {
                let code = ev.code as u8;
                if ev.state != KeyState::Up {
                    // down or single shot
                    _ = EVENTS.0.send(Event::Keyboard {
                        code,
                        state: ElementState::Pressed,
                    });
                }
                if ev.state != KeyState::Down {
                    // this is intentionally not an `else if`, single shot presses send both
                    // up or single shot
                    _ = EVENTS.0.send(Event::Keyboard {
                        code,
                        state: ElementState::Released,
                    });
                }
                if let Some(DecodedKey::Unicode(ch)) = keyboard.process_keyevent(ev) {
                    _ = EVENTS.0.send(Event::Text { ch });
                }
            }
        }
    }
}
