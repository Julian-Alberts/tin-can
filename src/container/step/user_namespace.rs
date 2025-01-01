use std::fmt::Debug;

use crate::{container::MapType, linux};

use crate::container::{Group, IdMap, Step, User};

pub struct UserNamespaceRoot<S> {
    next_step: S,
    uid_map: IdMap<User>,
    gid_map: IdMap<Group>,
}

impl<S> UserNamespaceRoot<S> {
    pub fn new_with_current_user_as_root(next_step: S) -> Self {
        Self {
            next_step,
            uid_map: IdMap::new_with_current_user_as_root(),
            gid_map: IdMap::new_with_current_user_as_root(),
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
        })
    }
}
impl<C> Step for UserNamespaceRoot<C>
where
    C: Step,
{
    type Error = BuildUserNamespaceRootError<C::Error>;
    type Ok = ();

    fn run(self) -> Result<Self::Ok, Self::Error> {
        let msg_queue_ctp = linux::EventFd::new().unwrap();
        let msg_queue_ptc = linux::EventFd::new().unwrap();
        let mut shared_data = SharedData {
            ret: None,
            component: Some(self.next_step),
            msg_queue_ctp: msg_queue_ctp.clone(),
            msg_queue_ptc: msg_queue_ptc.clone(),
        };
        let join_handle = linux::clone_vm_with_namespaces(
            libc::CLONE_NEWUSER,
            root_namespace_vm,
            &mut shared_data,
        )?;
        log::info!("PID: {}", join_handle.pid);
        fn write_id_map<T: MapType>(map: IdMap<T>, pid: libc::pid_t) -> Result<(), IdMapError<T>> {
            log::debug!("Creating {} for process {pid}", T::file());
            use std::io::Write as _;
            T::prepare_process(pid)?;
            let mut path = std::path::PathBuf::from("/proc/");
            path.push(pid.to_string());
            path.push(T::file());
            let mut file = std::fs::File::create(path)?;
            let mut buf = Vec::new();
            for entry in map.entries {
                log::debug!("{} {} {}", entry.internal, entry.external, entry.len);
                write!(buf, "{} {} {}\n", entry.internal, entry.external, entry.len)?;
            }
            file.write_all(buf.as_slice())?;
            Ok(())
        }
        msg_queue_ctp.receive().unwrap();
        write_id_map(self.uid_map, join_handle.pid)?;
        write_id_map(self.gid_map, join_handle.pid)?;
        msg_queue_ptc.send(1).unwrap();
        // shared_data.ret can only be assumed to be set after the child has finished
        join_handle.join();
        //let Some(res) = shared_data.ret else {
        //    panic!("No return value");
        //};
        Ok(())
    }
}

#[derive(thiserror::Error)]
pub struct IdMapError<T: MapType> {
    error: std::io::Error,
    _p: std::marker::PhantomData<T>,
}

impl<T> From<std::io::Error> for IdMapError<T>
where
    T: MapType,
{
    fn from(error: std::io::Error) -> Self {
        Self {
            error,
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

struct SharedData<C>
where
    C: Step,
{
    ret: Option<Result<C::Ok, BuildUserNamespaceRootError<C::Error>>>,
    component: Option<C>,
    msg_queue_ctp: linux::EventFd<usize>,
    msg_queue_ptc: linux::EventFd<usize>,
}
fn root_namespace_vm<C>(data: &mut SharedData<C>) -> i32
where
    C: Step,
{
    log::debug!("root namespace main");
    data.msg_queue_ctp.send(1).unwrap();
    data.msg_queue_ptc.receive().unwrap();
    log::debug!("Namespace resumed");
    linux::switch_user((0, 0)).unwrap();
    let res = data
        .component
        .take()
        .expect("Component called twice")
        .run()
        .map_err(BuildUserNamespaceRootError::ChildError)
        .inspect_err(|e| log::error!("{e}"));
    res.as_ref().unwrap();
    data.ret = Some(res);
    0
}

#[derive(Debug, thiserror::Error)]
pub enum BuildUserNamespaceRootError<C: std::error::Error> {
    #[error(transparent)]
    CloneError(#[from] linux::CloneError),
    #[error(transparent)]
    ChildError(C),
    #[error("Failed to create namespace: {0}")]
    UserIdMapError(#[from] IdMapError<User>),
    #[error("Failed to create namespace: {0}")]
    GroupIdMapError(#[from] IdMapError<Group>),
}
