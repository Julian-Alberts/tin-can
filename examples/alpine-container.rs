use std::process::{Command, Stdio};

use simplelog::*;
use tin_can::container::{
    step::{
        mount_namespace::{MountNamespace, MountOperation},
        run_command::RunCommand,
        switch_user::SwitchUser,
        user_namespace::UserNamespaceRoot,
    },
    ContainerBuilder, IdMap, User,
};

fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Trace,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )])
    .unwrap();

    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };

    let test_dir = std::path::PathBuf::from("test-data");

    let mut command = Command::new("/bin/ash");

    command
        .env("PATH", format!("/bin:/sbin/:/usr/bin:/usr/sbin/"))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    let container = ContainerBuilder::new(UserNamespaceRoot::new_with_current_user_as_root(
        MountNamespace::new(
            UserNamespaceRoot::new(
                IdMap::invert(IdMap::new_with_current_user_as_root()),
                IdMap::invert(IdMap::new_with_current_user_as_root()),
                Some((uid, gid)),
                RunCommand::new(command),
            )
            .unwrap(),
            vec![], /*
                                MountOperation::switch_root_with_overlay(
                                    &test_dir.join("alpine"),
                                    &test_dir.join("alpine-upper"),
                                    &test_dir.join("work"),
                                    &test_dir.join("root"),
                                    &std::path::PathBuf::from("put-old"),
                                ),
                    */
        ),
    ))
    .run()
    .unwrap()
    .join();
}
