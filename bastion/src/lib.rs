use lapin::ConnectionProperties;

pub trait BastionExt {
    fn with_bastion(self) -> Self
    where
        Self: Sized,
    {
        self.with_bastion_executor()
    }

    fn with_bastion_executor(self) -> Self
    where
        Self: Sized;
}

impl BastionExt for ConnectionProperties {
    fn with_bastion_executor(self) -> Self {
        self.with_executor(bastion_executor_trait::Bastion)
    }
}
