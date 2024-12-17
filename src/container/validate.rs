use super::*;

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error(transparent)]
    UserValidationError(#[from] UserValidationError),
    #[error(transparent)]
    MountValidationError(#[from] MountValidationError),
}

#[derive(Debug, thiserror::Error)]
pub enum UserValidationError {
    #[error("The group id is not mapped")]
    RunAsGidNotMapped,
    #[error("The user id is not mapped")]
    RunAsUidNotMapped,
}

#[derive(Debug, thiserror::Error)]
pub enum MountValidationError {
    #[error("Root user is not mapped")]
    RootNotMapped,
}

pub fn user<V: NotCreated>(
    user: UserNamespace<V>,
) -> Result<UserNamespace<Validated>, UserValidationError> {
    let (uid, gid) = user.run_as;
    if !user
        .uid_map
        .iter()
        .any(|map| (map.start_intern..(map.start_intern + map.len)).contains(&uid))
    {
        return Err(UserValidationError::RunAsUidNotMapped);
    }
    if !user
        .gid_map
        .iter()
        .any(|map| (map.start_intern..(map.start_intern + map.len)).contains(&gid))
    {
        return Err(UserValidationError::RunAsGidNotMapped);
    }
    Ok(UserNamespace {
        uid_map: user.uid_map,
        gid_map: user.gid_map,
        run_as: user.run_as,
        _p: Default::default(),
    })
}

pub(crate) fn mount<V: NotCreated>(
    mount: MountNamespace<V>,
    user: Option<&UserNamespace<NotValidated>>,
) -> Result<MountNamespace<Validated>, MountValidationError> {
    let Some(user) = user else {
        return Err(MountValidationError::RootNotMapped);
    };
    if !user.uid_map.iter().any(|map| map.start_intern == 0) {
        return Err(MountValidationError::RootNotMapped);
    }
    Ok(MountNamespace {
        mounts: mount.mounts,
        pivot_root: mount.pivot_root,
        _p: std::marker::PhantomData,
    })
}
