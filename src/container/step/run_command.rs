use crate::container::Step;

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
    type Ok = std::process::ExitStatus;
    fn run(mut self) -> Result<Self::Ok, Self::Error> {
        self.command.spawn()?.wait()
    }
}
