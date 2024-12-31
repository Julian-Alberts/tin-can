use crate::linux::{self, UnshareError};

use super::Component;

pub struct MountNamespace<C>(C)
where
    C: Component;

impl<C> MountNamespace<C>
where
    C: Component,
{
    pub fn new(c: C) -> Self {
        Self(c)
    }
}

impl<C> Component for MountNamespace<C>
where
    C: Component,
{
    type Error = MountNamespaceError<C::Error>;

    type Ok = C::Ok;

    fn run(self) -> Result<Self::Ok, Self::Error> {
        linux::unshare(libc::CLONE_NEWNS)?;
        self.0.run().map_err(MountNamespaceError::ChildError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MountNamespaceError<E>
where
    E: std::error::Error,
{
    #[error(transparent)]
    Unshare(#[from] UnshareError),
    #[error(transparent)]
    ChildError(E),
}
