use lapin::ConnectionProperties;

#[deprecated(note = "use bastion-executor-trait directly instead")]
pub trait BastionExt {
    #[deprecated(note = "use bastion-executor-trait directly instead")]
    fn with_bastion(self) -> Self
    where
        Self: Sized,
    {
        self.with_bastion_executor()
    }

    #[deprecated(note = "use bastion-executor-trait directly instead")]
    fn with_bastion_executor(self) -> Self
    where
        Self: Sized;
}

impl BastionExt for ConnectionProperties {
    fn with_bastion_executor(self) -> Self {
        self.with_executor(bastion_executor_trait::Bastion)
    }
}
