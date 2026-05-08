use crate::Result;
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "tokio")] {
        type DefaultRuntime = async_rs::TokioRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Tokio;

        pub fn default_runtime() -> Result<DefaultRuntime> {
            cfg_if! {
                if #[cfg(test)] {
                    Ok(async_rs::Runtime::tokio()?)
                } else {
                    Ok(async_rs::Runtime::tokio_current())
                }
            }
        }
    } else if #[cfg(feature = "async-global-executor")] {
        type DefaultRuntime = async_rs::AGERuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::util::RuntimeParts<async_rs::AsyncGlobalExecutor, async_rs::AsyncIO>;

        pub fn default_runtime() -> Result<DefaultRuntime> {
            Ok(async_rs::Runtime::async_global_executor())
        }
    } else if #[cfg(feature = "smol")] {
        type DefaultRuntime = async_rs::SmolRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Smol;

        pub fn default_runtime() -> Result<DefaultRuntime> {
            Ok(async_rs::Runtime::smol())
        }
    } else {
        type DefaultRuntime = async_rs::NoopRuntime;
        pub(crate) type DefaultRuntimeKit = async_rs::Noop;

        pub fn default_runtime() -> Result<DefaultRuntime> {
            Err(crate::ErrorKind::NoDefaultRuntime.into())
        }
    }
}
