use reactor_trait::{TcpReactor, TimeReactor};

pub trait FullReactor: TcpReactor + TimeReactor {}
impl<R: TcpReactor + TimeReactor> FullReactor for R {}
