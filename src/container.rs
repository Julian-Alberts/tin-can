mod validate;

pub struct NotValidated;
impl NotCreated for NotValidated {}
impl ContainerState for NotValidated {}
pub struct Validated;
impl NotCreated for Validated {}
impl ContainerState for Validated {}
pub struct Created;
impl ContainerState for Created {}

trait NotCreated: ContainerState {}
trait ContainerState {}

#[derive(Debug, Default, PartialEq)]
pub struct Container<State: ContainerState> {
    user: Option<UserNamespace<State>>,
    mount: Option<MountNamespace<State>>,
    pid: bool,
    net: Option<Network>,
    _p: std::marker::PhantomData<State>,
}

impl<S: NotCreated> Container<S> {
    fn to_not_validated<S1: NotCreated>(
        Container {
            user,
            mount,
            pid,
            net,
            _p,
        }: Container<S1>,
    ) -> Container<NotValidated> {
        Container {
            user: user.map(UserNamespace::to_not_validated),
            mount: mount.map(MountNamespace::to_not_validated),
            pid,
            net,
            _p: Default::default(),
        }
    }

    pub fn create_user_namespace(
        self,
        user_namespace: UserNamespace<NotValidated>,
    ) -> Container<NotValidated> {
        Container {
            user: Some(user_namespace),
            ..Container::<S>::to_not_validated(self)
        }
    }
    pub fn create_mount_namespace(
        self,
        mount: MountNamespace<NotValidated>,
    ) -> Container<NotValidated> {
        Container {
            mount: Some(mount),
            ..Container::<S>::to_not_validated(self)
        }
    }
    pub fn create_pid_namespace(self) -> Container<NotValidated> {
        Container {
            pid: true,
            ..Container::<S>::to_not_validated(self)
        }
    }
    pub fn create_net_namespace(self, net: Network) -> Container<NotValidated> {
        Container {
            net: Some(net),
            ..Container::<S>::to_not_validated(self)
        }
    }
}

impl Container<NotValidated> {
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
