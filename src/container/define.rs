use super::*;

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
