use super::Context;

pub mod mount_namespace;
pub mod pid_namespace;
pub mod run_command;
pub mod switch_user;
pub mod switch_working_directory;
pub mod user_namespace;

pub trait Step {
    type Error: std::error::Error;
    fn run(self, ctx: &mut Context) -> Result<(), Self::Error>;
}
