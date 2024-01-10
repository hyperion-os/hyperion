use alloc::collections::VecDeque;
use core::fmt::Write;

use futures_util::{stream::select, StreamExt};
use hyperion_futures::{keyboard::keyboard_events, timer::ticks};
use hyperion_input::keyboard::event::{ElementState, KeyCode, KeyboardEvent};
use hyperion_random::Rng;
use time::Duration;

use crate::{term::Term, Result};

//

pub async fn snake_game(term: &mut Term) -> Result<()> {
    let save_cursor = term.cursor;

    term.cursor.0 = term.size.0 / 2;
    term.cursor.1 = term.size.1 / 2;

    // let color_snake = color_to_code(Color::from_hex("#dadada").unwrap());
    // let color_back = color_to_code(Color::BLACK);

    let mut last_dir = Direction::Up;
    let mut events = keyboard_events().map(Some);
    let mut pieces = VecDeque::from_iter([term.cursor]);
    let mut rng = hyperion_random::next_fast_rng();

    loop {
        let undo_cursor = term.cursor;
        term.write_byte(b'#');
        term.cursor = undo_cursor;
        term.flush();

        let Some(dir) = snake_next_dir(&mut last_dir, &mut events).await else {
            break;
        };

        let old_pos = term.cursor;
        match dir {
            Direction::Up => {
                term.cursor.1 = term.cursor.1.saturating_sub(1);
            }
            Direction::Down => {
                term.cursor.1 = term.cursor.1.saturating_add(1);
                if term.cursor.1 >= term.size.1 {
                    term.cursor.1 = term.size.1 - 1;
                }
            }
            Direction::Left => {
                term.cursor.0 = term.cursor.0.saturating_sub(1);
            }
            Direction::Right => {
                term.cursor.0 = term.cursor.0.saturating_add(1);
                if term.cursor.0 >= term.size.0 {
                    term.cursor.0 = term.size.0 - 1;
                }
            }
        }

        let ate = term.read_at(term.cursor);
        if ate == b' ' {
            // remove the tail if nothing was eaten
            if let Some(tail) = pieces.pop_front() {
                let undo_cursor = term.cursor;
                term.cursor = tail;
                term.write_byte(b' ');
                term.cursor = undo_cursor;
            }
        } else {
            let undo_cursor = term.cursor;
            term.cursor = (rng.gen_range(0..term.size.0), rng.gen_range(0..term.size.1));
            term.write_byte(ate);
            term.cursor = undo_cursor;
        }

        let hit_self = pieces.iter().any(|piece| *piece == term.cursor);

        pieces.push_back(term.cursor);

        if old_pos == term.cursor || hit_self {
            // crashed to a wall

            term.cursor.0 = term.size.0 / 2;
            term.cursor.1 = term.size.1 / 2;
            term.write_bytes(b"GAME OVER\n");
            term.cursor.0 = term.size.0 / 2;
            _ = write!(term, "SCORE: {}", pieces.len());
            break;
        }
    }

    term.cursor = save_cursor;

    Ok(())
}

async fn snake_next_dir(
    last: &mut Direction,
    stream: &mut (impl StreamExt<Item = Option<KeyboardEvent>> + Unpin),
) -> Option<Direction> {
    use Direction::*;
    use KeyCode::*;

    let ticks = ticks(Duration::milliseconds(500)).map(|_| None);
    let mut events = select(ticks, stream);

    let dir = loop {
        break match events.next().await {
            Some(Some(KeyboardEvent {
                state: ElementState::Released,
                ..
            })) => continue,
            Some(Some(KeyboardEvent { keycode, .. })) => {
                let dir = match keycode {
                    ArrowUp => Up,
                    ArrowDown => Down,
                    ArrowLeft => Left,
                    ArrowRight => Right,
                    Escape => return None,
                    _ => continue,
                };

                if dir.opposite() == *last {
                    continue;
                }

                dir
            }
            Some(_) => *last, // tick event
            None => return None,
        };
    };
    *last = dir;

    Some(dir)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    const fn opposite(self) -> Self {
        use Direction::*;
        match self {
            Up => Down,
            Down => Up,
            Left => Right,
            Right => Left,
        }
    }
}
