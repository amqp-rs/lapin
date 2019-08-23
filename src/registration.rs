use mio::{self, Evented, Poll, PollOpt, Ready, SetReadiness, Token};
use parking_lot::Mutex;
use std::{fmt, io, sync::Arc};

#[derive(Clone)]
pub(crate) struct Registration {
    registration: Arc<Mutex<mio::Registration>>,
    set_readiness: SetReadiness,
}

impl Registration {
    pub(crate) fn set_readiness(&self, ready: Ready) -> io::Result<()> {
        self.set_readiness.set_readiness(ready)
    }
}

impl Default for Registration {
    fn default() -> Self {
        let (registration, set_readiness) = mio::Registration::new2();
        Self {
            registration: Arc::new(Mutex::new(registration)),
            set_readiness,
        }
    }
}

impl Evented for Registration {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration
            .lock()
            .register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration
            .lock()
            .reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        poll.deregister(&*self.registration.lock())
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Registration")
    }
}
