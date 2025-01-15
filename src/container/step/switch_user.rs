use crate::{
    container::step::{Step, StepHandle},
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
    type Error = SwitchUserError;

    type Handle = SwitchUserHandle<S::Handle, S::Error>;

    fn run(self) -> Result<Self::Handle, Self::Error> {
        linux::switch_user((self.uid, self.gid))?;
        Ok(SwitchUserHandle {
            res: self.next_step.run(),
            _p: std::marker::PhantomData,
        })
    }
}

pub struct SwitchUserHandle<O, E> {
    res: Result<O, E>,
    _p: std::marker::PhantomData<(O, E)>,
}

impl<O, E> StepHandle for SwitchUserHandle<O, E>
where
    E: std::error::Error,
{
    type Error = E;

    type Ok = O;

    fn join(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchUserError {
    #[error(transparent)]
    SwitchUser(#[from] linux::SwitchUserError),
}
