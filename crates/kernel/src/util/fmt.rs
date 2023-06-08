use core::{fmt, ops::DivAssign};

//

pub trait NumberPostfix: Sized + Copy + DivAssign + PartialOrd {
    const NUM_1000: Self;
    const NUM_1024: Self;

    fn postfix(mut self) -> NumberPostfixed<Self> {
        const TABLE: [&str; 10] = ["", "K", "M", "G", "T", "P", "E", "Z", "Y", "R"];
        for scale in TABLE {
            if self < Self::NUM_1000 {
                return NumberPostfixed { n: self, scale };
            }
            self /= Self::NUM_1000;
        }
        NumberPostfixed {
            n: self,
            scale: "Q",
        }
    }

    fn postfix_binary(mut self) -> NumberPostfixed<Self> {
        const TABLE: [&str; 10] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi", "Ri"];
        for scale in TABLE {
            if self < Self::NUM_1024 {
                return NumberPostfixed { n: self, scale };
            }
            self /= Self::NUM_1024;
        }
        NumberPostfixed {
            n: self,
            scale: "Qi",
        }
    }
}

//

#[derive(Debug)]
pub struct NumberPostfixed<T> {
    n: T,
    scale: &'static str,
}

//

impl NumberPostfix for f32 {
    const NUM_1000: Self = 1000.0;
    const NUM_1024: Self = 1024.0;
}

impl NumberPostfix for f64 {
    const NUM_1000: Self = 1000.0;
    const NUM_1024: Self = 1024.0;
}

impl NumberPostfix for u16 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
}

impl NumberPostfix for u32 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
}

impl NumberPostfix for u64 {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
}

impl NumberPostfix for usize {
    const NUM_1000: Self = 1000;
    const NUM_1024: Self = 1024;
}

impl<T> fmt::Display for NumberPostfixed<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.n, self.scale)
    }
}
