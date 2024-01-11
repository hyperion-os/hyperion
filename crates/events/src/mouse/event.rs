use crate::keyboard::event::ElementState;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEvent {
    Motion { delta: (i16, i16) },

    // Scroll { delta: (i16, i16) }, // TODO: init 4th ps2 mouse packet
    Button { button: Button, state: ElementState },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}
