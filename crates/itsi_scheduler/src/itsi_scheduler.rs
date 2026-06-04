mod io_helpers;
mod io_waiter;
mod timer;
use io_helpers::{build_interest, poll_readiness, set_nonblocking};
use io_waiter::IoWaiter;
use itsi_error::ItsiError;
use itsi_rb_helpers::call_without_gvl;
use magnus::{error::Result as MagnusResult, Ruby};
use mio::{Events, Poll, Token, Waker};
use parking_lot::Mutex;
use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    ffi::CString,
    os::fd::RawFd,
    ptr,
    time::Duration,
};
use timer::Timer;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Readiness(i16);

impl std::fmt::Debug for ItsiScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItsiScheduler").finish()
    }
}

const WAKE_TOKEN: Token = Token(0);

#[magnus::wrap(class = "Itsi::Scheduler", free_immediately, size)]
pub(crate) struct ItsiScheduler {
    timers: Mutex<BinaryHeap<Timer>>,
    io_waiters: Mutex<HashMap<Token, IoWaiter>>,
    registry: Mutex<HashMap<RawFd, VecDeque<IoWaiter>>>,
    poll: Mutex<Poll>,
    events: Mutex<Events>,
    waker: Mutex<Waker>,
}

impl Default for ItsiScheduler {
    fn default() -> Self {
        let poll = Poll::new().unwrap();
        let waker = Waker::new(poll.registry(), WAKE_TOKEN).unwrap();
        let events = Events::with_capacity(1024);

        ItsiScheduler {
            timers: Mutex::new(BinaryHeap::new()),
            io_waiters: Mutex::new(HashMap::new()),
            registry: Mutex::new(HashMap::new()),
            poll: Mutex::new(poll),
            events: Mutex::new(events),
            waker: Mutex::new(waker),
        }
    }
}

impl ItsiScheduler {
    pub fn initialize(&self) {}

    pub fn wake(&self) -> MagnusResult<()> {
        self.waker.lock().wake().map_err(|_| {
            magnus::Error::new(
                magnus::Ruby::get().unwrap().exception_standard_error(),
                "Failed to wake the scheduler",
            )
        })?;
        Ok(())
    }
    pub fn register_io_wait(
        &self,
        io_obj: i32,
        events: i16,
        timeout: Option<f64>,
        token: usize,
    ) -> MagnusResult<Option<i16>> {
        debug!(
            "Registering IO Wait for {:?}, {:?}, {:?}, {:?}",
            io_obj, events, timeout, token
        );
        let fd: RawFd = io_obj;

        let readiness = poll_readiness(fd, events).unwrap_or(Readiness(0));
        if readiness == Readiness(events) {
            return Ok(Some(readiness.0));
        }

        set_nonblocking(fd)?;
        let interest = build_interest(events)?;
        let token = Token(token);
        let mut waiter = IoWaiter::new(fd, events, token);
        self.io_waiters.lock().insert(token, waiter.clone());
        let mut binding = self.registry.lock();
        let queue = binding.entry(fd).or_default();

        queue.push_back(waiter.clone());

        if queue.len() == 1 {
            self.poll
                .lock()
                .registry()
                .register(&mut waiter, token, interest)
                .map_err(|e| ItsiError::ArgumentError(format!("register error: {}", e)))?;
        }
        Ok(None)
    }

    pub fn start_timer(&self, timeout: Option<f64>, token: usize) {
        if timeout.is_some_and(|t| t >= 0.0) {
            let timer_entry = Timer::new(Duration::from_secs_f64(timeout.unwrap()), Token(token));
            self.timers.lock().push(timer_entry);
        }
    }

    pub fn clear_timer(&self, token: usize) {
        self.timers.lock().retain(|timer| timer.token.0 != token);
    }

    pub fn cancel_wait(&self, token: usize) -> MagnusResult<()> {
        let token = Token(token);

        self.timers.lock().retain(|timer| timer.token != token);

        let mut io_waiters = self.io_waiters.lock();
        let Some(mut waiter) = io_waiters.remove(&token) else {
            return Ok(());
        };

        let mut registry = self.registry.lock();
        let Some(queue) = registry.get_mut(&waiter.fd) else {
            return Ok(());
        };

        let Some(position) = queue.iter().position(|entry| entry.token == token) else {
            return Ok(());
        };

        if position == 0 {
            self.poll
                .lock()
                .registry()
                .deregister(&mut waiter)
                .map_err(|_| {
                    ItsiError::ArgumentError("Failed to deregister".to_string())
                })?;
        }

        queue.remove(position);

        if position == 0 {
            if let Some(head) = queue.get_mut(0) {
                let interest = build_interest(head.readiness)?;
                self.poll
                    .lock()
                    .registry()
                    .register(head, head.token, interest)
                    .map_err(|_| {
                        ItsiError::ArgumentError("Failed to register".to_string())
                    })?;
            }
        }

        if queue.is_empty() {
            registry.remove(&waiter.fd);
        }

        Ok(())
    }

    pub fn has_pending_io(&self) -> bool {
        !self.timers.lock().is_empty() || !self.io_waiters.lock().is_empty()
    }

    pub fn class_info(msg: String) {
        info!(msg);
    }

    pub fn info(&self, msg: String) {
        info!(msg);
    }

    pub fn warn(&self, msg: String) {
        warn!(msg);
    }

    pub fn debug(&self, msg: String) {
        debug!(msg);
    }

    pub fn fetch_due_events(&self) -> MagnusResult<Option<Vec<(usize, i16)>>> {
        call_without_gvl(|| {
            let timeout = if let Some(timer) = self.timers.lock().peek() {
                timer.duration().or(Some(Duration::ZERO))
            } else {
                None
            };
            let mut due_fibers: Option<Vec<(usize, i16)>> = None;
            let mut io_waiters = self.io_waiters.lock();
            if !io_waiters.is_empty() || timeout.is_none() {
                let mut events = self.events.lock();
                {
                    let mut poll = self.poll.lock();
                    if let Err(_err) = poll.poll(&mut events, timeout) {
                        return Ok(due_fibers);
                    }
                };

                for event in events.iter() {
                    let token = event.token();
                    if token == WAKE_TOKEN {
                        continue;
                    }

                    let waiter = io_waiters.remove(&token);
                    if waiter.is_none() {
                        continue;
                    }
                    let mut waiter = waiter.unwrap();
                    let mut evt_readiness = 0;
                    if event.is_readable() {
                        evt_readiness |= 1;
                    }
                    if event.is_priority() {
                        evt_readiness |= 2;
                    }
                    if event.is_writable() {
                        evt_readiness |= 4
                    }
                    self.poll
                        .lock()
                        .registry()
                        .deregister(&mut waiter)
                        .map_err(|_| {
                            ItsiError::ArgumentError("Failed to deregister".to_string())
                        })?;

                    due_fibers
                        .get_or_insert_default()
                        .push((waiter.token.0, evt_readiness));

                    let mut binding = self.registry.lock();
                    // Pop the current item for the current waiter off the queue
                    let queue = binding.get_mut(&(waiter.fd)).unwrap();
                    queue.pop_front();

                    if let Some(head) = queue.get_mut(0) {
                        // Register the next item in the queue if there is one.
                        let interest = build_interest(head.readiness)?;
                        self.poll
                            .lock()
                            .registry()
                            .register(head, head.token, interest)
                            .map_err(|_| {
                                ItsiError::ArgumentError("Failed to deregister".to_string())
                            })?;
                    } else {
                        // Otherwise we drop the queue altogether.
                        binding.remove(&waiter.fd);
                    }
                }
                return Ok(due_fibers);
            }
            Ok(None)
        })
    }

    pub fn address_resolve(
        _ruby: &Ruby,
        _rself: &Self,
        hostname: String,
    ) -> MagnusResult<Option<Vec<String>>> {
        let result: Option<Vec<String>> = call_without_gvl(move || {
            let hostname = CString::new(hostname).ok()?;
            let hints = nix::libc::addrinfo {
                ai_flags: 0,
                ai_family: nix::libc::AF_UNSPEC,
                ai_socktype: nix::libc::SOCK_STREAM,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_addr: ptr::null_mut(),
                ai_canonname: ptr::null_mut(),
                ai_next: ptr::null_mut(),
            };
            let mut res: *mut nix::libc::addrinfo = ptr::null_mut();
            let rc = unsafe {
                nix::libc::getaddrinfo(hostname.as_ptr(), ptr::null(), &hints, &mut res)
            };
            if rc != 0 || res.is_null() {
                return None;
            }

            let mut ips = Vec::new();
            let mut current = res;
            while !current.is_null() {
                let ai = unsafe { &*current };
                if !ai.ai_addr.is_null() {
                    match ai.ai_family {
                        nix::libc::AF_INET => {
                            let addr = unsafe {
                                &*(ai.ai_addr as *const nix::libc::sockaddr_in)
                            };
                            let ip = std::net::Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
                            ips.push(ip.to_string());
                        }
                        nix::libc::AF_INET6 => {
                            let addr = unsafe {
                                &*(ai.ai_addr as *const nix::libc::sockaddr_in6)
                            };
                            let ip = std::net::Ipv6Addr::from(addr.sin6_addr.s6_addr);
                            ips.push(ip.to_string());
                        }
                        _ => {}
                    }
                }
                current = ai.ai_next;
            }

            unsafe {
                nix::libc::freeaddrinfo(res);
            }

            if ips.is_empty() {
                None
            } else {
                ips.sort();
                ips.dedup();
                Some(ips)
            }
        });
        Ok(result)
    }

    pub fn fetch_due_timers(&self) -> MagnusResult<Option<Vec<usize>>> {
        call_without_gvl(|| {
            let mut timers = self.timers.lock();
            let mut io_waiters = self.io_waiters.lock();
            let mut due_fibers: Option<Vec<usize>> = None;
            while let Some(timer) = timers.peek() {
                if timer.is_due() {
                    due_fibers.get_or_insert_default().push(timer.token.0);
                    if let Some(waiter) = io_waiters.remove(&timer.token) {
                        let mut binding = self.registry.lock();
                        // Pop the current item for the current waiter off the queue
                        let queue = binding.get_mut(&waiter.fd).unwrap();
                        queue.pop_front();

                        if let Some(head) = queue.get_mut(0) {
                            // Register the next item in the queue if there is one.
                            let interest = build_interest(head.readiness)?;
                            self.poll
                                .lock()
                                .registry()
                                .register(head, head.token, interest)
                                .map_err(|_| {
                                    ItsiError::ArgumentError("Failed to deregister".to_string())
                                })?;
                        } else {
                            // Otherwise we drop the queue altogether.
                            binding.remove(&waiter.fd);
                        }
                    }
                    timers.pop();
                } else {
                    break;
                }
            }
            Ok(due_fibers)
        })
    }
}
