use super::{decode, event::MouseEvent};
use crate::mpmc::{EventQueue, Recv};

//

pub fn send_raw(ps2_byte: u8, _ip: usize) {
    // TODO: process in userland at some point
    // TODO: device id
    for event in decode::process(ps2_byte) {
        send(event);
    }
}

pub fn send(event: MouseEvent) {
    BUF.send(event);
}

pub fn try_recv() -> Option<MouseEvent> {
    BUF.try_recv()
}

pub fn recv() -> Recv<'static, MouseEvent> {
    BUF.recv()
}

//

static BUF: EventQueue<MouseEvent> = EventQueue::new();
