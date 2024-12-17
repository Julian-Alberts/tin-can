use core::panic;

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Tried to use unsupported feature")]
    UnsupportedFeature,
    NotEnoughMemory,
    ToManyNamespaces,
    MissingPermissions,
    ToManyUserNamespaces,
}

impl Container<Validated> {
    pub fn create(
        Self {
            user,
            mount,
            pid,
            net,
            _p,
        }: Self,
    ) -> Result<Container<Created>, CreateError> {
        todo!()
    }
}

fn spawn<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(
    container: &Container<Validated>,
    f: F,
) -> std::thread::JoinHandle<Result<T, CreateError>> {
    let mut flags = 0;
    if container.user.is_some() {
        flags |= libc::CLONE_NEWUSER
    }
    if container.mount.is_some() {
        flags |= libc::CLONE_NEWNS
    }
    if container.pid {
        flags |= libc::CLONE_NEWPID
    }
    if container.net.is_some() {
        flags |= libc::CLONE_NEWNET
    }
    std::thread::spawn(move || {
        let res = unsafe { libc::unshare(flags) };
        if res == -1 {
            let Some(err) = std::io::Error::last_os_error().raw_os_error() else {
                panic!("Expected error")
            };
            let err = match err {
                libc::EINVAL => CreateError::UnsupportedFeature,
                libc::ENOMEM => CreateError::NotEnoughMemory,
                libc::ENOSPC => CreateError::ToManyNamespaces,
                libc::EPERM => CreateError::MissingPermissions,
                libc::EUSERS => CreateError::ToManyUserNamespaces,
                err => panic!("Unexpected OS error {err}"),
            };
            return Err(err);
        }

        Ok((f)())
    })
}
