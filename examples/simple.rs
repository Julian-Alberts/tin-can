use std::{io::Read, process::Stdio};

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
    Container::<Validated>::create(container, || {
        let cmd = std::process::Command::new("whoami")
            .stdout(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .unwrap();
        println!("{:?}", String::from_utf8(cmd.stdout));
        cmd.status.code().unwrap_or(i32::MAX)
    })
    .join();
}
