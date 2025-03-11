mod io_helpers;
mod io_waiter;
mod resume_args;
mod timer;
use io_helpers::{build_interest, poll_readiness, set_nonblocking};
use io_waiter::IoWaiter;
use itsi_error::ItsiError;
use itsi_instrument_entry::instrument_with_entry;
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread, HeapFiber, HeapValue};
use itsi_tracing::debug;
use magnus::{
    block::Proc,
    error::Result as MagnusResult,
    scan_args,
    value::{InnerValue, Lazy, LazyId, Opaque, ReprValue},
    ArgList, Fiber, IntoValue, Object, RClass, Ruby, Thread, TryConvert, Value,
};
use mio::{guide, Events, Poll, Token, Waker};
use nix::libc;
use parking_lot::{Mutex, RwLock};
use resume_args::ResumeArgs;
use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    os::fd::RawFd,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};
use timer::{Timer, TimerKind};
use tracing::{error, info, warn};
static ID_FILENO: LazyId = LazyId::new("fileno");
static ID_NEW: LazyId = LazyId::new("new");
static ID_SCHEDULER: LazyId = LazyId::new("scheduler");
static ID_UNBLOCK: LazyId = LazyId::new("unblock");
static ID_BACKTRACE: LazyId = LazyId::new("backtrace");
static BLOCKED_TOKEN: AtomicUsize = AtomicUsize::new(1);
static FIBER_CLASS: Lazy<RClass> =
    Lazy::new(|ruby| ruby.define_class("Fiber", ruby.class_object()).unwrap());

fn next_block_token() -> usize {
    BLOCKED_TOKEN.fetch_add(1, Ordering::SeqCst)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Readiness(i16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FdReadinessPair(RawFd, Readiness);

#[magnus::wrap(class = "Itsi::Scheduler", free_immediately, size)]
pub(crate) struct ItsiScheduler {
    current_thread: HeapValue<Thread>,
    shutdown: AtomicBool,
    waker: Arc<Waker>,
    poll: Mutex<Poll>,
    events: Mutex<Events>,
    unblock_mux: Mutex<()>,
    io_waiters: Mutex<HashMap<FdReadinessPair, IoWaiter>>,
    token_map: Mutex<HashMap<Token, FdReadinessPair>>,
    spawned_fibers: Mutex<HashMap<i64, HeapFiber>>,
    dependent: Mutex<HashMap<i64, HeapFiber>>,
    blocked: Mutex<HashMap<i64, usize>>,
    timers: Mutex<BinaryHeap<Timer>>,
    unblocked: Mutex<VecDeque<HeapFiber>>,
    yielded: Mutex<VecDeque<HeapFiber>>,
    resume_counts: Mutex<HashMap<i64, usize>>,
}

impl std::fmt::Debug for ItsiScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItsiScheduler").finish()
    }
}

const WAKE_TOKEN: Token = Token(0);
type ResumeQueue = Option<Vec<(HeapFiber, ResumeArgs)>>;

#[cfg(debug_assertions)]
fn current_fiber_name() -> String {
    format!("{:?}", Ruby::get().unwrap().fiber_current())
}

impl ItsiScheduler {
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber_name()))]
    pub fn new(ruby: &Ruby) -> MagnusResult<Self> {
        let poll = Poll::new().unwrap();
        let waker = Waker::new(poll.registry(), WAKE_TOKEN).unwrap();

        Ok(ItsiScheduler {
            current_thread: ruby.thread_current().into(),
            shutdown: AtomicBool::new(false),
            waker: Arc::new(waker),
            io_waiters: Mutex::new(HashMap::new()),
            token_map: Mutex::new(HashMap::new()),
            unblock_mux: Mutex::new(()),
            spawned_fibers: Mutex::new(HashMap::new()),
            timers: Mutex::new(BinaryHeap::new()),
            poll: Mutex::new(poll),
            events: Mutex::new(Events::with_capacity(1024)),
            dependent: Mutex::new(HashMap::new()),
            blocked: Mutex::new(HashMap::new()),
            unblocked: Mutex::new(VecDeque::new()),
            yielded: Mutex::new(VecDeque::new()),
            resume_counts: Mutex::new(HashMap::new()),
        })
    }

    /// Ruby hook to block the currently running fiber.
    /// Takes a block argument, and an optional timeout float or nil.
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber_name()))]
    pub fn block(ruby: &Ruby, rself: &Self, args: &[Value]) -> MagnusResult<()> {
        let args = scan_args::scan_args::<(Value,), (Option<Value>,), (), (), (), ()>(args)?;
        let (blocker,) = args.required;
        let (timeout,) = args.optional;
        let timeout = timeout.and_then(|v| f64::try_convert(v).ok());
        rself.block_fiber(
            ruby,
            ruby.fiber_current().into(),
            timeout.map(Duration::from_secs_f64),
            Some(blocker),
            TimerKind::Block(next_block_token()),
        )?;
        Ok(())
    }

    /// Ruby hook to put the current fiber to sleep.
    /// If duration is negative, it's a noop
    /// It it's positive we block with a timeout
    /// If it's missing, we simply yield to the event loop (putting this fiber to sleep indefinitely)
    #[instrument_with_entry(parent = None,skip(ruby),fields(fiber=current_fiber_name()))]
    pub fn kernel_sleep(ruby: &Ruby, rself: &Self, duration: Option<f64>) -> MagnusResult<()> {
        match duration {
            Some(duration) => {
                if duration < 0.0 {
                    Ok(())
                } else {
                    rself.block_fiber(
                        ruby,
                        ruby.fiber_current().into(),
                        Some(Duration::from_secs_f64(duration)),
                        None,
                        TimerKind::Sleep,
                    )
                }
            }
            None => {
                rself.block_fiber(
                    ruby,
                    ruby.fiber_current().into(),
                    None,
                    None,
                    TimerKind::Sleep,
                )?;
                Ok(())
            }
        }
    }

    /// Yields from the current Fiber (back to its parent or, if no parent, the event loop, returning the resumption value as a `Value`.
    pub fn yield_value<T>(&self, arglist: T) -> MagnusResult<Value>
    where
        T: ArgList,
    {
        self.yield_from(arglist)
    }

    /// Yields from the current Fiber (back to its parent or, if no parent, the event loop, returning the resumption value as a `V`.
    #[instrument_with_entry(parent = None,skip(arglist),fields(fiber=current_fiber_name()))]
    pub fn yield_from<T, V>(&self, arglist: T) -> MagnusResult<V>
    where
        T: ArgList,
        V: ReprValue + TryConvert,
    {
        // self.event_loop.transfer::<T, V>(arglist)
        Ruby::get().unwrap().fiber_yield(arglist)
    }

    /// Resume a yielded Fiber, if it's still alive.
    #[instrument_with_entry(parent = None,skip(self, args))]
    pub fn resume(
        &self,
        fiber: &HeapFiber,
        args: impl ArgList + std::fmt::Debug,
    ) -> MagnusResult<()> {
        let mut counts_lock = self.resume_counts.lock();
        let entry = counts_lock.entry(fiber.id()).or_insert(0);
        *entry += 1;
        if *entry > 1 && *entry < 10 {
            error!(
                "Warning. {:?} Fiber {:?} has been resumed {} times",
                fiber.clone(),
                fiber.clone().inner(),
                entry
            );
        }
        drop(counts_lock);
        debug!("Resuming fiber {:?} with args {:?}", fiber, args);
        if !fiber.is_alive() {
            error!("Attempted to resume a dead fiber");
        } else {
            fiber.resume::<_, Value>(args)?;
        }
        if !fiber.is_alive() {
            self.spawned_fibers.lock().remove(&fiber.id());
            debug!("Fiber finished {:?}", fiber);
        } else {
            debug!("Resumed has yielded again {:?}", fiber);
        }
        Ok(())
    }

    /// Flush out all cancelled timers.
    fn flush_timers(&self, timers: &mut BinaryHeap<Timer>) {
        while timers.peek().is_some_and(|t| t.canceled()) {
            timers.pop();
        }
    }

    /// Starts the event loop in the current fiber.
    /// This is automatically called at the end of the thread
    /// where the Scheduler is created.
    #[instrument_with_entry(parent = None,fields(fiber=current_fiber_name()))]
    pub fn run(_: &Ruby, rself: &Self) -> MagnusResult<()> {
        call_without_gvl(|| -> MagnusResult<()> {
            while !rself.shutdown.load(Ordering::Relaxed) {
                let mut timers = rself.timers.lock();
                rself.flush_timers(&mut timers);

                if timers.is_empty()
                    && rself.yielded.lock().is_empty()
                    && rself.io_waiters.lock().is_empty()
                    && rself.unblocked.lock().is_empty()
                    && rself.dependent.lock().is_empty()
                {
                    debug!("Breaking out now");
                    break;
                }
                if let Some(fibers) = {
                    let timeout = {
                        if let Some(timer) = timers.peek() {
                            let now = Instant::now();
                            if timer.wake_time >= now {
                                Some(timer.wake_time - now)
                            } else {
                                Some(Duration::ZERO)
                            }
                        } else if rself.yielded.lock().is_empty()
                            && rself.unblocked.lock().is_empty()
                        {
                            None
                        } else {
                            Some(Duration::ZERO)
                        }
                    };
                    drop(timers);
                    debug!("Going to sleep for {:?}", timeout);
                    rself.tick(timeout)?
                } {
                    call_with_gvl(|_| {
                        for (fiber, args) in &fibers {
                            if let Err(e) = match args {
                                ResumeArgs::None => rself.resume(fiber, ()),
                                ResumeArgs::Readiness(args) => rself.resume(fiber, (args.0,)),
                            } {
                                if let Some(rb_err) = e.value() {
                                    let backtrace = rb_err
                                        .funcall::<_, _, Vec<String>>(*ID_BACKTRACE, ())
                                        .unwrap_or_default();
                                    error!(
                                        "Fiber {:?} raised internal exception: {:?}",
                                        fiber.clone().inner(),
                                        rb_err
                                    );
                                    for line in backtrace {
                                        error!("{}", line);
                                    }
                                } else {
                                    error!("Fiber encountered internal exception {:?}", e);
                                }
                            };
                        }
                    })
                }
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Blocks the current fiber for the given duration (or indefinitely if no duration is given)
    #[instrument_with_entry(parent = None, skip(ruby))]
    pub fn block_fiber(
        &self,
        ruby: &Ruby,
        fiber: HeapFiber,
        duration: Option<Duration>,
        blocker: Option<Value>,
        kind: TimerKind,
    ) -> MagnusResult<()> {
        use tracing::warn;

        warn!("Aquiring block mux for `block_fiber`");
        let guard = self.unblock_mux.lock();
        let block_token = if let TimerKind::Block(token) = kind {
            token
        } else {
            next_block_token()
        };

        // Start a resume timer if we're given a duration.
        let timer = duration
            .map(|d| self.create_timer(d, kind, fiber.clone()))
            .transpose()?;

        let is_blocking_thread = blocker.is_some_and(|b| b.is_kind_of(ruby.class_thread()));
        if is_blocking_thread {
            self.dependent.lock().insert(fiber.id(), fiber.clone());
        }

        self.blocked.lock().insert(fiber.id(), block_token);
        drop(guard);
        // warn!("Released block mux for `block_fiber`");
        self.yield_value(())?;
        // warn!("Acquiring block mux again for `block_fiber`");
        let guard = self.unblock_mux.lock();
        self.blocked.lock().remove(&fiber.id());

        if is_blocking_thread {
            self.dependent.lock().remove(&fiber.id());
        }
        timer.inspect(|t| t.cancel());
        drop(guard);
        // warn!("Released block mux for `block_fiber`");
        Ok(())
    }

    #[instrument_with_entry(parent = None, fields(fiber=current_fiber_name()))]
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
    #[instrument_with_entry(parent = None, fields(fiber=current_fiber_name()))]
    pub fn shutdown(_: &Ruby, rself: &Self) -> MagnusResult<()> {
        rself.shutdown.store(true, Ordering::SeqCst);
        rself.waker.wake().ok();
        Ok(())
    }

    fn drain_queue(
        &self,
        queue: &Mutex<VecDeque<HeapFiber>>,
    ) -> Option<Vec<(HeapFiber, ResumeArgs)>> {
        let mut queue = queue.lock();
        if queue.is_empty() {
            None
        } else {
            Some(
                queue
                    .drain(..)
                    .map(|fiber| (fiber, ResumeArgs::None))
                    .collect::<Vec<_>>(),
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
    pub fn tick(&self, timeout: Option<Duration>) -> MagnusResult<ResumeQueue> {
        info!("Done timers, starting events");
        let fired_events = if self.io_waiters.lock().is_empty() && timeout == Some(Duration::ZERO) {
            None
        } else {
            self.poll_events(timeout)?
        };
        // warn!("Aquiring block mux for `tick`");
        let guard = self.unblock_mux.lock();
        // warn!("Got it!");
        let due_timers = if self.timers.lock().is_empty() {
            None
        } else {
            self.poll_timers()?
        };

        info!(
            "Due timers {:?}",
            due_timers.as_ref().map(|timers| timers.len())
        );
        info!(
            "Fired events {:?}",
            fired_events.as_ref().map(|events| events.len())
        );
        info!("Unblocked {:?}", self.unblocked.lock());
        info!("Yielded {:?}", self.yielded.lock());
        let to_resume = due_timers
            .into_iter()
            .chain(fired_events)
            .chain(self.drain_queue(&self.unblocked))
            .chain(self.drain_queue(&self.yielded))
            .flatten()
            .collect::<Vec<_>>();

        drop(guard);
        // warn!("Releasing block mux for `tick`");
        Ok(Some(to_resume))
    }

    /// Poll timers, returning all that are due.
    pub fn poll_events(&self, timeout: Option<Duration>) -> MagnusResult<ResumeQueue> {
        info!("Polling events");
        let mut due_fibers: ResumeQueue = None;
        let mut io_waiters = self.io_waiters.lock();
        info!("Got IO Waiters");

        if !io_waiters.is_empty() || timeout != Some(Duration::ZERO) {
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
                let mut is_empty = false;
                let mut rdy: Option<FdReadinessPair> = None;
                if let Some(readiness) = self.token_map.lock().get(&token) {
                    rdy = Some(*readiness);
                    if let Some(waiter) = io_waiters.get_mut(readiness) {
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
                                due_fibers.get_or_insert_default().push((
                                    next_fiber,
                                    ResumeArgs::Readiness(Readiness(evt_readiness)),
                                ));
                                break;
                            }
                        }
                        is_empty = waiter.fibers.is_empty();
                    }
                }
                if is_empty {
                    let mut waiter = io_waiters
                        .remove(
                            &rdy.ok_or(ItsiError::ArgumentError("Readiness Missing".to_string()))?,
                        )
                        .ok_or(ItsiError::ArgumentError("Waiter Missing".to_string()))?;
                    self.poll
                        .lock()
                        .registry()
                        .deregister(&mut waiter)
                        .map_err(|_| {
                            ItsiError::ArgumentError("Failed to deregister".to_string())
                        })?;
                    self.token_map.lock().remove(&token);
                }
            }
            info!("Returning");
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
            if timer.wake_time <= now && !timer.canceled() {
                match timer.kind {
                    TimerKind::Sleep => {
                        debug!("Sleep finished, queueing fiber {:?}", timer.fiber);
                        due_fibers
                            .get_or_insert_default()
                            .push((timer.fiber.clone(), ResumeArgs::None));
                    }
                    TimerKind::Block(token) => {
                        if let Some(current_block_token) =
                            self.blocked.lock().remove(&timer.fiber.id())
                        {
                            if token == current_block_token {
                                due_fibers
                                    .get_or_insert_default()
                                    .push((timer.fiber.clone(), ResumeArgs::None));
                            } else {
                                debug!("Refusing to resume out of date token");
                            }
                        }
                    }
                    TimerKind::IoWait(token) => {
                        if self.io_waiters.lock().remove(&token).is_some() {
                            debug!("IO wait finished, queueing fiber {:?}", timer.fiber);
                            due_fibers
                                .get_or_insert_default()
                                .push((timer.fiber.clone(), ResumeArgs::None));
                        } else {
                            debug!("IO wait finished but no waiter found {:?}", timer.fiber);
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
    #[instrument_with_entry(parent = None, skip(ruby),fields(fiber=current_fiber_name()))]
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
        let readiness_pair = FdReadinessPair(fd, Readiness(events));
        {
            let mut binding = rself.io_waiters.lock();
            let io_waiter = binding
                .entry(readiness_pair)
                .or_insert_with(|| IoWaiter::new(fd));
            debug!(
                "Registering interest for fd: {:?} with token {:?}",
                fd, io_waiter.token
            );
            io_waiter.fibers.push_back(current_fiber);
            rself
                .token_map
                .lock()
                .entry(io_waiter.token)
                .or_insert(readiness_pair);

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
                    TimerKind::IoWait(readiness_pair),
                );
                rself.timers.lock().push(timer_entry);
            }
        }
        rself.waker.wake().ok();
        rself.yield_value(())
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
                .funcall::<_, _, Value>(*ID_SCHEDULER, ())?,
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

        self.block_fiber(
            ruby,
            ruby.fiber_current().into(),
            None,
            Some(self.current_thread.as_value()),
            TimerKind::Block(next_block_token()),
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

    #[instrument_with_entry(parent = None, fields(fiber=current_fiber_name()))]
    pub fn unblock(_: &Ruby, rself: &Self, _: Value, fiber: Fiber) -> MagnusResult<()> {
        // warn!("Aquiring block mux for `unblock`");
        let guard = rself.unblock_mux.lock();
        // Only unblock the fiber if its still running, and we haven't already unblocked it.
        if fiber.is_alive() && !rself.unblocked.lock().contains(&fiber.into()) {
            rself.unblocked.lock().push_back(fiber.into());
            rself.waker.wake().expect("Failed to wake scheduler");
        } else {
            debug!(
                "Failed to unblock on Fiber raw: {:?}",
                HeapFiber::from(fiber)
            );
        }
        drop(guard);
        // warn!("Releasing block mux for `unblock`");
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
            rself.yielded.lock().push_back(ruby.fiber_current().into());
            debug!("Yielding to scheduler");
            rself.yield_value(())?;
        } else {
            debug!("Yielded to scheduler, no work");
        }
        Ok(())
    }

    #[instrument_with_entry(parent = None, skip(ruby),fields(fiber=current_fiber_name()))]
    pub fn fiber(ruby: &Ruby, rself: &Self) -> MagnusResult<Fiber> {
        let block: Proc = ruby.block_proc()?;
        let fiber: HeapFiber = ruby
            .get_inner(&FIBER_CLASS)
            .funcall_with_block::<_, _, Fiber>(*ID_NEW, (), block)
            .expect("Failed to create fiber")
            .into();
        rself
            .spawned_fibers
            .lock()
            .insert(fiber.id(), fiber.clone());
        rself.resume(&fiber, ())?;
        Ok(fiber.inner())
    }
}
