mod io_helpers;
mod io_waiter;
mod timer;
use io_helpers::{build_interest, poll_readiness, set_nonblocking};
use io_waiter::IoWaiter;
use itsi_error::ItsiError;
use itsi_rb_helpers::{call_without_gvl, create_ruby_thread};
use magnus::{
    error::Result as MagnusResult,
    value::{InnerValue, Opaque, ReprValue},
    Module, RClass, Ruby, Value,
};
use mio::{Events, Poll, Token, Waker};
use parking_lot::{Mutex, RwLock};
use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    os::fd::RawFd,
    sync::Arc,
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
                magnus::exception::standard_error(),
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
                    poll.poll(&mut events, timeout)
                        .map_err(|e| ItsiError::ArgumentError(format!("poll error: {}", e)))?;
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

    pub fn run_blocking_in_thread<T, F>(&self, ruby: &Ruby, work: F) -> MagnusResult<Option<T>>
    where
        T: Send + Sync + std::fmt::Debug + 'static,
        F: FnOnce() -> Option<T> + Send + 'static,
    {
        let result: Arc<RwLock<Option<T>>> = Arc::new(RwLock::new(None));
        let result_clone = Arc::clone(&result);

        let current_fiber = Opaque::from(ruby.fiber_current());
        let scheduler = Opaque::from(
            ruby.module_kernel()
                .const_get::<_, RClass>("Fiber")
                .unwrap()
                .funcall::<_, _, Value>("scheduler", ())
                .unwrap(),
        );

        create_ruby_thread(move || {
            call_without_gvl(|| {
                let outcome = work();
                *result_clone.write() = outcome;
            });

            let ruby = Ruby::get().unwrap();
            scheduler
                .get_inner_with(&ruby)
                .funcall::<_, _, Value>("unblock", (None::<String>, current_fiber))
                .unwrap();
        });

        scheduler
            .get_inner_with(ruby)
            .funcall::<_, _, Value>("block", (None::<Value>, None::<u64>))?;

        let result_opt = Arc::try_unwrap(result).unwrap().write().take();
        Ok(result_opt)
    }

    pub fn address_resolve(
        ruby: &Ruby,
        rself: &Self,
        hostname: String,
    ) -> MagnusResult<Option<Vec<String>>> {
        let result: Option<Vec<String>> = rself.run_blocking_in_thread(ruby, move || {
            use std::net::ToSocketAddrs;
            let addrs_res = (hostname.as_str(), 0).to_socket_addrs();
            match addrs_res {
                Ok(addrs) => {
                    let ips: Vec<String> = addrs.map(|s| s.ip().to_string()).collect();
                    Some(ips)
                }
                Err(_) => None,
            }
        })?;
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
