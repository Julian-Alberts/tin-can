use std::path::{Path, PathBuf};

use crate::{
    container::Step,
    linux::{self, UnshareError},
};

pub struct MountNamespace<'a, C>
where
    C: Step,
{
    c: C,
    operations: Vec<MountOperation<'a>>,
}

impl<'a, C> MountNamespace<'a, C>
where
    C: Step,
{
    pub fn new(c: C, operations: Vec<MountOperation<'a>>) -> Self {
        Self { c, operations }
    }
}

impl<'a, C> Step for MountNamespace<'a, C>
where
    C: Step,
{
    type Error = MountNamespaceError<C::Error>;

    type Ok = C::Ok;

    fn run(self) -> Result<Self::Ok, Self::Error> {
        log::info!("Unshare mount namespace");
        linux::unshare(libc::CLONE_NEWNS)?;
        self.operations
            .into_iter()
            .try_for_each(MountOperation::run)?;
        log::info!("Finished mounting");
        self.c.run().map_err(MountNamespaceError::ChildError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MountNamespaceError<E>
where
    E: std::error::Error,
{
    #[error(transparent)]
    Unshare(#[from] UnshareError),
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
    },
    BindMount {
        src: PathBuf,
        target: PathBuf,
    },
    Unmount {
        mount: PathBuf,
        lazy: bool,
    },
    Mount {
        source: PathBuf,
        target: PathBuf,
        fs_type: Option<&'a std::ffi::CStr>,
        flags: u64,
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
                src: new_root.clone().into(),
                target: new_root.clone().into(),
            },
            Self::PivotRoot {
                new_root: new_root.into(),
                put_old: put_old.into(),
                auto_unmount: true,
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
            Self::Mount {
                source: "devpts".into(),
                target: upper_rw.clone().join("dev/pts"),
                fs_type: Some(c"devpts"),
                flags: 0,
                data: None,
            },
            Self::Mount {
                source: "proc".into(),
                target: upper_rw.clone().join("proc"),
                fs_type: Some(c"proc"),
                flags: 0,
                data: None,
            },
            Self::OverlayMount {
                lower: lower_ro.into(),
                upper: upper_rw.into(),
                work: work_sys.into(),
                merged: new_root.clone(),
            },
            Self::BindMount {
                src: new_root.clone().into(),
                target: new_root.clone().into(),
            },
            Self::PivotRoot {
                new_root: new_root.into(),
                put_old: put_old.into(),
                auto_unmount: true,
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
            } => linux::mount_overlay(&lower, &upper, &work, &merged).map_err(|error| {
                MountingError {
                    mount_type: "overlay",
                    error,
                }
            }),
            MountOperation::PivotRoot {
                new_root,
                put_old,
                auto_unmount,
            } => {
                log::debug!(
                    "PivotRoot use {:?} as new root and move old root to {:?}",
                    new_root,
                    put_old
                );
                let abs_put_old = new_root.join(&put_old);
                linux::pivot_root(&new_root, &abs_put_old).map_err(|error| MountingError {
                    mount_type: "pivot_root",
                    error,
                })?;
                if auto_unmount {
                    let put_old = PathBuf::from("/").join(put_old);
                    MountOperation::Unmount {
                        mount: put_old.clone(),
                        lazy: true,
                    }
                    .run()?;
                }
                Ok(())
            }
            MountOperation::BindMount { src, target } => {
                log::debug!("Bind {:?} to {:?}", src, target);
                linux::bind_mount(&src, &target).map_err(|error| MountingError {
                    mount_type: "bind",
                    error,
                })
            }
            MountOperation::Unmount { mount, lazy } => {
                log::debug!("Unmount {} lazy: {lazy}", mount.to_string_lossy());
                linux::unmount(&mount, lazy).map_err(|error| MountingError {
                    mount_type: "unmount",
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
                log::debug!("mounting {source:?} as {fs_type:?} to {target:?} with flags: {flags} and options {data:?}");
                linux::mount(&source, &target, fs_type, flags, data).map_err(|error| {
                    MountingError {
                        mount_type: "mount",
                        error,
                    }
                })
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Mount operation failed type: \"{mount_type}\" error: {error}")]
pub struct MountingError {
    mount_type: &'static str,
    error: std::io::Error,
}
