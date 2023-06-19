use snafu::Snafu;

//

#[derive(Debug, Snafu)]
pub enum IoError {
    #[snafu(display("not found"))]
    NotFound,

    #[snafu(display("already exists"))]
    AlreadyExists,

    #[snafu(display("not a directory"))]
    NotADirectory,

    #[snafu(display("is a directory"))]
    IsADirectory,

    #[snafu(display("internal filesystem error"))]
    FilesystemError,

    #[snafu(display("permission denied"))]
    PermissionDenied,

    #[snafu(display("unexpected end of file"))]
    UnexpectedEOF,

    #[snafu(display("interrupted"))]
    Interrupted,

    #[snafu(display("wrote nothing"))]
    WriteZero,
}

pub type IoResult<T> = Result<T, IoError>;

//

impl IoError {
    pub fn msg(&self) -> &'static str {
        match self {
            IoError::NotFound => "not found",
            IoError::AlreadyExists => "already exists",
            IoError::NotADirectory => "not a directory",
            IoError::IsADirectory => "is a directory",
            IoError::FilesystemError => "filesystem error",
            IoError::PermissionDenied => "permission denied",
            IoError::UnexpectedEOF => "unexpected eof",
            IoError::Interrupted => "interrupted",
            IoError::WriteZero => "wrote nothing",
        }
    }
}
