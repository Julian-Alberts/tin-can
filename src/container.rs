mod create;
mod define;
mod validate;

#[derive(Debug)]
pub struct NotValidated;
impl NotCreated for NotValidated {}
impl ContainerState for NotValidated {}
#[derive(Debug)]
pub struct Validated;
impl NotCreated for Validated {}
impl ContainerState for Validated {}
#[derive(Debug)]
pub struct Created;
impl ContainerState for Created {}

pub trait NotCreated: ContainerState {}
pub trait ContainerState {}

#[derive(Debug, Default, PartialEq)]
pub struct Container<State: ContainerState> {
    user: Option<UserNamespace<State>>,
    mount: Option<MountNamespace<State>>,
    pid: bool,
    net: Option<Network>,
    _p: std::marker::PhantomData<State>,
}

impl Container<NotValidated> {
    pub fn new() -> Self {
        Self {
            user: None,
            mount: None,
            pid: false,
            net: None,
            _p: std::marker::PhantomData,
        }
    }

    pub fn validate(
        Self {
            user,
            mount,
            pid,
            net,
            _p,
        }: Self,
    ) -> Result<Container<Validated>, validate::ValidationError> {
        Ok(Container {
            mount: mount
                .map(|m| validate::mount(m, user.as_ref()))
                .transpose()?,
            user: user.map(validate::user).transpose()?,
            pid,
            net,
            _p: Default::default(),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct UserNamespace<S: ContainerState> {
    uid_map: Vec<IdMap>,
    gid_map: Vec<IdMap>,
    run_as: (libc::uid_t, libc::gid_t),
    _p: std::marker::PhantomData<S>,
}

impl UserNamespace<NotValidated> {
    pub fn new(
        uid_map: Vec<IdMap>,
        gid_map: Vec<IdMap>,
        run_as: (libc::uid_t, libc::gid_t),
    ) -> Self {
        Self {
            uid_map,
            gid_map,
            run_as,
            _p: std::marker::PhantomData,
        }
    }
}

impl<V: NotCreated> UserNamespace<V> {
    fn to_not_validated(
        UserNamespace {
            uid_map,
            gid_map,
            run_as,
            _p,
        }: UserNamespace<V>,
    ) -> UserNamespace<NotValidated> {
        UserNamespace {
            uid_map,
            gid_map,
            run_as,
            _p: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MountNamespace<S: ContainerState> {
    mounts: Vec<MountPoint>,
    pivot_root: Option<PivotRoot>,
    _p: std::marker::PhantomData<S>,
}

impl<V: NotCreated> MountNamespace<V> {
    fn to_not_validated(
        MountNamespace {
            mounts,
            _p,
            pivot_root,
        }: MountNamespace<V>,
    ) -> MountNamespace<NotValidated> {
        MountNamespace {
            mounts,
            pivot_root,
            _p: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MountPoint {
    Overlay,
}

#[derive(Debug, PartialEq)]
pub struct PivotRoot {}

#[derive(Debug, PartialEq)]
pub struct IdMap {
    start_intern: u32,
    start_extern: u32,
    len: u32,
}
impl From<(u32, u32, u32)> for IdMap {
    fn from(value: (u32, u32, u32)) -> Self {
        Self {
            start_intern: value.0,
            start_extern: value.1,
            len: value.2,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Network {
    interfaces: Vec<Interface>,
}

#[derive(Debug, PartialEq)]
pub struct Interface {
    name: String,
    interface_type: InterfaceType,
}

#[derive(Debug, PartialEq)]
pub enum InterfaceType {}
