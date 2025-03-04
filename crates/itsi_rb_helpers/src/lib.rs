use std::marker::PhantomData;
use std::sync::Mutex;
use std::{os::raw::c_void, ptr::null_mut, sync::Arc};

use magnus::rb_sys::AsRawValue;
use magnus::{
    RArray, Ruby, Thread, Value,
    rb_sys::FromRawValue,
    value::{LazyId, ReprValue},
};
use rb_sys::bindings::uncategorized::{rb_gc_register_address, rb_gc_unregister_address};
use rb_sys::{
    rb_thread_call_with_gvl, rb_thread_call_without_gvl, rb_thread_create, rb_thread_schedule,
    rb_thread_wakeup,
};

static ID_FORK: LazyId = LazyId::new("fork");
static ID_LIST: LazyId = LazyId::new("list");
static ID_EQ: LazyId = LazyId::new("==");
static ID_EXIT: LazyId = LazyId::new("exit");
static ID_JOIN: LazyId = LazyId::new("join");
static ID_ALIVE: LazyId = LazyId::new("alive?");
static ID_THREAD_VARIABLE_GET: LazyId = LazyId::new("thread_variable_get");

use rb_sys::VALUE;

pub struct RetainedValueInner<T> {
    inner: Box<VALUE>,
    _marker: PhantomData<T>, // Phantom type parameter
}

#[derive(Clone)]
pub struct RetainedValue<T>
where
    T: ReprValue,
{
    inner: Arc<Mutex<Option<Arc<RetainedValueInner<T>>>>>,
}

unsafe impl<T> Send for RetainedValueInner<T> where T: ReprValue {}
unsafe impl<T> Sync for RetainedValueInner<T> where T: ReprValue {}

impl<T> RetainedValueInner<T>
where
    T: ReprValue,
{
    pub fn new(value: T) -> Self {
        let mut value = Box::new(value.as_raw());
        let ptr: *mut VALUE = &mut *value as *mut VALUE;
        unsafe { rb_gc_register_address(ptr) };
        RetainedValueInner {
            inner: value,
            _marker: PhantomData,
        }
    }
}

impl<T> RetainedValue<T>
where
    T: ReprValue,
{
    pub fn new(value: T) -> Self {
        let inner = Arc::new(Mutex::new(Some(Arc::new(RetainedValueInner::new(value)))));
        RetainedValue { inner }
    }

    pub fn empty() -> Self {
        RetainedValue {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn as_value(&self) -> Option<T> {
        {
            let guard = self.inner.lock().unwrap();
            guard
                .as_ref()
                .map(|inner| unsafe { T::from_value_unchecked(Value::from_raw(*inner.inner)) })
        }
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().take();
    }
}

impl<T> Drop for RetainedValueInner<T> {
    fn drop(&mut self) {
        unsafe {
            rb_gc_unregister_address(&mut *self.inner as *mut VALUE);
        }
    }
}

pub fn schedule_thread() {
    unsafe {
        rb_thread_schedule();
    };
}
pub fn create_ruby_thread<F>(f: F) -> Thread
where
    F: FnOnce() -> u64 + Send + 'static,
{
    extern "C" fn trampoline<F>(ptr: *mut c_void) -> u64
    where
        F: FnOnce() -> u64,
    {
        // Reconstruct the boxed Option<F> that holds our closure.
        let boxed_closure: Box<Option<F>> = unsafe { Box::from_raw(ptr as *mut Option<F>) };
        // Extract the closure. (The Option should be Some; panic otherwise.)
        let closure = (*boxed_closure).expect("Closure already taken");
        // Call the closure and return its result.
        closure()
    }

    // Box the closure (wrapped in an Option) to create a stable pointer.
    let boxed_closure = Box::new(Some(f));
    let ptr = Box::into_raw(boxed_closure) as *mut c_void;

    // Call rb_thread_create with our trampoline and boxed closure.
    unsafe {
        let thread = rb_thread_create(Some(trampoline::<F>), ptr);
        rb_thread_wakeup(thread);
        rb_thread_schedule();
        Thread::from_value(Value::from_raw(thread)).unwrap()
    }
}

pub fn call_without_gvl<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    // This is the function that Ruby calls “in the background” with the GVL released.
    extern "C" fn trampoline<F, R>(arg: *mut c_void) -> *mut c_void
    where
        F: FnOnce() -> R,
    {
        // 1) Reconstruct the Box that holds our closure
        let closure_ptr = arg as *mut Option<F>;
        let closure = unsafe { (*closure_ptr).take().expect("Closure already taken") };

        // 2) Call the user’s closure
        let result = closure();

        // 3) Box up the result so we can return a pointer to it
        let boxed_result = Box::new(result);
        Box::into_raw(boxed_result) as *mut c_void
    }

    // Box up the closure so we have a stable pointer
    let mut closure_opt = Some(f);
    let closure_ptr = &mut closure_opt as *mut Option<F> as *mut c_void;

    // 4) Actually call `rb_thread_call_without_gvl`
    let raw_result_ptr = unsafe {
        rb_thread_call_without_gvl(Some(trampoline::<F, R>), closure_ptr, None, null_mut())
    };

    // 5) Convert the returned pointer back into R
    let result_box = unsafe { Box::from_raw(raw_result_ptr as *mut R) };
    *result_box
}

pub fn call_with_gvl<F, R>(f: F) -> R
where
    F: FnOnce(Ruby) -> R,
{
    extern "C" fn trampoline<F, R>(arg: *mut c_void) -> *mut c_void
    where
        F: FnOnce(Ruby) -> R,
    {
        // 1) Reconstruct the Box that holds our closure
        let closure_ptr = arg as *mut Option<F>;
        let closure = unsafe { (*closure_ptr).take().expect("Closure already taken") };

        // 2) Call the user’s closure
        let result = closure(Ruby::get().unwrap());

        // 3) Box up the result so we can return a pointer to it
        let boxed_result = Box::new(result);
        Box::into_raw(boxed_result) as *mut c_void
    }

    // Box up the closure so we have a stable pointer
    let mut closure_opt = Some(f);
    let closure_ptr = &mut closure_opt as *mut Option<F> as *mut c_void;

    // 4) Actually call `rb_thread_call_without_gvl`
    let raw_result_ptr = unsafe { rb_thread_call_with_gvl(Some(trampoline::<F, R>), closure_ptr) };

    // 5) Convert the returned pointer back into R
    let result_box = unsafe { Box::from_raw(raw_result_ptr as *mut R) };
    *result_box
}

pub fn fork(after_fork: Arc<Option<impl Fn()>>) -> Option<i32> {
    let ruby = Ruby::get().unwrap();
    let fork_result = ruby
        .module_kernel()
        .funcall::<_, _, Option<i32>>(*ID_FORK, ())
        .unwrap();
    if fork_result.is_none() {
        if let Some(f) = &*after_fork {
            f()
        }
    }
    fork_result
}

pub fn soft_kill_threads(threads: Vec<Thread>) {
    for thr in &threads {
        let _: Option<Value> = thr.funcall(*ID_EXIT, ()).expect("Failed to exit thread");
    }

    for thr in &threads {
        let _: Option<Value> = thr
            .funcall(*ID_JOIN, (0.5_f64,))
            .expect("Failed to join thread");
    }

    for thr in &threads {
        let alive: bool = thr
            .funcall(*ID_ALIVE, ())
            .expect("Failed to check if thread is alive");
        if alive {
            thr.kill().expect("Failed to kill thread");
        }
    }
}

pub fn terminate_non_fork_safe_threads() {
    let ruby = Ruby::get().unwrap();
    let thread_class = ruby.class_thread();
    let current: Thread = ruby.thread_current();
    let threads: RArray = thread_class
        .funcall(*ID_LIST, ())
        .expect("Failed to list Ruby threads");

    let non_fork_safe_threads = threads
        .into_iter()
        .filter_map(|v| {
            let v_thread = Thread::from_value(v).unwrap();
            let non_fork_safe = !v_thread
                .funcall::<_, _, bool>(*ID_EQ, (current,))
                .unwrap_or(false)
                && !v_thread
                    .funcall::<_, _, bool>(*ID_THREAD_VARIABLE_GET, (ruby.sym_new("fork_safe"),))
                    .unwrap_or(false);
            if non_fork_safe { Some(v_thread) } else { None }
        })
        .collect::<Vec<_>>();

    soft_kill_threads(non_fork_safe_threads);
}
