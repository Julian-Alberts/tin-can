use crate::container::{step::Step, Context};

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
    fn run(mut self, _: &mut Context) -> Result<(), Self::Error> {
        log::info!(
            "Started run command ${:?} {:?}",
            self.command.get_program(),
            self.command.get_args()
        );
        self.command.spawn()?.wait()?;
        Ok(())
    }
}
