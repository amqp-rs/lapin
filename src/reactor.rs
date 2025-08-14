use reactor_trait::{Reactor, TcpReactor, TimeReactor};

pub trait FullReactor: Reactor + TcpReactor + TimeReactor {}
impl<R: Reactor + TcpReactor + TimeReactor> FullReactor for R {}
