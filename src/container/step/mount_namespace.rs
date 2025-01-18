use std::{
    ffi::{CStr, CString},
    os::unix::ffi::OsStrExt as _,
    path::PathBuf,
};

use nix::mount::{MntFlags, MsFlags};

use crate::{
    container::{step::Step, Context},
    linux,
};

pub struct MountNamespace<'a, C>
where
    C: Step,
{
    next: C,
    operations: Vec<MountOperation<'a>>,
}

impl<'a, C> MountNamespace<'a, C>
where
    C: Step,
{
    pub fn new(operations: Vec<MountOperation<'a>>, c: C) -> Self {
        Self {
            next: c,
            operations,
        }
    }
}

impl<'a, C> Step for MountNamespace<'a, C>
where
    C: Step,
{
    type Error = MountNamespaceError<C::Error>;

    fn run(self, ctx: &mut Context) -> Result<(), Self::Error> {
        ctx.entered_mnt_ns();
        log::info!("Unshare mount namespace");
        nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS)
            .map_err(MountNamespaceError::Unshare)?;
        self.operations
            .into_iter()
            .try_for_each(MountOperation::run)?;
        log::info!("Finished mounting");
        self.next.run(ctx).map_err(MountNamespaceError::ChildError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MountNamespaceError<E>
where
    E: std::error::Error,
{
    #[error("Failed to unshare {0}")]
    Unshare(nix::errno::Errno),
    #[error("Failed to run mount operation {0}")]
    Op(#[from] MountingError),
    #[error(transparent)]
    ChildError(E),
}

pub enum MountOperation<'a> {
    OverlayMount {
        lower: PathBuf,
        upper: PathBuf,
        work: PathBuf,
        merged: PathBuf,
    },
    PivotRoot {
        new_root: PathBuf,
        put_old: PathBuf,
        auto_unmount: bool,
        create_if_does_not_exisit: bool,
    },
    BindMount {
        src: Option<PathBuf>,
        target: PathBuf,
    },
    Unmount {
        mount: PathBuf,
        lazy: bool,
    },
    Mount {
        source: Option<PathBuf>,
        target: PathBuf,
        fs_type: Option<&'a std::ffi::CStr>,
        flags: nix::mount::MsFlags,
        data: Option<&'a std::ffi::CStr>,
    },
}

impl<'a> MountOperation<'a> {
    pub fn switch_root(
        new_root: impl Into<PathBuf> + Clone,
        put_old: impl Into<PathBuf> + Clone,
    ) -> Vec<Self> {
        vec![
            Self::BindMount {
                src: Some(new_root.clone().into()),
                target: new_root.clone().into(),
            },
            Self::PivotRoot {
                new_root: new_root.into(),
                put_old: put_old.into(),
                auto_unmount: true,
                create_if_does_not_exisit: true,
            },
        ]
    }

    pub fn switch_root_with_overlay(
        lower_ro: impl Into<PathBuf> + Clone,
        upper_rw: impl Into<PathBuf> + Clone,
        work_sys: impl Into<PathBuf> + Clone,
        new_root: impl Into<PathBuf> + Clone,
        put_old: impl Into<PathBuf> + Clone,
        //mount_sys_dirs: bool,
    ) -> Vec<Self> {
        let new_root: PathBuf = new_root.into();
        let upper_rw: PathBuf = upper_rw.into();
        vec![
            Self::OverlayMount {
                lower: lower_ro.into(),
                upper: upper_rw.clone().into(),
                work: work_sys.into(),
                merged: new_root.clone(),
            },
            /*Self::Mount {
                source: "devpts".into(),
                target: new_root.join("dev/pts"),
                fs_type: Some(c"devpts"),
                flags: 0,
                data: None,
            },
            Self::Mount {
                source: None,
                target: new_root.join("proc"),
                fs_type: Some(c"proc"),
                flags: MsFlags::empty(),
                data: None,
            },*/
            Self::BindMount {
                src: new_root.clone().into(),
                target: new_root.clone().into(),
            },
            Self::PivotRoot {
                new_root: new_root.into(),
                put_old: put_old.into(),
                auto_unmount: false,
                create_if_does_not_exisit: false,
            },
        ]
    }

    fn run(self) -> Result<(), MountingError> {
        match self {
            MountOperation::OverlayMount {
                lower,
                upper,
                work,
                merged,
            } => {
                let mut data = b"lowerdir=".to_vec();
                data.extend_from_slice(lower.as_os_str().as_bytes());
                data.extend_from_slice(b",upperdir=");
                data.extend_from_slice(upper.as_os_str().as_bytes());
                data.extend_from_slice(b",workdir=");
                data.extend_from_slice(work.as_os_str().as_bytes());
                nix::mount::mount(
                    Some("overlay"),
                    &merged,
                    Some("overlay"),
                    MsFlags::empty(),
                    Some(CString::new(data).unwrap().as_c_str()),
                )
                .map_err(|error| MountingError::Fallback {
                    mount_type: "overlay",
                    error,
                })
            }
            MountOperation::PivotRoot {
                new_root,
                put_old,
                auto_unmount,
                create_if_does_not_exisit,
            } => pivot_root(
                new_root.as_path(),
                put_old.as_path(),
                auto_unmount,
                create_if_does_not_exisit,
            ),
            MountOperation::BindMount { src, target } => {
                log::debug!("Bind {:?} to {:?}", src, target);
                nix::mount::mount(
                    src.as_ref(),
                    &target,
                    None::<&CStr>,
                    MsFlags::MS_BIND,
                    None::<&CStr>,
                )
                .map_err(|error| MountingError::Fallback {
                    mount_type: "bind",
                    error,
                })
            }
            MountOperation::Unmount { mount, lazy } => {
                log::debug!("Unmount {} lazy: {lazy}", mount.to_string_lossy());
                let lazy = if lazy {
                    MntFlags::MNT_DETACH
                } else {
                    MntFlags::empty()
                };
                nix::mount::umount2(&mount, lazy).map_err(|error| MountingError::Fallback {
                    mount_type: "umount",
                    error,
                })
            }
            MountOperation::Mount {
                source,
                target,
                fs_type,
                flags,
                data,
            } => {
                log::debug!("mounting {source:?} of type {fs_type:?} to {target:?} with flags: {flags:?} and options {data:?}");
                nix::mount::mount(source.as_ref(), &target, fs_type, flags, data).map_err(|error| {
                    MountingError::Fallback {
                        mount_type: "mount",
                        error,
                    }
                })
            }
        }
    }
}

fn pivot_root(
    new_root: &std::path::Path,
    put_old: &std::path::Path,
    auto_unmount: bool,
    create_if_does_not_exist: bool,
) -> Result<(), MountingError> {
    log::debug!(
        "PivotRoot use {:?} as new root and move old root to {:?}",
        new_root,
        put_old
    );
    let put_old = new_root.join(put_old);
    let existed = if create_if_does_not_exist && !put_old.exists() {
        std::fs::create_dir_all(&put_old).map_err(MountingError::UnableToCreatePutOld)?;
        false
    } else {
        true
    };
    linux::pivot_root(&new_root, &put_old)?;
    if auto_unmount {
        let put_old = PathBuf::from("/").join(&put_old);
        MountOperation::Unmount {
            mount: put_old.clone(),
            lazy: true,
        }
        .run()?;
    }
    if !existed {
        std::fs::remove_dir(&put_old).map_err(MountingError::UnableToRmPutOld)?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MountingError {
    #[error("Mount operation failed type: \"{mount_type}\" error: {error}")]
    Fallback {
        mount_type: &'static str,
        error: nix::errno::Errno,
    },
    #[error("Failed to pivot root {0}")]
    PivotRoot(#[from] linux::PivotRootError),
    #[error("Failed to create put_old")]
    UnableToCreatePutOld(std::io::Error),
    #[error("Failed to remove put_old")]
    UnableToRmPutOld(std::io::Error),
}
