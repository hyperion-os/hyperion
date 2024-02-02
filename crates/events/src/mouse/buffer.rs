use super::{decode, event::MouseEvent};
use crate::mpmc::{EventQueue, Recv};

//

pub fn send_raw(ps2_byte: u8, _ip: usize) {
    // TODO: process in userland at some point
    // TODO: device id
    let Some(raw) = decode::unpack(ps2_byte) else {
        return;
    };

    RAW_BUF.send(raw);

    for event in decode::process(raw) {
        send(event);
    }
}

pub fn send(event: MouseEvent) {
    BUF.send(event);
}

pub fn try_recv_raw() -> Option<[u8; 3]> {
    RAW_BUF.try_recv()
}

pub fn recv_raw() -> Recv<'static, [u8; 3]> {
    RAW_BUF.recv()
}

pub fn try_recv() -> Option<MouseEvent> {
    BUF.try_recv()
}

pub fn recv() -> Recv<'static, MouseEvent> {
    BUF.recv()
}

//

static BUF: EventQueue<MouseEvent> = EventQueue::new();
static RAW_BUF: EventQueue<[u8; 3]> = EventQueue::new();
