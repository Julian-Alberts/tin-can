use core::panic;
use std::{
    borrow::Borrow,
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
};

use capability::CAP_SETUID;

mod capability;

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
pub enum CloneError {
    #[error("Only namespace flags are allowed")]
    InvalidFlags,
}

#[derive(Debug)]
pub struct ProcessHandle<'a> {
    pub(super) pid: libc::pid_t,
    stack_ptr: *mut libc::c_void,
    _p: std::marker::PhantomData<&'a ()>,
}

impl<'a> ProcessHandle<'a> {
    pub fn join(mut self) -> i32 {
        log::info!("Joining namespace process");
        let mut status = -1;
        let res = unsafe { libc::waitpid(self.pid, &mut status, 0) };
        if res == -1 {
            let os_err = std::io::Error::last_os_error()
                .raw_os_error()
                .expect(EXPECT_RAW_OS_ERROR);
            match os_err {
                libc::EAGAIN => panic!("EAGAIN"),
                libc::ECHILD => panic!("ECHILD"),
                _ => panic!("Unknown"),
            }
        }
        self.pid = 0;
        status
    }

    pub fn placeholder() -> Self {
        Self {
            pid: 0,
            stack_ptr: std::ptr::null_mut(),
            _p: std::marker::PhantomData,
        }
    }
}

impl<'a> Drop for ProcessHandle<'a> {
    fn drop(&mut self) {
        if self.pid != 0 {
            let mut status = -1;
            unsafe { libc::waitpid(self.pid, &mut status, 0) };
            unsafe { libc::free(self.stack_ptr) };
        }
    }
}

pub fn clone_vm_with_namespaces<'a, T>(
    flags: i32,
    f: fn(&mut T) -> i32,
    args: &'a mut T,
) -> Result<ProcessHandle<'a>, CloneError> {
    const NAMESPACE_FLAGS: i32 = libc::CLONE_NEWNS
        | libc::CLONE_NEWIPC
        | libc::CLONE_NEWNET
        | libc::CLONE_NEWPID
        | libc::CLONE_NEWUTS
        | libc::CLONE_NEWTIME
        | libc::CLONE_NEWUSER
        | libc::CLONE_NEWCGROUP;
    const VM_FLAGS: i32 = libc::CLONE_FILES | libc::SIGCHLD;
    if flags & !NAMESPACE_FLAGS != 0 {
        return Err(CloneError::InvalidFlags);
    }
    let stack = new_stack();
    #[derive(Debug)]
    struct Args<'a, T> {
        fn_args: &'a mut T,
        callback: fn(&mut T) -> i32,
    }
    extern "C" fn callback<T>(args: *mut libc::c_void) -> i32 {
        log::info!("Successfuly cloned new process");
        let args = args as *mut Args<T>;
        let args = unsafe { args.as_mut() }.unwrap();
        log::info!("Calling callback with args");
        let res = (args.callback)(&mut args.fn_args);
        log::info!("Finished callback");
        res
    }
    let res = unsafe {
        libc::clone(
            callback::<T>,
            stack,
            flags | VM_FLAGS,
            &mut Args {
                fn_args: args,
                callback: f,
            } as *mut Args<T> as *mut libc::c_void,
        )
    };
    match res {
        -1 => todo!("Handle error {:?}", std::io::Error::last_os_error()),
        pid => Ok(ProcessHandle {
            pid,
            stack_ptr: stack,
            _p: std::marker::PhantomData,
        }),
    }
}

fn new_stack() -> *mut libc::c_void {
    const STACK_SIZE: libc::size_t = 1024 * 1024 * 10;
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            STACK_SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_STACK,
            -1,
            0,
        )
    };
    unsafe { ptr.add(STACK_SIZE) }
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

pub(crate) fn kill(pid: i32) {
    unsafe { libc::kill(pid, libc::SIGTERM) };
}

pub(crate) fn waitpid(pid: i32, status: &mut i32, options: i32) {
    let res = unsafe { libc::waitpid(pid, status as *mut _, options) };
    if res != -1 {
        return;
    }
    match std::io::Error::last_os_error()
        .raw_os_error()
        .expect("No error")
    {
        libc::EAGAIN => println!("EAGAIN"),
        libc::ECHILD => println!("ECHILD"),
        libc::EINTR => println!("EINTR"),
        libc::EINVAL => println!("EINVAL"),
        libc::ESRCH => println!("ESRCH"),
        _ => panic!(),
    }
}

pub(crate) fn get_euid() -> u32 {
    unsafe { libc::geteuid() }
}

pub(crate) fn get_egid() -> u32 {
    unsafe { libc::getegid() }
}

#[derive(Debug)]
pub struct EventFd<T> {
    event_fd: libc::c_int,
    _p: std::marker::PhantomData<T>,
}

impl<T> EventFd<T> {
    pub fn new() -> std::io::Result<Self> {
        let event_fd = unsafe { libc::eventfd(0, 0) };
        if event_fd == -1 {
            return Err(std::io::Error::last_os_error());
        };
        Ok(Self {
            event_fd,
            _p: Default::default(),
        })
    }

    pub fn send(&self, data: T) -> std::io::Result<()> {
        let res = unsafe {
            libc::write(
                self.event_fd,
                &data as *const _ as *const _,
                std::mem::size_of::<T>(),
            )
        };

        if res == -1 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(())
    }

    pub fn receive(&self) -> std::io::Result<T> {
        let mut data = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        let res = unsafe {
            libc::read(
                self.event_fd,
                &mut data as *mut _ as *mut _,
                std::mem::size_of::<T>(),
            )
        };

        if res == -1 {
            return Err(std::io::Error::last_os_error());
        }

        Ok(data)
    }
}

impl<T> Clone for EventFd<T> {
    fn clone(&self) -> Self {
        Self {
            event_fd: unsafe { libc::dup(self.event_fd) },
            _p: self._p.clone(),
        }
    }
}

impl<T> Drop for EventFd<T> {
    fn drop(&mut self) {
        unsafe { libc::close(self.event_fd) };
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Capability {
    SETGID,
    SETUID,
}

impl Capability {
    fn to_cap(&self) -> capability::cap_value_t {
        match self {
            Capability::SETGID => capability::CAP_SETGID as i32,
            Capability::SETUID => capability::CAP_SETUID as i32,
        }
    }
}

pub fn has_capability(cap: Capability) -> bool {
    let caps = unsafe { capability::cap_get_proc() };
    let mut cap_value = 0;
    unsafe {
        capability::cap_get_flag(
            caps,
            cap.to_cap(),
            capability::cap_flag_t_CAP_EFFECTIVE,
            &mut cap_value,
        )
    };
    cap_value == 1
}
