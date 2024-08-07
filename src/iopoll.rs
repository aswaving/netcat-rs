//! Event loop wrapper around the [`poll`] function.
//!
use libc::{c_int, nfds_t, poll, pollfd, POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT, POLLPRI};
use std::fmt;
use std::io::Stdin;
use std::os::unix::io::{AsRawFd, RawFd};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Token(pub i32);

pub struct EventLoop {
    /// Registered file descriptors that are polled for I/O.
    pollfds: Vec<pollfd>,
    /// Timeout used in the [`poll`] call. (0: return immediately without blocking, -1: wait indefinitely.)
    timeout: c_int,
    /// Flag indicating whether the main loop is active, `run` sets `active` to `true`,
    /// `shutdown` sets `active` to false, eventually stopping the main loop.
    active: bool,
}

pub trait EventHandler {
    fn ready_for_io(
        &mut self,
        event_loop: &mut EventLoop,
        stream_id: Token,
        eventset: EventSet,
    ) -> std::io::Result<()>;
    fn error(&mut self, event_loop: &mut EventLoop, stream_id: Token);
    fn hangup(&mut self, event_loop: &mut EventLoop, stream_id: Token);
    fn not_valid(&mut self, event_loop: &mut EventLoop, stream_id: Token) {
        let Token(fd) = stream_id;
        event_loop.remove_fd(fd);
    }
    fn timeout(&mut self, event_loop: &mut EventLoop);
}

pub const TIMEOUT_INFINITE: c_int = -1;

#[derive(Copy, Clone)]
pub struct EventSet {
    events: i16,
}

impl EventSet {
    pub fn empty() -> EventSet {
        EventSet { events: 0 }
    }

    pub fn is_readable(self) -> bool {
        (self.events & POLLIN) == POLLIN || (self.events & POLLPRI) == POLLPRI
    }

    #[allow(unused)]
    pub fn is_high_prio_readable(self) -> bool {
        (self.events & POLLPRI) == POLLPRI
    }

    pub fn is_writable(self) -> bool {
        (self.events & POLLOUT) == POLLOUT
    }

    pub fn is_not_valid(self) -> bool {
        (self.events & POLLNVAL) == POLLNVAL
    }

    pub fn is_error(self) -> bool {
        (self.events & POLLERR) == POLLERR
    }

    pub fn is_hangup(self) -> bool {
        (self.events & POLLHUP) == POLLHUP
    }

    pub fn is_empty(self) -> bool {
        self.events == 0
    }
}

impl fmt::Display for EventSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut events_str = String::new();

        if (self.events & POLLIN) == POLLIN {
            events_str.push_str("POLLIN|");
        }
        if (self.events & POLLOUT) == POLLOUT {
            events_str.push_str("POLLOUT|");
        }
        if (self.events & POLLERR) == POLLERR {
            events_str.push_str("POLLERR|");
        }
        if (self.events & POLLHUP) == POLLHUP {
            events_str.push_str("POLLHUP|");
        }
        if (self.events & POLLNVAL) == POLLNVAL {
            events_str.push_str("POLLNVAL|");
        }
        if (self.events & POLLPRI) == POLLPRI {
            events_str.push_str("POLLPRI|");
        }
        if !events_str.is_empty() {
            let last_char_idx = events_str.len() - 1;
            events_str.remove(last_char_idx);
        }
        write!(formatter, "{events_str}")
    }
}

pub struct EventSetBuilder {
    eventset: EventSet,
}

impl EventSetBuilder {
    pub fn new() -> EventSetBuilder {
        EventSetBuilder {
            eventset: EventSet::empty(),
        }
    }

    pub fn finalize(self) -> EventSet {
        self.eventset
    }
    pub fn readable(mut self) -> EventSetBuilder {
        self.eventset.events |= POLLIN | POLLPRI;
        self
    }

    #[allow(unused)]
    pub fn writable(mut self) -> EventSetBuilder {
        self.eventset.events |= POLLOUT;
        self
    }

    #[allow(unused)]
    pub fn all(mut self) -> EventSetBuilder {
        self.eventset.events = POLLIN | POLLPRI | POLLOUT;
        self
    }
}

impl EventLoop {
    /// Create a new event loop (with infinite timeout)
    #[allow(unused)]
    pub fn new() -> EventLoop {
        EventLoop::new_with_timeout(None)
    }

    /// Create a new event loop with timeout.
    /// `None` implies the infinite timeout.
    /// Panics when timeout > i32::MAX.
    pub fn new_with_timeout(timeout: Option<u32>) -> EventLoop {
        let mut my_timeout = TIMEOUT_INFINITE;
        if let Some(timeout) = timeout {
            my_timeout = c_int::try_from(timeout).unwrap(); // TODO
        }
        EventLoop {
            pollfds: Vec::<pollfd>::new(),
            active: false,
            timeout: my_timeout,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn shutdown(&mut self) {
        self.active = false;
        trace!("Event loop shutdown requested");
    }

    pub fn remove_fd(&mut self, fd: RawFd) {
        let found = self.pollfds.iter().position(|pollfd| pollfd.fd == fd);
        if let Some(index) = found {
            self.pollfds.remove(index);
        } else {
            panic!();
        }
    }

    pub fn run(&mut self, event_handler: &mut dyn EventHandler) -> Result<(), String> {
        let pollfds_ptr = self.pollfds.as_mut_ptr();
        self.active = true;
        let result = Ok(());
        while self.is_active() {
            let mut remove_pollfds = Vec::<RawFd>::new();
            let poll_result = unsafe {
                poll(
                    pollfds_ptr,
                    nfds_t::try_from(self.pollfds.len()).unwrap(), // TODO
                    self.timeout,
                )
            };
            let mut shutdown_loop = false;
            match poll_result {
                -1 => {
                    // todo handle error, look at errno
                    trace!("Last I/O error {:?}", std::io::Error::last_os_error());
                    shutdown_loop = true;
                }
                0 => {
                    event_handler.timeout(self);
                }
                _ => {
                    trace!("poll_result={} descriptors ready for io", poll_result);
                    let mut triggered_events = Vec::<(RawFd, EventSet)>::new();
                    for pollfd in &mut self.pollfds {
                        let received_events = EventSet {
                            events: pollfd.revents,
                        };
                        if !received_events.is_empty() {
                            triggered_events.push((pollfd.fd, received_events));
                        }
                    }
                    for (fd, eventset) in triggered_events {
                        trace!("event {} on fd {}", eventset, fd);
                        if eventset.is_readable() || eventset.is_writable() {
                            if let Err(e) = event_handler.ready_for_io(self, Token(fd), eventset) {
                                return Err(e.to_string());
                            }
                        }
                        if eventset.is_hangup() {
                            event_handler.hangup(self, Token(fd));
                        }
                        if eventset.is_error() {
                            event_handler.error(self, Token(fd));
                        }
                        if eventset.is_not_valid() {
                            event_handler.not_valid(self, Token(fd));
                            remove_pollfds.push(fd);
                        }
                    }
                }
            }

            for fd in remove_pollfds {
                self.remove_fd(fd);
            }

            if shutdown_loop {
                trace!("Event loop is shut down");
                self.shutdown();
            }
        }
        result
    }

    /// Register filedescriptor with given event set.
    fn register_fd(&mut self, fd: RawFd, eventset: EventSet) {
        let pollfd = pollfd {
            fd,
            events: eventset.events,
            revents: 0,
        };
        self.pollfds.push(pollfd);
    }

    /// Unregister file descriptor
    fn unregister_fd(&mut self, fd: RawFd) {
        let found = self
            .pollfds
            .iter()
            .enumerate()
            .find(|&(_, pollfd)| pollfd.fd == fd)
            .map(|(i, _)| i);
        if let Some(index) = found {
            self.pollfds.remove(index);
        }
    }

    /// Register stdin for read events.
    pub fn register_stdin(&mut self, stdin_stream: &Stdin) -> Token {
        let eventset = EventSetBuilder::new().readable().finalize();
        self.register_fd(stdin_stream.as_raw_fd(), eventset);
        Token(0)
    }

    /// unregister the stdin (stop listening to I/O events from it.)
    pub fn unregister_stdin(&mut self) {
        self.unregister_fd(0);
    }

    pub fn register_read<T>(&mut self, io: &T) -> Token
    where
        T: AsRawFd,
    {
        let fd = io.as_raw_fd();
        self.register_fd(fd, EventSetBuilder::new().readable().finalize());
        Token(fd)
    }

    #[allow(unused)]
    pub fn register_write<T>(&mut self, io: &mut T) -> Token
    where
        T: AsRawFd,
    {
        let fd = io.as_raw_fd();
        self.register_fd(fd, EventSetBuilder::new().writable().finalize());
        Token(fd)
    }

    // pub fn register_read_write<T>(&mut self, io: &'a mut T)
    // where T: AsRawFd + Read + Write + 'a
    // {
    // let fd = io.as_raw_fd();
    // self.register_fd(fd, EventSetBuilder::new().all().finalize());
    // }
}
