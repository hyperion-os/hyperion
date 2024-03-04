use std::{
    fs::File,
    io::Read,
    sync::atomic::{AtomicU8, Ordering},
};

use heapless::Vec;
use hyperion_windowing::shared::{Button, ElementState, Event, Mouse};

use crate::EVENTS;

//

pub fn mouse() {
    let mut mouse_dev = File::open("/dev/mouse").unwrap();

    let mut buf = [0u8; 3];

    loop {
        let n = mouse_dev.read(&mut buf).unwrap();
        if n != 3 {
            panic!()
        }

        for ev in process(buf) {
            EVENTS.0.send(Event::Mouse(ev)).unwrap();
        }
    }
}

fn process(ev: [u8; 3]) -> impl Iterator<Item = Mouse> {
    let mut events = Vec::new();
    decode_bytes(&mut events, ev);
    events.into_iter()
}

// TODO: 4th byte
fn decode_bytes(events: &mut Vec<Mouse, 4>, bytes: [u8; 3]) {
    let cmd = Byte1::from_bits_truncate(bytes[0]);
    let x: u16 = bytes[1] as _;
    let y: u16 = bytes[2] as _;

    if cmd.contains(Byte1::X_OVERFLOW | Byte1::Y_OVERFLOW) || !cmd.contains(Byte1::ONE) {
        return;
    }

    let last_cmd = Byte1::from_bits_truncate(LAST_CMD.swap(cmd.bits(), Ordering::Acquire));
    let diff = cmd.symmetric_difference(last_cmd);

    if diff.contains(Byte1::LEFT_BTN) {
        let state = change_to_state(cmd.contains(Byte1::LEFT_BTN));
        _ = events.push(Mouse::Button {
            btn: Button::Left,
            state,
        });
    }

    if diff.contains(Byte1::MIDDLE_BTN) {
        let state = change_to_state(cmd.contains(Byte1::MIDDLE_BTN));
        _ = events.push(Mouse::Button {
            btn: Button::Middle,
            state,
        });
    }

    if diff.contains(Byte1::RIGHT_BTN) {
        let state = change_to_state(cmd.contains(Byte1::RIGHT_BTN));
        _ = events.push(Mouse::Button {
            btn: Button::Right,
            state,
        });
    }

    let x_sign = cmd.contains(Byte1::X_SIGN);
    let y_sign = cmd.contains(Byte1::Y_SIGN);
    let x = (x | if x_sign { 0xFF00 } else { 0 }) as i16;
    let y = (y | if y_sign { 0xFF00 } else { 0 }) as i16;

    if x != 0 || y != 0 {
        _ = events.push(Mouse::Motion {
            x: x as f32,
            y: y as f32,
        });
    }
}

fn change_to_state(change: bool) -> ElementState {
    if change {
        ElementState::Pressed
    } else {
        ElementState::Released
    }
}

//

static LAST_CMD: AtomicU8 = AtomicU8::new(0);

//

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Byte1: u8 {
        const LEFT_BTN   = 0b0000_0001;
        const RIGHT_BTN  = 0b0000_0010;
        const MIDDLE_BTN = 0b0000_0100;
        const ONE        = 0b0000_1000; // always one
        const X_SIGN     = 0b0001_0000;
        const Y_SIGN     = 0b0010_0000;
        const X_OVERFLOW = 0b0100_0000; // discard the packet
        const Y_OVERFLOW = 0b1000_0000; // discard the packet
    }
}
