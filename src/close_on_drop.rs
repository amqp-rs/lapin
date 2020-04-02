use std::ops::Deref;

#[derive(Debug)]
pub struct CloseOnDrop<T: __private::Closable> {
    inner: T,
}

impl<T: __private::Closable> CloseOnDrop<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: __private::Closable> Deref for CloseOnDrop<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: __private::Closable> Drop for CloseOnDrop<T> {
    fn drop(&mut self) {
        self.close();
    }
}

pub(crate) mod __private {
    pub trait Closable {
        fn close(&self);
    }
}
