use core::slice;

use hyperion_framebuffer::framebuffer::{Framebuffer, FramebufferInfo};
use hyperion_syscall::get_pid;
use hyperion_windowing::{
    client::Connection,
    shared::{ElementState, Event},
};

//

fn main() {
    let wm = Connection::new().unwrap();

    let mut window = wm.new_window().unwrap();

    let mut i = get_pid();

    window.buf_base();

    let slice = unsafe {
        slice::from_raw_parts_mut(
            window.buf_base().as_ptr().cast::<u8>(),
            window.pitch * window.height * 4,
        )
    };

    let mut fbo = Framebuffer::new(
        slice,
        FramebufferInfo {
            width: window.width,
            height: window.height,
            pitch: window.pitch * 4,
        },
    );

    hyperion_framebuffer::logger::_print_to(format_args!("Hello, world!\n"), &mut fbo);
    hyperion_framebuffer::logger::_print_to(format_args!("I am PID:{i}\n"), &mut fbo);

    let mut t = hyperion_syscall::timestamp().unwrap() as u64;
    loop {
        // let colors = [Color::RED, Color::GREEN, Color::BLUE];
        // window.fill(colors[i % 3].as_u32());

        match wm.next_event() {
            Event::Keyboard {
                code: 88 | 101, // up or left
                state: ElementState::Pressed,
            } => i = i.wrapping_add(1),
            Event::Keyboard {
                code: 102 | 103, // down or right
                state: ElementState::Pressed,
            } => i = i.wrapping_sub(1),
            Event::Text { ch } => {
                hyperion_framebuffer::logger::_print_to(format_args!("{ch}"), &mut fbo);
            }
            _ => {}
        }

        t += 16_666_667;
        hyperion_syscall::nanosleep_until(t);

        // hyperion_syscall::yield_now();
    }
}
