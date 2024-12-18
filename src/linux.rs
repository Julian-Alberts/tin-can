use std::fmt::Display;

const EXPECT_RAW_OS_ERROR: &str = "Syscall failed with undefined error code";

#[derive(Debug, thiserror::Error)]
pub enum UnshareError {
    #[error("Tried to use unsupported feature")]
    UnsupportedFeature,
    #[error("Not enough memory for unshare")]
    NotEnoughMemory,
    #[error("To many namespaces")]
    ToManyNamespaces,
    #[error("Missing permissions")]
    MissingPermissions,
}
pub fn unshare(flags: i32) -> Result<(), UnshareError> {
    let res = unsafe { libc::unshare(flags) };
    if res == -1 {
        let err = std::io::Error::last_os_error()
            .raw_os_error()
            .expect(EXPECT_RAW_OS_ERROR);
        let err = match err {
            libc::EINVAL
                if flags & !(libc::CLONE_NEWNS | libc::CLONE_NEWIPC | libc::CLONE_NEWUSER) == 0 =>
            {
                UnshareError::UnsupportedFeature
            }
            libc::EINVAL => UnshareError::UnsupportedFeature,
            libc::ENOMEM => UnshareError::NotEnoughMemory,
            libc::ENOSPC | libc::EUSERS => UnshareError::ToManyNamespaces,
            libc::EPERM => UnshareError::MissingPermissions,
            err => panic!("Unexpected OS error {err}"),
        };
        return Err(err);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum CloneError {}

pub fn clone3(flags: i32) -> Result<libc::pid_t, CloneError> {
    let clone_args = libc::clone_args {
        flags: (flags | libc::CLONE_IO | libc::CLONE_VM) as u64,
        pidfd: 0,
        child_tid: 0,
        parent_tid: 0,
        exit_signal: 0,
        stack: 0,
        stack_size: 0,
        tls: 0,
        set_tid: 0,
        set_tid_size: 0,
        cgroup: 0,
    };
    let res = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &clone_args as *const _,
            std::mem::size_of::<libc::clone_args>(),
        )
    };
    match res {
        -1 => todo!("Handle error {:?}", std::io::Error::last_os_error()),
        pid => Ok(pid as i32),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Unable to set effective {property}: {kind}")]
pub struct SwitchUserError {
    property: SwitchUserProperty,
    kind: SwitchUserErrorKind,
}
#[derive(Debug)]
pub enum SwitchUserProperty {
    Uid,
    Gid,
}
impl Display for SwitchUserProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchUserProperty::Uid => write!(f, "uid"),
            SwitchUserProperty::Gid => write!(f, "gid"),
        }
    }
}
#[derive(Debug)]
pub enum SwitchUserErrorKind {
    InvalidId,
    MissingPermissions,
}
impl Display for SwitchUserErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchUserErrorKind::InvalidId => write!(f, "Invalid id"),
            SwitchUserErrorKind::MissingPermissions => write!(f, "Missing permissions"),
        }
    }
}
pub fn switch_user((uid, gid): (libc::uid_t, libc::gid_t)) -> Result<(), SwitchUserError> {
    fn convert_err(prop: SwitchUserProperty) -> SwitchUserError {
        match std::io::Error::last_os_error()
            .raw_os_error()
            .expect(EXPECT_RAW_OS_ERROR)
        {
            libc::EINVAL => SwitchUserError {
                property: prop,
                kind: SwitchUserErrorKind::InvalidId,
            },
            libc::EPERM => SwitchUserError {
                property: prop,
                kind: SwitchUserErrorKind::MissingPermissions,
            },
            e => panic!("Unexpected OS error {e}"),
        }
    }
    let res = unsafe { libc::seteuid(uid) };
    if res == -1 {
        return Err(convert_err(SwitchUserProperty::Uid));
    }

    let res = unsafe { libc::setegid(gid) };
    if res == -1 {
        return Err(convert_err(SwitchUserProperty::Gid));
    }

    Ok(())
}
