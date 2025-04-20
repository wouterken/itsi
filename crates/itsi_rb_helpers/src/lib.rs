use std::{ffi::c_int, os::raw::c_void, ptr::null_mut};

use magnus::{
    ArgList, RArray, Ruby, Thread, Value,
    block::Proc,
    rb_sys::{AsRawId, FromRawValue, protect},
    value::{IntoId, LazyId, ReprValue},
};
use rb_sys::{
    VALUE, rb_funcallv, rb_thread_call_with_gvl, rb_thread_call_without_gvl, rb_thread_create,
    rb_thread_schedule, rb_thread_wakeup,
};

mod heap_value;
pub use heap_value::{HeapVal, HeapValue};
static ID_FORK: LazyId = LazyId::new("fork");
static ID_LIST: LazyId = LazyId::new("list");
static ID_EQ: LazyId = LazyId::new("==");
static ID_ALIVE: LazyId = LazyId::new("alive?");
static ID_THREAD_VARIABLE_GET: LazyId = LazyId::new("thread_variable_get");
static ID_BACKTRACE: LazyId = LazyId::new("backtrace");

pub fn schedule_thread() {
    unsafe {
        rb_thread_schedule();
    };
}
pub fn create_ruby_thread<F>(f: F) -> Option<Thread>
where
    F: FnOnce() + Send + 'static,
{
    extern "C" fn trampoline<F>(ptr: *mut c_void) -> u64
    where
        F: FnOnce(),
    {
        // Reconstruct the boxed Option<F> that holds our closure.
        let boxed_closure: Box<Option<F>> = unsafe { Box::from_raw(ptr as *mut Option<F>) };
        // Extract the closure. (The Option should be Some; panic otherwise.)
        let closure = (*boxed_closure).expect("Closure already taken");
        // Call the closure and return its result.
        closure();
        0
    }

    // Box the closure (wrapped in an Option) to create a stable pointer.
    let boxed_closure = Box::new(Some(f));
    let ptr = Box::into_raw(boxed_closure) as *mut c_void;

    // Call rb_thread_create with our trampoline and boxed closure.
    unsafe {
        let thread = rb_thread_create(Some(trampoline::<F>), ptr);
        rb_thread_wakeup(thread);
        rb_thread_schedule();
        Thread::from_value(Value::from_raw(thread))
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

pub fn fork(after_fork: Option<HeapValue<Proc>>) -> Option<i32> {
    let ruby = Ruby::get().unwrap();
    let fork_result = ruby
        .module_kernel()
        .funcall::<_, _, Option<i32>>(*ID_FORK, ())
        .ok()
        .flatten();
    if fork_result.is_none() {
        if let Some(proc) = after_fork {
            call_proc_and_log_errors(proc)
        }
    }
    fork_result
}

pub fn call_proc_and_log_errors(proc: HeapValue<Proc>) {
    if let Err(e) = proc.call::<_, Value>(()) {
        if let Some(value) = e.value() {
            print_rb_backtrace(value);
        } else {
            eprintln!("Error occurred {:?}", e);
        }
    }
}

pub fn kill_threads<T>(threads: Vec<T>)
where
    T: ReprValue,
{
    for thr in &threads {
        let alive: bool = thr
            .funcall(*ID_ALIVE, ())
            .expect("Failed to check if thread is alive");
        if !alive {
            eprintln!("Thread killed");
            break;
        }
        eprintln!("Killing thread {:?}", thr.as_value());
        thr.funcall::<_, _, Value>("terminate", ())
            .expect("Failed to kill thread");
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

    kill_threads(non_fork_safe_threads);
}

pub fn print_rb_backtrace(rb_err: Value) {
    let backtrace = rb_err
        .funcall::<_, _, Vec<String>>(*ID_BACKTRACE, ())
        .unwrap_or_default();
    let rust_backtrace = std::backtrace::Backtrace::capture().to_string();
    eprintln!("Ruby exception {:?}", rb_err);
    for line in backtrace {
        eprintln!("{}", line);
    }
    for line in rust_backtrace.lines() {
        eprintln!("{}", line);
    }
}

pub fn funcall_no_ret<T, M, A>(target: T, method: M, args: A) -> magnus::error::Result<()>
where
    T: ReprValue,
    M: IntoId,
    A: ArgList,
{
    protect(|| {
        let handle = Ruby::get().unwrap();
        let method = method.into_id_with(&handle);
        let args = args.into_arg_list_with(&handle);
        let slice = args.as_ref();
        unsafe {
            rb_funcallv(
                target.as_rb_value(),
                method.as_raw(),
                slice.len() as c_int,
                slice.as_ptr() as *const VALUE,
            );
        }
        0
    })?;
    Ok(())
}
