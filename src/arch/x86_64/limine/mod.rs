use core::fmt::{self, Arguments, Write};
use limine::{LimineTerminalRequest, LimineTerminalResponse};
use spin::{Lazy, Mutex, MutexGuard, Once};

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    *crate::BOOTLOADER.lock() = "Limine";
    crate::kernel_main()
}

//

struct Writer(pub &'static LimineTerminalResponse);

unsafe impl Send for Writer {}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut write = self.0.write().ok_or(fmt::Error)?;

        for term in self.0.terminals() {
            write(term, s);
        }

        Ok(())
    }
}

static TERMINALS: LimineTerminalRequest = LimineTerminalRequest::new(0);
static WRITER: Once<Mutex<Writer>> = Once::new();

fn get() -> Result<MutexGuard<'static, Writer>, fmt::Error> {
    WRITER.try_call_once(|| {
        Ok(Mutex::new(Writer(
            TERMINALS.get_response().get().ok_or(fmt::Error)?,
        )))
    })?;
    WRITER.get().ok_or(fmt::Error).map(|mutex| mutex.lock())
}

fn print(args: Arguments) -> Option<()> {
    Some(())
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    if let Ok(mut writer) = get() {
        _ = writer.write_fmt(args)
    }
}
