use crate::{
    container::{step::Step, Context},
    linux,
};

pub struct PIDNamespace<S>(S)
where
    S: Step;

impl<S> PIDNamespace<S>
where
    S: Step,
{
    pub fn new(next: S) -> Self {
        Self(next)
    }
}

impl<S> Step for PIDNamespace<S>
where
    S: Step,
{
    type Error = PidNamespaceError<S::Error>;

    fn run(self, ctx: &mut Context) -> Result<(), Self::Error> {
        let res = linux::clone_vm_with_namespaces(
            libc::CLONE_NEWPID,
            unshare_pid_ns,
            SharedData {
                next: Some(self.0),
                ctx,
            },
        )
        .map_err(PidNamespaceError::ChildError)
        .unwrap();
        res.join().unwrap().map_err(PidNamespaceError::ChildError)?;
        Ok(())
    }
}

struct SharedData<'a, S>
where
    S: Step,
{
    next: Option<S>,
    ctx: &'a mut Context,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum PidNamespaceError<S>
where
    S: std::error::Error,
{
    #[error("Error creating pid namespace {0}")]
    ChildError(S),
}

fn unshare_pid_ns<S>(data: &mut SharedData<S>) -> (i32, Result<(), <S as Step>::Error>)
where
    S: Step,
{
    data.ctx.entered_pid_ns();
    linux::unshare(libc::CLONE_NEWPID).unwrap();
    log::trace!("New PID namespace {}", std::process::id());
    let r = data.next.take().unwrap().run(data.ctx);
    (0, r)
}
