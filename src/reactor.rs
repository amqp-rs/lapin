use reactor_trait::{Reactor, TimeReactor};

pub trait FullReactor: Reactor + TimeReactor {}
impl<R: Reactor + TimeReactor> FullReactor for R {}
