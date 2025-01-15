pub mod mount_namespace;
pub mod pid_namespace;
pub mod run_command;
pub mod switch_user;
pub mod switch_working_directory;
pub mod user_namespace;

pub trait Step {
    type Error: std::error::Error;
    type Handle: StepHandle;
    fn run(self) -> Result<Self::Handle, Self::Error>;
}

pub trait StepHandle {
    type Error: std::error::Error;
    type Ok;
    fn join(self) -> Result<Self::Ok, Self::Error>;
}
