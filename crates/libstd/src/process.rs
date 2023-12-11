use core::{convert::Infallible, fmt};

use hyperion_syscall::exit;

use crate::eprintln;

//

#[lang = "termination"]
pub trait Termination {
    fn report(self) -> ExitCode;
}

impl Termination for () {
    fn report(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}

impl Termination for ! {
    fn report(self) -> ExitCode {
        self
    }
}

impl Termination for Infallible {
    fn report(self) -> ExitCode {
        match self {}
    }
}

impl Termination for ExitCode {
    fn report(self) -> ExitCode {
        self
    }
}

impl<T: Termination, E: fmt::Debug> Termination for Result<T, E> {
    fn report(self) -> ExitCode {
        match self {
            Ok(val) => val.report(),
            Err(err) => {
                eprintln!("Error: {err:?}");
                ExitCode::FAILURE
            }
        }
    }
}

//

pub struct ExitCode(i32);

impl ExitCode {
    pub const SUCCESS: Self = Self(0);

    pub const FAILURE: Self = Self(-1);

    pub fn from_i32(i: i32) -> Self {
        Self(i)
    }

    pub fn to_i32(self) -> i32 {
        self.0
    }

    pub fn exit_process(self) -> ! {
        exit(self.to_i32() as _);
    }
}

impl From<u8> for ExitCode {
    fn from(value: u8) -> Self {
        ExitCode(value as _)
    }
}

impl Default for ExitCode {
    fn default() -> Self {
        Self::SUCCESS
    }
}
