use std::process::{Command, Stdio};

use simplelog::*;
use tin_can::container::{
    step::{
        mount_namespace::{MountNamespace, MountOperation},
        pid_namespace::PIDNamespace,
        run_command::RunCommand,
        switch_working_directory::SwitchWorkingDirectory,
        user_namespace::UserNamespaceRoot,
    },
    ContainerBuilder, IdMap,
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
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit());

    command
        .env("PATH", format!("/bin:/sbin/:/usr/bin:/usr/sbin/"))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    let container = ContainerBuilder::new(UserNamespaceRoot::new_with_current_user_as_root(
        PIDNamespace::new(MountNamespace::new(
            MountOperation::switch_root_with_overlay(
                &test_dir.join("alpine"),
                &test_dir.join("alpine-upper"),
                &test_dir.join("work"),
                &test_dir.join("root"),
                &std::path::PathBuf::from("put-old"),
            ),
            SwitchWorkingDirectory::new(
                "/".into(),
                /*UserNamespaceRoot::new(
                IdMap::invert(IdMap::new_with_current_user_as_root()),
                IdMap::invert(IdMap::new_with_current_user_as_root()),
                Some((uid, gid)),*/
                RunCommand::new(command),
                /*)
                .unwrap(),*/
            ),
        )),
    ))
    .run()
    .unwrap()
    .join()
    .unwrap()
    .unwrap();
}
