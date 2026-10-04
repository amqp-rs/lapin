use crate::Result;
use cfg_if::cfg_if;

macro_rules! decl_default_runtime {
    ($body:expr) => {
        /// Return the default [`async_rs::Runtime`] for the active runtime feature.
        ///
        /// The exact type depends on which runtime feature flag is enabled:
        /// `tokio` (default), `smol`, or `async-global-executor`.
        ///
        /// Returns [`ErrorKind::NoDefaultRuntime`] if no runtime feature is active.
        ///
        /// [`ErrorKind::NoDefaultRuntime`]: crate::ErrorKind::NoDefaultRuntime
        pub fn default_runtime() -> Result<DefaultRuntime> {
            $body
        }
    };
}

cfg_if! {
    if #[cfg(feature = "tokio")] {
        type DefaultRuntime = async_rs::TokioRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Tokio;
        decl_default_runtime! {
            {
                #[cfg(test)]
                return Ok(async_rs::Runtime::tokio()?);
                #[cfg(not(test))]
                return Ok(async_rs::Runtime::tokio_current());
            }
        }
    } else if #[cfg(feature = "async-global-executor")] {
        type DefaultRuntime = async_rs::AGERuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::util::RuntimeParts<async_rs::AsyncGlobalExecutor, async_rs::AsyncIO>;
        decl_default_runtime! { Ok(async_rs::Runtime::async_global_executor()) }
    } else if #[cfg(feature = "smol")] {
        type DefaultRuntime = async_rs::SmolRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Smol;
        decl_default_runtime! { Ok(async_rs::Runtime::smol()) }
    } else {
        type DefaultRuntime = async_rs::NoopRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Noop;
        decl_default_runtime! { Err(crate::ErrorKind::NoDefaultRuntime.into()) }
    }
}

/// Poll `first`, then `second`, and return the output of whichever completes
/// first.
pub(crate) async fn race<T>(first: impl Future<Output = T>, second: impl Future<Output = T>) -> T {
    let mut first = std::pin::pin!(first);
    let mut second = std::pin::pin!(second);
    std::future::poll_fn(|cx| {
        if let std::task::Poll::Ready(output) = first.as_mut().poll(cx) {
            return std::task::Poll::Ready(output);
        }
        second.as_mut().poll(cx)
    })
    .await
}
