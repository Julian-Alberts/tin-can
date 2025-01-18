use crate::container::step::Step;

use super::{Container, Context};

pub struct ContainerBuilder<C> {
    component: C,
}

impl<C> ContainerBuilder<C>
where
    C: Step,
{
    pub fn new(component: C) -> Self {
        Self { component }
    }
}

impl<C> ContainerBuilder<C>
where
    C: Step,
{
    pub fn run(self) -> Result<(), C::Error> {
        let mut ctx = Context::default();
        self.component.run(&mut ctx)?;
        Ok(Container { ctx })
    }
}
