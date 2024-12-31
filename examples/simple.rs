use std::process::{Command, Stdio};

use simplelog::*;
use tin_can::container::{
    step::{
        mount_namespace::MountNamespace, run_command::RunCommand, user_namespace::UserNamespaceRoot,
    },
    ContainerBuilder, IdMap,
};

fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )])
    .unwrap();

    let mut command = Command::new("bash");
    command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    ContainerBuilder::new(UserNamespaceRoot::new(
        IdMap::new_with_current_user_as_root(),
        IdMap::new_with_current_user_as_root(),
        MountNamespace::new(RunCommand::new(command), vec![]),
    ))
    .run()
    .unwrap();
}
