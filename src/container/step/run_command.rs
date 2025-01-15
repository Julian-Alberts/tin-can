use crate::container::step::{Step, StepHandle};

pub struct RunCommand {
    command: std::process::Command,
}

impl RunCommand {
    pub fn new(command: std::process::Command) -> Self {
        Self { command }
    }
}

impl Step for RunCommand {
    type Error = std::io::Error;
    type Handle = RunCommandHandle;
    fn run(mut self) -> Result<Self::Handle, Self::Error> {
        log::info!(
            "Started run command ${:?} {:?}",
            self.command.get_program(),
            self.command.get_args()
        );
        Ok(RunCommandHandle(self.command.spawn()?))
    }
}

pub struct RunCommandHandle(std::process::Child);
impl StepHandle for RunCommandHandle {
    type Error = std::io::Error;

    type Ok = std::process::ExitStatus;

    fn join(mut self) -> Result<Self::Ok, Self::Error> {
        log::info!("Waiting for command to finish");
        let res = self.0.try_wait().map(|status| status.unwrap());
        log::info!("Command finished with {res:?}");
        res
    }
}
