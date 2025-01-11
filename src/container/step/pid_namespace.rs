use crate::{
    container::Step,
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

    type Ok = Result<S::Ok, S::Error>;

    fn run(self) -> Result<Self::Ok, Self::Error> {
        let res = linux::clone_vm_with_namespaces(libc::CLONE_NEWPID, unshare_pid_ns, Some(self.0));
        let res = match res {
            Ok(h) => h.join().unwrap(),
            Err(_) => todo!(),
        };
        Ok(res)
    }
}

fn unshare_pid_ns<S>(next: &mut Option<S>) -> (i32, Result<<S as Step>::Ok, <S as Step>::Error>)
where
    S: Step,
{
    linux::unshare(libc::CLONE_NEWPID).unwrap();
    log::trace!("New PID namespace {}", std::process::id());
    (0, next.take().unwrap().run())
}
