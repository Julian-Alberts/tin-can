use std::fmt::Debug;

use crate::{container::MapType, linux};

use super::{Component, Group, IdMap, InitComponent, User};

pub struct UserNamespaceRoot<C> {
    component: C,
    uid_map: IdMap<User>,
    gid_map: IdMap<Group>,
}

impl<C> UserNamespaceRoot<C> {
    pub fn new(uid_map: IdMap<User>, gid_map: IdMap<Group>, component: C) -> Self {
        Self {
            component,
            uid_map,
            gid_map,
        }
    }
}
impl<C> InitComponent for UserNamespaceRoot<C> where C: Component {}
impl<C> Component for UserNamespaceRoot<C>
where
    C: Component,
{
    type Error = BuildUserNamespaceRootError<C::Error>;
    type Ok = C::Ok;

    fn run(self) -> Result<Self::Ok, Self::Error> {
        let msg_queue_ctp = linux::EventFd::new().unwrap();
        let msg_queue_ptc = linux::EventFd::new().unwrap();
        let mut shared_data = SharedData {
            ret: None,
            component: Some(self.component),
            msg_queue_ctp: msg_queue_ctp.clone(),
            msg_queue_ptc: msg_queue_ptc.clone(),
        };
        let join_handle = linux::clone_vm_with_namespaces(
            libc::CLONE_NEWUSER,
            root_namespace_vm,
            &mut shared_data,
        )?;
        log::info!("PID: {}", join_handle.pid);
        // std::thread::sleep(std::time::Duration::new(60, 0));
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
        let Some(res) = shared_data.ret else {
            panic!("No return value");
        };
        res
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
    C: Component,
{
    ret: Option<Result<C::Ok, BuildUserNamespaceRootError<C::Error>>>,
    component: Option<C>,
    msg_queue_ctp: linux::EventFd<usize>,
    msg_queue_ptc: linux::EventFd<usize>,
}
fn root_namespace_vm<C>(data: &mut SharedData<C>) -> i32
where
    C: Component,
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
        .map_err(BuildUserNamespaceRootError::ChildError);
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
