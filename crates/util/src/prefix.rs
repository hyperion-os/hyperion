use core::{
    fmt,
    ops::{DivAssign, Mul},
};

//

pub trait NumberFmt: Sized + Copy + DivAssign + PartialOrd + Mul<Output = Self> {
    const NUM_1000: Self;
    const NUM_1024: Self;
    const NUM_10: Self;

    fn decimal(mut self) -> NumberFormatted<Self> {
        const TABLE: [&str; 10] = ["", "K", "M", "G", "T", "P", "E", "Z", "Y", "R"];
        for scale in TABLE {
            if self < Self::NUM_1000 * Self::NUM_10 {
                return NumberFormatted { n: self, scale };
            }
            self /= Self::NUM_1000;
        }
        NumberFormatted {
            n: self,
            scale: "Q",
        }
    }

    fn binary(mut self) -> NumberFormatted<Self> {
        const TABLE: [&str; 10] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi", "Ri"];
        for scale in TABLE {
            if self < Self::NUM_1024 * Self::NUM_10 {
                return NumberFormatted { n: self, scale };
            }
            self /= Self::NUM_1024;
        }
        NumberFormatted {
            n: self,
            scale: "Qi",
        }
    }
}

//

#[derive(Debug, Clone, Copy)]
pub struct NumberFormatted<T> {
    n: T,
    scale: &'static str,
}

impl<T> NumberFormatted<T> {
    pub fn into_inner(self) -> T {
        self.n
    }

    pub fn scale(self) -> &'static str {
        self.scale
    }
}

//

impl NumberFmt for f32 {
    const NUM_1000: Self = 1000.0;
    const NUM_1024: Self = 1024.0;
    const NUM_10: Self = 10.0;
}

impl NumberFmt for f64 {
    const NUM_1000: Self = 1000.0;
    const NUM_1024: Self = 1024.0;
    const NUM_10: Self = 10.0;
}

impl NumberFmt for u16 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
    const NUM_10: Self = 10;
}

impl NumberFmt for u32 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
    const NUM_10: Self = 10;
}

impl NumberFmt for u64 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
    const NUM_10: Self = 10;
}

impl NumberFmt for usize {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
    const NUM_10: Self = 10;
}

impl<T> fmt::Display for NumberFormatted<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.n, self.scale)
    }
}
