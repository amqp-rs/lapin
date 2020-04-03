use std::ops::Deref;

#[derive(Debug)]
pub struct CloseOnDrop<T: __private::Closable> {
    inner: Option<T>,
}

impl<T: __private::Closable> CloseOnDrop<T> {
    pub fn new(inner: T) -> Self {
        Self { inner: Some(inner) }
    }

    /// Give the underlying object, cancelling the close on drop action
    pub fn into_inner(mut self) -> T {
        self.inner.take().expect("inner should only be None once consumed or dropped")
    }
}

impl<T: __private::Closable> Deref for CloseOnDrop<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("inner should only be None once consumed or dropped")
    }
}

impl<T: __private::Closable> Drop for CloseOnDrop<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.as_ref() {
            inner.close();
        }
    }
}

pub(crate) mod __private {
    pub trait Closable {
        fn close(&self);
    }
}
