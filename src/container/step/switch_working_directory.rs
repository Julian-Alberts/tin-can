use std::path::PathBuf;

use crate::container::{Step, StepHandle};

pub struct SwitchWorkingDirectory<S>
where
    S: Step,
{
    new_wd: PathBuf,
    next: S,
}

impl<S> SwitchWorkingDirectory<S>
where
    S: Step,
{
    pub fn new(new_wd: PathBuf, next: S) -> Self {
        Self { new_wd, next }
    }
}

impl<S> Step for SwitchWorkingDirectory<S>
where
    S: Step,
{
    type Error = SwitchWorkingDirectoryError<S::Error>;

    type Handle = SwitchWorkingDirectoryHandle<S>;

    fn run(self) -> Result<Self::Handle, Self::Error> {
        std::env::set_current_dir(self.new_wd)?;
        Ok(SwitchWorkingDirectoryHandle(self.next.run()))
    }
}

pub struct SwitchWorkingDirectoryHandle<S>(Result<S::Handle, S::Error>)
where
    S: Step;

impl<S> StepHandle for SwitchWorkingDirectoryHandle<S>
where
    S: Step,
{
    type Error = S::Error;

    type Ok = S::Handle;

    fn join(self) -> Result<Self::Ok, Self::Error> {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchWorkingDirectoryError<E> {
    #[error(transparent)]
    ChildError(E),
    #[error("Error switching directories {0}")]
    SwitchDir(#[from] std::io::Error),
}
