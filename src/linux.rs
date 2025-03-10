use core::panic;
use std::{
    fmt::{Debug, Display},
    os::{fd::FromRawFd, unix::ffi::OsStrExt},
    path::PathBuf,
};

use nix::errno::Errno;

#[cfg(feature = "cap")]
pub mod libcap;

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
pub struct ProcessHandle<T, R> {
    pub(super) pid: libc::pid_t,
    stack_ptr: *mut libc::c_void,
    args: *mut CloneArgs<T, R>,
}

impl<T, R> ProcessHandle<T, R> {
    /// TODO: Change return to result
    pub fn join(mut self) -> Option<R> {
        log::info!("Waiting for namespace process {}", self.pid);
        let mut status = -1;
        let res = unsafe { libc::waitpid(self.pid, &mut status, 0) };
        log::info!("namespace process {} returned", self.pid);
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
        log::debug!("Trying to restore arguments from pointer");
        let args = unsafe { Box::from_raw(self.args) };
        args.result
    }
}

impl<T, R> Drop for ProcessHandle<T, R> {
    fn drop(&mut self) {
        if self.pid != 0 {
            let mut status = -1;
            unsafe { libc::kill(self.pid, libc::SIGTERM) };
            unsafe { libc::waitpid(self.pid, &mut status, 0) };
            self.pid = 0;
        }
    }
}

pub fn clone_vm_with_namespaces<T, R>(
    flags: i32,
    f: fn(&mut T) -> (i32, R),
    // The arguments will be move onto the heap. They will not be dropped when leaving this
    // function. Dropping args is up to the process handle.
    args: T,
) -> Result<ProcessHandle<T, R>, CloneError> {
    log::trace!("clone new vm namespace");
    const NAMESPACE_FLAGS: i32 = libc::CLONE_NEWNS
        | libc::CLONE_NEWIPC
        | libc::CLONE_NEWNET
        | libc::CLONE_NEWPID
        | libc::CLONE_NEWUTS
        | libc::CLONE_NEWTIME
        | libc::CLONE_NEWUSER
        | libc::CLONE_NEWCGROUP;
    const VM_FLAGS: i32 = libc::CLONE_FILES | libc::SIGCHLD | libc::CLONE_VM;
    if flags & !NAMESPACE_FLAGS != 0 {
        return Err(CloneError::InvalidFlags);
    }
    let stack = new_stack();
    extern "C" fn callback<T, R>(args: *mut libc::c_void) -> i32 {
        log::info!("Successfuly cloned new process");
        let args = args as *mut CloneArgs<T, R>;
        let args = unsafe { args.as_mut() }.unwrap();
        log::info!("Calling callback with args");
        let res = (args.callback)(&mut args.fn_args);
        log::info!("Finished callback");
        args.result = Some(res.1);
        res.0
    }
    log::debug!("Given callback addr {:p}", std::ptr::addr_of!(f),);
    let args = Box::into_raw(Box::new(CloneArgs {
        fn_args: args,
        callback: f,
        result: None,
    }));
    let res = unsafe { libc::clone(callback::<T, R>, stack, flags | VM_FLAGS, args as *mut _) };
    match res {
        -1 => todo!("Handle error {:?}", std::io::Error::last_os_error()),
        pid => Ok(ProcessHandle {
            pid,
            stack_ptr: stack,
            args,
        }),
    }
}

#[derive(Debug)]
struct CloneArgs<T, R> {
    fn_args: T,
    callback: fn(&mut T) -> (i32, R),
    result: Option<R>,
}

fn new_stack() -> *mut libc::c_void {
    const STACK_SIZE: libc::size_t = 1024 * 1024;
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
            log::error!("Failed to send data to fd: {}", self.event_fd);
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
        log::debug!("Close event fd: {}", self.event_fd);
        unsafe { libc::close(self.event_fd) };
    }
}

pub(crate) fn pivot_root(
    new_root: &std::path::Path,
    put_old: &std::path::Path,
) -> Result<(), PivotRootError> {
    match nix::unistd::pivot_root(new_root, put_old) {
        Ok(_) => Ok(()),
        Err(Errno::EBUSY) => Err(PivotRootError::NewRootIsOldRoot),
        Err(Errno::EINVAL) if !is_mount_point(new_root) => {
            Err(PivotRootError::NewRootIsNotMountPoint)
        }
        Err(Errno::EINVAL) if !is_mount_point(std::path::PathBuf::from("/").as_path()) => {
            Err(PivotRootError::CurrentRootIsNotMountPoint)
        }
        Err(Errno::ENOTDIR) if !new_root.is_dir() => Err(PivotRootError::NewRootIsNotDir),
        Err(Errno::ENOTDIR) if !put_old.is_dir() => Err(PivotRootError::PutOldIsNotDir),
        Err(Errno::EPERM) => Err(PivotRootError::MissingPermissions),
        Err(e) => panic!("Unexpected error {e}"),
    }
}

fn is_mount_point(path: &std::path::Path) -> bool {
    let mtab_fs = unsafe { libc::setmntent(c"/etc/mtab".as_ptr(), c"r".as_ptr()) };
    if mtab_fs.is_null() {
        log::error!("Unable to open /etc/mtab.");
        return false;
    };

    let mut mounted = false;
    let mut mount_point = unsafe { libc::getmntent(mtab_fs) };

    while !mount_point.is_null() {
        let mnt = unsafe { mount_point.as_ref().unwrap() };
        if !mnt.mnt_fsname.is_null() {
            unsafe { std::hint::assert_unchecked(mnt.mnt_fsname.as_ref().is_some()) };
            let fsname = unsafe { mnt.mnt_fsname.as_ref().unwrap() };
            let fsname = unsafe { std::ffi::CStr::from_ptr(fsname) };
            let path = std::ffi::CStr::from_bytes_with_nul(path.as_os_str().as_bytes()).unwrap();

            if fsname == path {
                mounted = true;
                break;
            }
        }
        mount_point = unsafe { libc::getmntent(mtab_fs) }
    }

    unsafe { libc::endmntent(mtab_fs) };
    mounted
}

#[derive(Debug, thiserror::Error)]
pub enum PivotRootError {
    #[error("new_root is the same as the old root")]
    NewRootIsOldRoot,
    #[error("new_root is not a directory")]
    NewRootIsNotDir,
    #[error("put_old is not a directory")]
    PutOldIsNotDir,
    #[error("Missing permissions to run pivot_root")]
    MissingPermissions,
    #[error("new_root is not a mount point")]
    NewRootIsNotMountPoint,
    #[error("Current root (/) is not a mount point")]
    CurrentRootIsNotMountPoint,
}

pub(crate) fn mount_overlay(
    lower: &std::path::Path,
    upper: &std::path::Path,
    work: &std::path::Path,
    merged: &std::path::Path,
) -> Result<(), std::io::Error> {
    let mut data = b"lowerdir=".to_vec();
    data.extend_from_slice(lower.as_os_str().as_bytes());
    data.extend_from_slice(b",upperdir=");
    data.extend_from_slice(upper.as_os_str().as_bytes());
    data.extend_from_slice(b",workdir=");
    data.extend_from_slice(work.as_os_str().as_bytes());
    data.push(0);
    log::debug!("overlay data: {:?}", String::from_utf8(data.clone()));
    log::debug!("overlay merged: {:?}", merged);
    let data = std::ffi::CStr::from_bytes_with_nul(&data).unwrap();
    mount(
        &PathBuf::from("overlay"),
        merged,
        Some(c"overlay"),
        0,
        Some(data),
    )
}

pub fn mount(
    source: &std::path::Path,
    target: &std::path::Path,
    file_system_type: Option<&std::ffi::CStr>,
    mount_flags: libc::c_ulong,
    data: Option<&std::ffi::CStr>,
) -> Result<(), std::io::Error> {
    let src = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
    let target = std::ffi::CString::new(target.as_os_str().as_bytes()).unwrap();
    let data = if let Some(data) = data {
        data.as_ptr()
    } else {
        std::ptr::null()
    } as *const _;
    let file_system_type = if let Some(fs) = file_system_type {
        fs.as_ptr()
    } else {
        std::ptr::null()
    };
    let res = unsafe {
        libc::mount(
            src.as_ptr(),
            target.as_ptr(),
            file_system_type,
            mount_flags,
            data,
        )
    };

    if res == -1 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

pub(crate) fn get_user_name(uid: u32) -> Option<String> {
    let passwd = unsafe { libc::getpwuid(uid) };
    if passwd.is_null() {
        return None;
    }

    let passwd = unsafe { passwd.as_ref().unwrap() };
    let username = unsafe { std::ffi::CStr::from_ptr(passwd.pw_name) };
    Some(username.to_str().unwrap().to_string())
}

pub(crate) fn pidfd_open(pid: u32) -> std::fs::File {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    unsafe { std::fs::File::from_raw_fd(fd as i32) }
}
