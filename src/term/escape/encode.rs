use core::fmt;

//

pub trait EscapeEncoder {
    fn with_escape_code<'a>(&'a self, code: &'a str) -> EncodedPart<'a, Self> {
        EncodedPart { code, data: self }
    }

    fn true_red(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;255;0;0m")
    }

    fn true_green(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;0;255;0m")
    }

    fn true_blue(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;0;0;255m")
    }

    fn true_cyan(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;0;255;255m")
    }

    fn true_magenta(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;255;0;255m")
    }

    fn true_yellow(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;255;255;0m")
    }

    fn true_black(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;0;0;0m")
    }

    fn true_white(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;255;255;255m")
    }

    fn true_grey(&self) -> EncodedPart<Self> {
        self.with_escape_code("\x1B[38;2;128;128;128m")
    }
}

pub struct EncodedPart<'a, T: ?Sized> {
    code: &'a str,
    data: &'a T,
}

//

// impl EscapeEncoder for &str {}
//
// impl EscapeEncoder for char {}

impl<T> EscapeEncoder for T {}

impl<'a, T> fmt::Display for EncodedPart<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}\x1B[m", self.code, self.data)
    }
}

impl<'a, T> fmt::Debug for EncodedPart<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)?;
        self.data.fmt(f)?;
        write!(f, "\x1B[m")
    }
}
