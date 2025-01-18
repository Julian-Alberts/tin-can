use std::path::PathBuf;

use crate::container::{step::Step, Context};

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

    fn run(self, ctx: &mut Context) -> Result<(), Self::Error> {
        std::env::set_current_dir(self.new_wd)?;
        Ok(self
            .next
            .run(ctx)
            .map_err(SwitchWorkingDirectoryError::ChildError)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchWorkingDirectoryError<E> {
    #[error(transparent)]
    ChildError(E),
    #[error("Error switching directories {0}")]
    SwitchDir(#[from] std::io::Error),
}
