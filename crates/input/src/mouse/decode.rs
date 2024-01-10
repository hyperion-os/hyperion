use core::sync::atomic::{AtomicU8, Ordering};

use heapless::Vec;

use super::event::{Button, MouseEvent};
use crate::keyboard::event::ElementState;

//

pub(crate) fn process(ps2_byte: u8) -> impl Iterator<Item = MouseEvent> {
    let mut events: Vec<MouseEvent, 4> = Vec::new();

    match NEXT.load(Ordering::Acquire) {
        CMD => {
            DATA.0.store(ps2_byte, Ordering::Release);

            _ = NEXT.compare_exchange(CMD, X, Ordering::Release, Ordering::Relaxed);
        }
        X => {
            DATA.1.store(ps2_byte, Ordering::Release);

            _ = NEXT.compare_exchange(X, Y, Ordering::Release, Ordering::Relaxed);
        }
        Y => {
            let cmd = DATA.0.load(Ordering::Acquire);
            let x = DATA.1.load(Ordering::Acquire);
            let y = ps2_byte;

            if NEXT
                .compare_exchange(Y, CMD, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                decode_bytes(&mut events, [cmd, x, y]);
            }
        }
        _ => unreachable!(),
    };

    events.into_iter()
}

// TODO: 4th byte
fn decode_bytes(events: &mut Vec<MouseEvent, 4>, bytes: [u8; 3]) {
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
        _ = events.push(MouseEvent::Button {
            button: Button::Left,
            state,
        });
    }

    if diff.contains(Byte1::MIDDLE_BTN) {
        let state = change_to_state(cmd.contains(Byte1::MIDDLE_BTN));
        _ = events.push(MouseEvent::Button {
            button: Button::Middle,
            state,
        });
    }

    if diff.contains(Byte1::RIGHT_BTN) {
        let state = change_to_state(cmd.contains(Byte1::RIGHT_BTN));
        _ = events.push(MouseEvent::Button {
            button: Button::Right,
            state,
        });
    }

    let x_sign = cmd.contains(Byte1::X_SIGN);
    let y_sign = cmd.contains(Byte1::Y_SIGN);
    let x = (x | if x_sign { 0xFF00 } else { 0 }) as i16;
    let y = (y | if y_sign { 0xFF00 } else { 0 }) as i16;

    if x != 0 || y != 0 {
        _ = events.push(MouseEvent::Motion { delta: (x, y) });
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

const CMD: u8 = 0;
const X: u8 = 1;
const Y: u8 = 2;

static NEXT: AtomicU8 = AtomicU8::new(X);
static LAST_CMD: AtomicU8 = AtomicU8::new(0);
static DATA: (AtomicU8, AtomicU8) = (AtomicU8::new(0), AtomicU8::new(0));

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
