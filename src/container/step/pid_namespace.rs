use crate::{
    container::step::{Step, StepHandle},
    linux::{self, CloneError, ProcessHandle},
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
    type Error = CloneError;

    type Handle = PidNamespaceHandle<S>;

    fn run(self) -> Result<Self::Handle, Self::Error> {
        let res =
            linux::clone_vm_with_namespaces(libc::CLONE_NEWPID, unshare_pid_ns, Some(self.0))?;
        Ok(PidNamespaceHandle(res))
    }
}

pub struct PidNamespaceHandle<S>(ProcessHandle<Option<S>, Result<S::Handle, S::Error>>)
where
    S: Step;

impl<S> StepHandle for PidNamespaceHandle<S>
where
    S: Step,
{
    type Error = S::Error;

    type Ok = S::Handle;

    fn join(self) -> Result<S::Handle, S::Error> {
        self.0.join().unwrap() // TODO: remove unwrap
    }
}

fn unshare_pid_ns<S>(next: &mut Option<S>) -> (i32, Result<<S as Step>::Handle, <S as Step>::Error>)
where
    S: Step,
{
    linux::unshare(libc::CLONE_NEWPID).unwrap();
    log::trace!("New PID namespace {}", std::process::id());
    (0, next.take().unwrap().run())
}
