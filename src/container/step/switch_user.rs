use crate::{
    container::{step::Step, Context},
    linux,
};

pub struct SwitchUser<S>
where
    S: Step,
{
    uid: libc::uid_t,
    gid: libc::gid_t,
    next_step: S,
}

impl<S> SwitchUser<S>
where
    S: Step,
{
    pub fn new(uid: u32, gid: u32, next_step: S) -> Self {
        Self {
            uid,
            gid,
            next_step,
        }
    }
}

impl<S> Step for SwitchUser<S>
where
    S: Step,
{
    type Error = SwitchUserError<S::Error>;

    fn run(self, ctx: &mut Context) -> Result<(), Self::Error> {
        linux::switch_user((self.uid, self.gid))?;
        Ok(self
            .next_step
            .run(ctx)
            .map_err(SwitchUserError::ChildError)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchUserError<S>
where
    S: std::error::Error,
{
    #[error(transparent)]
    SwitchUser(#[from] linux::SwitchUserError),
    #[error("Error switching user: {0}")]
    ChildError(S),
}
