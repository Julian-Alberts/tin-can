use std::fmt::Debug;

use crate::container::Context;
use crate::{container::MapType, linux};

use crate::container::{step::Step, Group, IdMap, User};

pub struct UserNamespaceRoot<S> {
    next_step: S,
    uid_map: IdMap<User>,
    gid_map: IdMap<Group>,
    switch_to: Option<(u32, u32)>,
}

impl<S> UserNamespaceRoot<S> {
    pub fn new_with_current_user_as_root(next_step: S) -> Self {
        Self {
            next_step,
            uid_map: IdMap::new_with_current_user_as_root(),
            gid_map: IdMap::new_with_current_user_as_root(),
            switch_to: Some((0, 0)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NewUserNamespaceError {
    #[error("Process is missing capability SETUID")]
    MissingCapabilitySetUid,
    #[error("Process is missing capability SETGID")]
    MissingCapabilitySetGid,
}

#[cfg(feature = "map_uid_range")]
impl<S> UserNamespaceRoot<S> {
    pub fn new(
        uid_map: IdMap<User>,
        gid_map: IdMap<Group>,
        user: Option<(u32, u32)>,
        next_step: S,
    ) -> Result<Self, NewUserNamespaceError> {
        if uid_map.entries.len() > 1
            && !linux::libcap::has_capability(linux::libcap::Capability::SETUID)
        {
            log::error!("The process is missing the SETUID permission");
            return Err(NewUserNamespaceError::MissingCapabilitySetUid);
        }
        if gid_map.entries.len() > 1
            && !linux::libcap::has_capability(linux::libcap::Capability::SETGID)
        {
            log::error!("The process is missing the SETGID permission");
            return Err(NewUserNamespaceError::MissingCapabilitySetGid);
        }
        Ok(Self {
            next_step,
            uid_map,
            gid_map,
            switch_to: user,
        })
    }
}
impl<S> Step for UserNamespaceRoot<S>
where
    S: Step,
{
    type Error = BuildUserNamespaceRootError<S::Error>;

    fn run(self, ctx: &mut Context) -> Result<(), Self::Error> {
        log::trace!("Create user namespace");
        let msg_queue_ctp = linux::EventFd::new().unwrap();
        let msg_queue_ptc = linux::EventFd::new().unwrap();
        let shared_data = SharedData {
            component: Some(self.next_step),
            msg_queue_ctp: msg_queue_ctp.clone(),
            msg_queue_ptc: msg_queue_ptc.clone(),
            switch_to: self.switch_to,
            ctx,
        };
        let join_handle =
            linux::clone_vm_with_namespaces(libc::CLONE_NEWUSER, root_namespace_vm, shared_data)?;
        log::info!("PID: {}", join_handle.pid);
        fn write_id_map<T: MapType>(map: IdMap<T>, pid: libc::pid_t) -> Result<(), IdMapError<T>> {
            log::debug!("Creating {} for process {pid}", T::file());
            use std::io::Write as _;
            T::prepare_process(pid).map_err(|error| IdMapError {
                error,
                kind: IdMapErrorKind::FailedToPrepareProcess,
                _p: Default::default(),
            })?;
            let mut path = std::path::PathBuf::from("/proc/");
            path.push(pid.to_string());
            path.push(T::file());
            log::debug!("Writing {:?}", path);
            let mut file = std::fs::File::create(path).map_err(|error| IdMapError {
                error,
                kind: IdMapErrorKind::FailedToCreateIdMapFile,
                _p: std::marker::PhantomData,
            })?;
            let mut buf = Vec::new();
            for entry in map.entries {
                log::debug!("{} {} {}", entry.internal, entry.external, entry.len);
                write!(buf, "{} {} {}\n", entry.internal, entry.external, entry.len).map_err(
                    |error| IdMapError {
                        error,
                        kind: IdMapErrorKind::FailedToWriteIdMapFile,
                        _p: std::marker::PhantomData,
                    },
                )?;
            }
            file.write_all(buf.as_slice()).map_err(|error| IdMapError {
                error,
                kind: IdMapErrorKind::FailedToWriteIdMapFile,
                _p: std::marker::PhantomData,
            })?;
            Ok(())
        }

        log::debug!("Wait for Signal");
        msg_queue_ctp.receive().unwrap();
        log::debug!("Got Signal");
        write_id_map(self.uid_map, join_handle.pid)?;
        write_id_map(self.gid_map, join_handle.pid)?;
        log::debug!("Send Signal");
        msg_queue_ptc.send(1).unwrap();
        // shared_data.ret can only be assumed to be set after the child has finished
        log::debug!("Wait for namespace");
        let ctx = join_handle.join().unwrap()?;
        Ok(ctx)
    }
}

#[derive(thiserror::Error)]
pub struct IdMapError<T: MapType> {
    error: std::io::Error,
    kind: IdMapErrorKind,
    _p: std::marker::PhantomData<T>,
}

pub enum IdMapErrorKind {
    FailedToPrepareProcess,
    FailedToCreateIdMapFile,
    FailedToWriteIdMapFile,
}

impl<T> IdMapError<T>
where
    T: MapType,
{
    fn new(kind: IdMapErrorKind, error: std::io::Error) -> Self {
        Self {
            error,
            kind,
            _p: std::marker::PhantomData,
        }
    }
}

impl std::fmt::Debug for IdMapError<User> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdMapError<User>")
            .field("error", &self.error)
            .finish()
    }
}

impl std::fmt::Debug for IdMapError<Group> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdMapError<Group>")
            .field("error", &self.error)
            .finish()
    }
}

impl std::fmt::Display for IdMapError<User> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error while mapping user id: {}", self.error)
    }
}

impl std::fmt::Display for IdMapError<Group> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error while mapping group id: {}", self.error)
    }
}

struct SharedData<'a, C>
where
    C: Step,
{
    component: Option<C>,
    msg_queue_ctp: linux::EventFd<usize>,
    msg_queue_ptc: linux::EventFd<usize>,
    switch_to: Option<(u32, u32)>,
    ctx: &'a mut Context,
}
fn root_namespace_vm<S>(
    data: &mut SharedData<S>,
) -> (i32, Result<(), BuildUserNamespaceRootError<S::Error>>)
where
    S: Step,
{
    data.ctx.entered_user_ns();
    log::debug!("root namespace main");
    if let Err(e) = data.msg_queue_ctp.send(1) {
        log::error!("Failed to send signal to parent: {e}");
        return (1, Err(BuildUserNamespaceRootError::MsgQueue));
    };
    if let Err(e) = data.msg_queue_ptc.receive() {
        log::error!("Failed to receive signal from parent: {e}");
        return (1, Err(BuildUserNamespaceRootError::MsgQueue));
    };
    log::debug!("Namespace resumed");
    if let Some(user) = data.switch_to {
        linux::switch_user(user).unwrap();
        log::debug!("Switched to user uid: {} gid: {}", user.0, user.1)
    }
    let res = data
        .component
        .take()
        .expect("Component called twice")
        .run(data.ctx)
        .map_err(BuildUserNamespaceRootError::ChildError)
        .inspect_err(|e| log::error!("{e}"));
    (0, res)
}

#[derive(Debug, thiserror::Error)]
pub enum BuildUserNamespaceRootError<E: std::error::Error> {
    #[error(transparent)]
    CloneError(#[from] linux::CloneError),
    #[error(transparent)]
    ChildError(E),
    #[error("Failed to create namespace: {0}")]
    UserIdMapError(#[from] IdMapError<User>),
    #[error("Failed to create namespace: {0}")]
    GroupIdMapError(#[from] IdMapError<Group>),
    #[error("Error while using the message queue")]
    MsgQueue,
}
