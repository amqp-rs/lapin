use crate::Result;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "tokio")] {
        pub fn default_runtime() -> Result<async_rs::TokioRuntime> {
            cfg_if! {
                if #[cfg(test)] {
                    Ok(async_rs::Runtime::tokio()?)
                } else {
                    Ok(async_rs::Runtime::tokio_current())
                }
            }
        }
    } else if #[cfg(feature = "async-global-executor")] {
        pub fn default_runtime() -> Result<async_rs::AGERuntime> {
            Ok(async_rs::Runtime::async_global_executor())
        }
    } else if #[cfg(feature = "smol")] {
        pub fn default_runtime() -> Result<async_rs::SmolRuntime> {
            Ok(async_rs::Runtime::smol())
        }
    } else {
        pub fn default_runtime() -> Result<async_rs::NoopRuntime> {
            Err(crate::ErrorKind::NoDefaultRuntime.into())
        }
    }
}
