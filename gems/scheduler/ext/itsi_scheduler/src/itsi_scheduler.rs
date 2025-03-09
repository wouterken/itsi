mod immediate;
mod io_helpers;
mod io_waiter;
mod resume_args;
mod timer;
use immediate::Immediate;
use io_helpers::{build_interest, poll_readiness, set_nonblocking};
use io_waiter::IoWaiter;
use itsi_error::ItsiError;
use itsi_instrument_entry::instrument_with_entry;
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread, HeapFiber, HeapValue};
use itsi_tracing::info;
use magnus::{
    block::Proc,
    error::Result as MagnusResult,
    rb_sys::AsRawValue,
    scan_args,
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    ArgList, Fiber, IntoValue, RClass, Ruby, Thread, TryConvert, Value,
};
use mio::{Events, Poll, Token, Waker};
use nix::libc;
use parking_lot::{Mutex, RwLock};
use rb_sys::VALUE;
use resume_args::ResumeArgs;
use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    os::fd::RawFd,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use timer::{Timer, TimerKind};
use tracing::error;
static ID_FILENO: LazyId = LazyId::new("fileno");
static FIBER_CLASS: Lazy<RClass> =
    Lazy::new(|ruby| ruby.define_class("Fiber", ruby.class_object()).unwrap());
static ID_NEW: LazyId = LazyId::new("new");
static ID_SCHEDULER: LazyId = LazyId::new("scheduler");
static ID_UNBLOCK: LazyId = LazyId::new("unblock");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Readiness(i16);

#[magnus::wrap(class = "Itsi::Scheduler", free_immediately, size)]
pub(crate) struct ItsiScheduler {
    current_thread: HeapValue<Thread>,
    shutdown: AtomicBool,
    waker: Arc<Waker>,
    io_waiters: Mutex<HashMap<RawFd, IoWaiter>>,
    token_map: Mutex<HashMap<Token, RawFd>>,
    poll: Mutex<Poll>,
    events: Mutex<Events>,
    suspended: Mutex<HashMap<VALUE, HeapFiber>>,
    timers: Mutex<BinaryHeap<Timer>>,
    spawned_fibers: Mutex<HashMap<VALUE, HeapFiber>>,
    unblocked: Mutex<VecDeque<Immediate>>,
    yielded: Mutex<VecDeque<Immediate>>,
}

impl std::fmt::Debug for ItsiScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItsiScheduler").finish()
    }
}

const WAKE_TOKEN: Token = Token(0);
type ResumeQueue = Option<Vec<(HeapFiber, ResumeArgs)>>;

fn current_fiber() -> String {
    format!("{:?}", Ruby::get().unwrap().fiber_current())
}

impl ItsiScheduler {
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber()))]
    pub fn new(ruby: &Ruby) -> MagnusResult<Self> {
        let poll = Poll::new().unwrap();
        let events = Events::with_capacity(1024);
        let waker = Waker::new(poll.registry(), WAKE_TOKEN).unwrap();
        info!("Creating new Itsi::Scheduler");
        Ok(ItsiScheduler {
            current_thread: ruby.thread_current().into(),
            shutdown: AtomicBool::new(false),
            waker: Arc::new(waker),
            io_waiters: Mutex::new(HashMap::new()),
            token_map: Mutex::new(HashMap::new()),
            timers: Mutex::new(BinaryHeap::new()),
            poll: Mutex::new(poll),
            events: Mutex::new(events),
            suspended: Mutex::new(HashMap::new()),
            spawned_fibers: Mutex::new(HashMap::new()),
            unblocked: Mutex::new(VecDeque::new()),
            yielded: Mutex::new(VecDeque::new()),
        })
    }

    /// Ruby hook to block the currently running fiber.
    /// Takes a block argument, and an optional timeout float or nil.
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber()))]
    pub fn block(ruby: &Ruby, rself: &Self, args: &[Value]) -> MagnusResult<()> {
        let args = scan_args::scan_args::<(Value,), (Option<Value>,), (), (), (), ()>(args)?;
        let (blocker,) = args.required;
        let (timeout,) = args.optional;
        let timeout = timeout.and_then(|v| f64::try_convert(v).ok());
        rself.block_current_fiber(
            ruby,
            timeout.map(Duration::from_secs_f64),
            Some(blocker),
            TimerKind::Block,
        )?;
        Ok(())
    }

    /// Ruby hook to put the current fiber to sleep.
    /// If duration is negative, it's a noop
    /// It it's positive we block with a timeout
    /// If it's missing, we simply yield to the event loop (putting this fiber to sleep indefinitely)
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber()))]
    pub fn kernel_sleep(ruby: &Ruby, rself: &Self, duration: Option<f64>) -> MagnusResult<()> {
        match duration {
            Some(duration) => {
                if duration < 0.0 {
                    Ok(())
                } else {
                    rself.block_current_fiber(
                        ruby,
                        Some(Duration::from_secs_f64(duration)),
                        None,
                        TimerKind::Sleep,
                    )
                }
            }
            None => {
                rself.yield_value(())?;
                Ok(())
            }
        }
    }

    /// Yields to the event loop, returning the resumption value as a `Value`
    pub fn yield_value<T>(&self, arglist: T) -> MagnusResult<Value>
    where
        T: ArgList,
    {
        self.yield_from(arglist)
    }

    /// Yields to the event loop, returning the resumption value as a `V`
    #[instrument_with_entry(parent = None,skip(arglist),fields(fiber=current_fiber()))]
    pub fn yield_from<T, V>(&self, arglist: T) -> MagnusResult<V>
    where
        T: ArgList,
        V: ReprValue + TryConvert,
    {
        // self.event_loop.transfer::<T, V>(arglist)
        Ruby::get().unwrap().fiber_yield(arglist)
    }

    #[instrument_with_entry(parent = None,skip(self, args))]
    pub fn resume(
        &self,
        fiber: &HeapFiber,
        args: impl ArgList + std::fmt::Debug,
    ) -> MagnusResult<()> {
        info!("Resuming fiber {:?} with args {:?}", fiber, args);
        if !fiber.is_alive() {
            error!("Attempted to resume a dead fiber");
        } else {
            fiber.resume::<_, Value>(args)?;
        }
        info!("Resume complete");
        if !fiber.is_alive() {
            self.spawned_fibers.lock().remove(&fiber.as_raw());
        }
        Ok(())
    }

    fn flush_timers(&self, timers: &mut BinaryHeap<Timer>) {
        while timers.peek().is_some_and(|t| t.canceled()) {
            timers.pop();
        }
    }

    /// Starts the event loop in the current fiber.
    /// This is automatically called at the end of the thread
    /// where the Scheduler is created.
    #[instrument_with_entry(parent = None,fields(fiber=current_fiber()))]
    pub fn run(_: &Ruby, rself: &Self) -> MagnusResult<()> {
        call_without_gvl(|| -> MagnusResult<()> {
            while !rself.shutdown.load(Ordering::Relaxed) {
                if let Some(fibers) = {
                    let timeout = {
                        let mut timers = rself.timers.lock();
                        rself.flush_timers(&mut timers);
                        if let Some(timer) = timers.peek() {
                            let now = Instant::now();
                            if timer.wake_time >= now {
                                Some(timer.wake_time - now)
                            } else {
                                Some(Duration::ZERO)
                            }
                        } else if rself.yielded.lock().is_empty() {
                            None
                        } else {
                            Some(Duration::ZERO)
                        }
                    };
                    info!("Going to sleep for {:?}", timeout);
                    rself.tick(timeout)?
                } {
                    for (fiber, args) in &fibers {
                        match args {
                            ResumeArgs::None => call_with_gvl(|_| rself.resume(fiber, ())).ok(),
                            ResumeArgs::Readiness(args) => {
                                info!("Calling with readiness {:?}", args);
                                call_with_gvl(|_| rself.resume(fiber, (args.0,))).ok()
                            }
                        };
                    }
                }
                if rself.timers.lock().is_empty()
                    && rself.yielded.lock().is_empty()
                    && rself.io_waiters.lock().is_empty()
                    && rself.suspended.lock().is_empty()
                {
                    info!("Breaking out now");
                    break;
                }
            }
            Ok(())
        })?;
        info!("Event loop finished");
        Ok(())
    }

    /// Blocks the current fiber for the given duration (or indefinitely if None)
    #[instrument_with_entry(parent = None, skip(ruby),fields(fiber=current_fiber()))]
    pub fn block_current_fiber(
        &self,
        ruby: &Ruby,
        duration: Option<Duration>,
        blocker: Option<Value>,

        kind: TimerKind,
    ) -> MagnusResult<()> {
        let current_fiber: HeapFiber = ruby.fiber_current().into();
        // Start a resume timer if we're given a duration.
        let timer = duration
            .map(|d| self.create_timer(d, kind, current_fiber.clone()))
            .transpose()?;

        // Suspend the current fiber, by adding it to the suspended set
        // and transferring control to the event loop.
        let should_block = blocker.is_some_and(|b| b.is_kind_of(ruby.class_thread()));
        info!("Blocking!. Should block: {}", should_block);
        if should_block {
            self.suspended
                .lock()
                .insert(current_fiber.clone().as_raw(), current_fiber.clone());
        }

        self.yield_value(())?;

        // Someone resumed us! Either the timer or a manual unblock.
        // Remove from suspended set, and cancel timer if it exists.
        if should_block {
            self.suspended.lock().remove(&current_fiber.as_raw());
        }
        if let Some(timer) = timer {
            timer.cancel();
        }

        Ok(())
    }

    #[instrument_with_entry(parent = None, fields(fiber=current_fiber()))]
    pub fn create_timer(
        &self,
        duration: Duration,
        kind: TimerKind,
        fiber: HeapFiber,
    ) -> MagnusResult<Timer> {
        let timer = Timer::new(duration, fiber, kind);
        self.timers.lock().push(timer.clone());
        Ok(timer)
    }

    /// Set our shutdown flag, and wake the event loop.
    #[instrument_with_entry(parent = None, fields(fiber=current_fiber()))]
    pub fn shutdown(_: &Ruby, rself: &Self) -> MagnusResult<()> {
        rself.shutdown.store(true, Ordering::SeqCst);
        let _ = rself.waker.wake();
        Ok(())
    }

    fn drain_queue(
        &self,
        queue: &Mutex<VecDeque<Immediate>>,
    ) -> Option<Vec<(HeapFiber, ResumeArgs)>> {
        let mut queue = queue.lock();
        if queue.is_empty() {
            None
        } else {
            Some(
                queue
                    .drain(..)
                    .filter_map(|immediate| {
                        if immediate.canceled() || !immediate.fiber.is_alive() {
                            None
                        } else {
                            Some((immediate.fiber, ResumeArgs::None))
                        }
                    })
                    .collect(),
            )
        }
    }

    /// A single tick of the event loop.
    /// * Wakes any due IO waiters (Waiting for up to timeout for events, or if interrupted)
    /// * Fires any due timers
    /// * Wakes any due Unblocked Fibers, Either unblocked through:
    /// * * An explicit unblock from another thread; or
    /// * * A process wait; or
    /// * * An address resolution
    // #[instrument_with_entry(fields(fiber=current_fiber()))]
    pub fn tick(&self, timeout: Option<Duration>) -> MagnusResult<ResumeQueue> {
        let due_timers = if self.timers.lock().is_empty() {
            None
        } else {
            self.poll_timers()?
        };
        let fired_events = if self.io_waiters.lock().is_empty() && timeout == Some(Duration::ZERO) {
            None
        } else {
            self.poll_events(timeout)?
        };
        let to_resume = due_timers
            .into_iter()
            .chain(fired_events)
            .chain(self.drain_queue(&self.unblocked))
            .chain(self.drain_queue(&self.yielded))
            .flatten()
            .collect::<Vec<_>>();

        Ok(Some(to_resume))
    }

    /// Poll timers, returning all that are due.
    pub fn poll_events(&self, timeout: Option<Duration>) -> MagnusResult<ResumeQueue> {
        let mut due_fibers: ResumeQueue = None;
        let mut io_waiters = self.io_waiters.lock();
        if !io_waiters.is_empty() || timeout != Some(Duration::ZERO) {
            let mut events = self.events.lock();
            {
                let mut poll = self.poll.lock();
                poll.poll(&mut events, timeout)
                    .map_err(|e| ItsiError::ArgumentError(format!("poll error: {}", e)))
                    .unwrap();
            };

            for event in events.iter() {
                let token = event.token();
                if token == WAKE_TOKEN {
                    continue;
                }
                let mut is_empty = false;
                let mut rdy: i32 = 0;
                if let Some(readiness) = self.token_map.lock().get(&token) {
                    rdy = *readiness;
                    if let Some(waiter) = io_waiters.get_mut(readiness) {
                        info!("Event is ready for {:?} and token {:?}", readiness, token);
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

                        while !waiter.fibers.is_empty() {
                            if let Some(next_fiber) = waiter.fibers.pop_front() {
                                if !next_fiber.is_alive() {
                                    continue;
                                }
                                info!(
                                    "Will call {:?} for token {:?} and readiness {:?}",
                                    next_fiber, token, evt_readiness
                                );
                                due_fibers.get_or_insert_default().push((
                                    next_fiber,
                                    ResumeArgs::Readiness(Readiness(evt_readiness)),
                                ));
                            }
                        }
                        is_empty = waiter.fibers.is_empty();
                    }
                }
                if is_empty {
                    let mut waiter = io_waiters.remove(&rdy).unwrap();
                    self.poll.lock().registry().deregister(&mut waiter).unwrap();
                    self.token_map.lock().remove(&token);
                }
            }
            return Ok(due_fibers);
        }
        Ok(None)
    }
    /// Poll timers, returning all that are due.
    pub fn poll_timers(&self) -> MagnusResult<ResumeQueue> {
        let mut timers = self.timers.lock();
        let now = Instant::now();
        let mut due_fibers: ResumeQueue = None;
        while let Some(timer) = timers.peek() {
            if timer.wake_time <= now && !timer.canceled() && timer.fiber.is_alive() {
                match timer.kind {
                    TimerKind::Sleep | TimerKind::Block => {
                        info!("Sleep finished, queueing fiber {:?}", timer.fiber);
                        due_fibers
                            .get_or_insert_default()
                            .push((timer.fiber.clone(), ResumeArgs::None));
                    }
                    TimerKind::IoWait(token) => {
                        if self.io_waiters.lock().remove(&token).is_some() {
                            info!("IO wait finished, queueing fiber {:?}", timer.fiber);
                            due_fibers
                                .get_or_insert_default()
                                .push((timer.fiber.clone(), ResumeArgs::None));
                        } else {
                            info!("IO wait finished but no waiter found {:?}", timer.fiber);
                        }
                    }
                }
            } else if timer.wake_time > now {
                break;
            }
            timers.pop();
        }
        Ok(due_fibers)
    }

    /// Allows a fiber to register interest in the given set of events for an IO object.
    /// If there's already a fiber registered for the same IO object and events, this Fiber will be queued.
    /// (To try and ensure fair scheduling we use a FIFO for listeners.)
    #[instrument_with_entry(parent = None, skip(ruby),fields(fiber=current_fiber()))]
    pub fn io_wait(
        ruby: &Ruby,
        rself: &Self,
        io_obj: Value,
        events: i16,
        timeout: Option<f64>,
    ) -> MagnusResult<Value> {
        let fd: RawFd = io_obj
            .funcall::<_, _, RawFd>(*ID_FILENO, ())
            .expect("Couldn't get fileno");

        // Return immediately if the fd is already ready for given events.
        let readiness = poll_readiness(fd, events).unwrap_or(Readiness(0));
        if readiness == Readiness(events) {
            return Ok(readiness.0.into_value());
        }

        // Otherwise make sure FD is non-blocking, and we register our interest in the given events.
        set_nonblocking(fd)?;
        let interest = build_interest(events)?;
        let current_fiber: HeapFiber = ruby.fiber_current().into();
        {
            let mut binding = rself.io_waiters.lock();

            let io_waiter = binding.entry(fd).or_insert_with(|| IoWaiter::new(fd));
            info!(
                "Registering interest for fd: {:?} with token {:?}",
                fd, io_waiter.token
            );
            io_waiter.fibers.push_back(current_fiber);
            rself.token_map.lock().entry(io_waiter.token).or_insert(fd);

            // No need to re-register, if we're already registered.
            if io_waiter.fibers.len() == 1 {
                rself
                    .poll
                    .lock()
                    .registry()
                    .register(io_waiter, io_waiter.token, interest)
                    .map_err(|e| ItsiError::ArgumentError(format!("register error: {}", e)))?;
            }
            if timeout.is_some_and(|t| t > 0.0) {
                let timer_entry = Timer::new(
                    Duration::from_secs_f64(timeout.unwrap()),
                    ruby.fiber_current().into(),
                    TimerKind::IoWait(fd),
                );
                rself.timers.lock().push(timer_entry);
            }
        }
        rself.waker.wake().ok();
        let readiness = rself.yield_value(())?;
        Ok(readiness)
    }

    pub fn process_wait(
        ruby: &Ruby,
        rself: &Self,
        pid: i32,
        flags: i32,
    ) -> MagnusResult<Option<i32>> {
        let result = rself.run_blocking_in_thread(ruby, move || {
            let mut status: i32 = 0;
            unsafe {
                libc::waitpid(pid, &mut status as *mut i32, flags);
            }
            Some(status)
        })?;
        Ok(result)
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
            ruby.get_inner(&FIBER_CLASS)
                .funcall::<_, _, Value>(*ID_SCHEDULER, ())
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
                .funcall::<_, _, Value>(*ID_UNBLOCK, (None::<String>, current_fiber))
                .unwrap();
        });

        self.block_current_fiber(
            ruby,
            None,
            Some(self.current_thread.as_value()),
            TimerKind::Block,
        )?;
        let result_opt = Arc::try_unwrap(result).unwrap().write().take();
        Ok(result_opt)
    }

    pub fn address_resolve(
        ruby: &Ruby,
        rself: &Self,
        hostname: String,
    ) -> MagnusResult<Option<Vec<String>>> {
        let result = rself.run_blocking_in_thread(ruby, move || {
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

    #[instrument_with_entry(parent = None, fields(fiber=current_fiber()))]
    pub fn unblock(_: &Ruby, rself: &Self, _: Value, fiber: Fiber) -> MagnusResult<()> {
        if fiber.is_alive() {
            rself
                .unblocked
                .lock()
                .push_back(Immediate::new(fiber.into()));
            rself.waker.wake().unwrap();
        }

        Ok(())
    }

    pub fn has_work(&self) -> bool {
        !self.yielded.lock().is_empty()
            || !self.unblocked.lock().is_empty()
            || !self.io_waiters.lock().is_empty()
            || self.timers.lock().iter().any(|t| t.due())
    }

    pub fn scheduler_yield(ruby: &Ruby, rself: &Self) -> MagnusResult<()> {
        if rself.has_work() {
            let immediate = Immediate::new(ruby.fiber_current().into());
            rself.yielded.lock().push_back(immediate.clone());
            rself.yield_value(())?;
            immediate.cancel();
        }
        Ok(())
    }

    #[instrument_with_entry(parent = None, skip(ruby),fields(fiber=current_fiber()))]
    pub fn fiber(ruby: &Ruby, rself: &Self, args: &[Value]) -> MagnusResult<Fiber> {
        let args = scan_args::scan_args::<(), (), (), (), (), Proc>(args)?;
        let block: Proc = args.block;
        let fiber: HeapFiber = ruby
            .get_inner(&FIBER_CLASS)
            .funcall_with_block::<_, _, Fiber>(*ID_NEW, (), block)
            .unwrap()
            .into();
        rself
            .spawned_fibers
            .lock()
            .insert(fiber.as_raw(), fiber.clone());
        rself.resume(&fiber, ())?;
        Ok(fiber.inner())
    }
}
