use super::Step;

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
    pub fn run(self) -> Result<C::Ok, C::Error> {
        self.component.run()
    }
}
