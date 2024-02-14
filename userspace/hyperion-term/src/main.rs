use hyperion_color::Color;
use hyperion_syscall::get_pid;
use hyperion_windowing::{client::Connection, shared::Event};

//

fn main() {
    let wm = Connection::new().unwrap();

    let mut window = wm.new_window().unwrap();

    let colors = [Color::RED, Color::GREEN, Color::BLUE];
    let mut i = get_pid();

    // let mut t = timestamp().unwrap() as u64;
    loop {
        window.fill(colors[i % 3].as_u32());

        match wm.next_event() {
            Event::Keyboard {
                code: 88 | 101, // up or left
                state: 1,
            } => i = i.wrapping_add(1),
            Event::Keyboard {
                code: 102 | 103, // down or right
                state: 1,
            } => i = i.wrapping_sub(1),
            _ => {}
        }

        // t += 16_666_667;
        // nanosleep_until(t);
    }
}
