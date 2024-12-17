pub mod container;

#[derive(Debug, thiserror::Error)]
pub enum CreateNameSpaceError {}
#[derive(Debug, thiserror::Error)]
pub enum JoinNamespaceError {
    #[error("Namespace not found for process")]
    NamespaceNotFound,
    #[error("PID not found")]
    UnableToOpenPidFd,
    #[error("Error opening namespace file {0:?}")]
    ErrorOpeningNamespace(std::io::Error),
}

pub enum Namespace {
    CGroup,
    Ipc,
    Mnt,
    Net,
    Pid,
    Time,
    User,
    Uts,
}

impl Namespace {
    fn ns_type(&self) -> libc::c_int {
        match self {
            Namespace::CGroup => libc::CLONE_NEWCGROUP,
            Namespace::Ipc => libc::CLONE_NEWIPC,
            Namespace::Mnt => libc::CLONE_NEWNS,
            Namespace::Net => libc::CLONE_NEWNET,
            Namespace::Pid => libc::CLONE_NEWPID,
            Namespace::Time => libc::CLONE_NEWTIME,
            Namespace::User => libc::CLONE_NEWUSER,
            Namespace::Uts => libc::CLONE_NEWUTS,
        }
    }
}

pub fn join_namespaces(
    pid: libc::pid_t,
    namespaces: &[Namespace],
) -> Result<(), JoinNamespaceError> {
    let res = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if res == -1 {
        return Err(JoinNamespaceError::UnableToOpenPidFd);
    }
    setns(
        res as libc::c_int,
        namespaces
            .iter()
            .map(Namespace::ns_type)
            .fold(0, |g, n| g | n),
    )?;
    Ok(())
}

fn setns(fd: libc::c_int, ns: libc::c_int) -> Result<(), JoinNamespaceError> {
    unsafe { libc::setns(fd, ns) };
    Ok(())
}
