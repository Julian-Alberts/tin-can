use tin_can::container::{Container, IdMap, NotValidated, UserNamespace, Validated};

fn main() {
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let container: Container<NotValidated> = tin_can::container::Container::new()
        .create_user_namespace(UserNamespace::new(
            vec![(0, uid, 1).into()],
            vec![(0, gid, 1).into()],
            (0, 0),
        ));
    let container = Container::<NotValidated>::validate(container).unwrap();
    Container::<Validated>::create(container, || std::process::Command::new("whoami"));
}
