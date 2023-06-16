use core::sync::atomic::{AtomicBool, Ordering};

use hyperion_driver_acpi::ioapic::IoApic;
use hyperion_keyboard::provide_keyboard_event;
use pc_keyboard::{
    layouts::{AnyLayout, DVP104Key, De105Key, Dvorak104Key, Uk105Key, Us104Key},
    DecodedKey, HandleControl, KeyCode, KeyEvent, Keyboard, ScancodeSet1,
};
use spin::Mutex;
use x86_64::instructions::port::Port;

//

pub fn init() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if ONCE.swap(false, Ordering::SeqCst) {
        if let Some(mut io_apic) = IoApic::any() {
            let irq = hyperion_interrupts::set_any_interrupt_handler(
                |irq| irq >= 0x20,
                || {
                    /* hyperion_log::debug!(
                        "avail?: {}",
                        unsafe { Port::<u8>::new(0x64).read() } & 0b1
                    ); */
                    let scancode: u8 = unsafe { Port::new(0x60).read() };

                    process(scancode);
                },
            )
            .expect("No room for PS/2 keyboard irq");

            io_apic.set_irq_any(1, irq);
            hyperion_log::debug!("PS/2 keyboard irq: {irq}");
        }
    }
}

//

pub fn process(scancode: u8) -> Option<char> {
    let mut kb = KEYBOARD.lock();

    DEBUG_KEY.store(false, Ordering::SeqCst);

    kb.add_byte(scancode)
        .ok()
        .flatten()
        .and_then(|ev: KeyEvent| kb.process_keyevent(ev))
        .and_then(|key| match key {
            DecodedKey::Unicode(ch) => Some(ch),
            DecodedKey::RawKey(KeyCode::Home) => {
                panic!("Panic key")
            }
            DecodedKey::RawKey(_key) => None,
        })
        .map(|c| {
            provide_keyboard_event(c);
            c
        })
}

pub fn set_layout(name: &str) -> Option<()> {
    let layout = match name {
        "us" => AnyLayout::Us104Key(Us104Key),
        "uk" => AnyLayout::Uk105Key(Uk105Key),
        "de" => AnyLayout::De105Key(De105Key),
        "dvorak" => AnyLayout::Dvorak104Key(Dvorak104Key),
        "dvp" => AnyLayout::DVP104Key(DVP104Key),
        _ => return None,
    };

    *KEYBOARD.lock() = Keyboard::new(ScancodeSet1::new(), layout, HandleControl::Ignore);
    Some(())
}

pub fn debug_key() -> bool {
    DEBUG_KEY.load(Ordering::SeqCst)
}

static DEBUG_KEY: AtomicBool = AtomicBool::new(false);

static KEYBOARD: Mutex<Keyboard<AnyLayout, ScancodeSet1>> = Mutex::new(Keyboard::new(
    ScancodeSet1::new(),
    AnyLayout::Us104Key(Us104Key),
    HandleControl::Ignore,
));
