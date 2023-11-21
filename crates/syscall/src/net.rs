#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SocketDesc(pub usize);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SocketDomain(pub usize);

impl SocketDomain {
    pub const UNIX: Self = Self::LOCAL;
    pub const LOCAL: Self = Self(0);
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SocketType(pub usize);

impl SocketType {
    pub const STREAM: Self = Self(0);
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Protocol(pub usize);

impl Protocol {
    pub const UNIX: Self = Self::LOCAL;
    pub const LOCAL: Self = Self(0);
}

//

// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// pub struct SocketStream(pub usize);
