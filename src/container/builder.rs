use super::{Component, InitComponent};

pub struct RunCommand {
    command: std::process::Command,
}

impl RunCommand {
    pub fn new(command: std::process::Command) -> Self {
        Self { command }
    }
}

impl Component for RunCommand {
    type Error = std::io::Error;
    type Ok = std::process::ExitStatus;
    fn run(mut self) -> Result<Self::Ok, Self::Error> {
        self.command.spawn()?.wait()
    }
}

pub struct ContainerBuilder<C> {
    component: C,
}

impl<C> ContainerBuilder<C>
where
    C: InitComponent,
{
    pub fn new(component: C) -> Self {
        Self { component }
    }
}

impl<C> ContainerBuilder<C>
where
    C: Component,
{
    pub fn run(self) -> Result<C::Ok, C::Error> {
        self.component.run()
    }
}
