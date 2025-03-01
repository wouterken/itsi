use std::{os::raw::c_void, ptr::null_mut};

use rb_sys::{
    rb_thread_call_with_gvl, rb_thread_call_without_gvl, rb_thread_create, rb_thread_schedule,
    rb_thread_wakeup,
};

pub fn schedule_thread() {
    unsafe {
        rb_thread_schedule();
    };
}
pub fn create_ruby_thread<F>(f: F)
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
        rb_thread_wakeup(rb_thread_create(Some(trampoline::<F>), ptr));
        rb_thread_schedule();
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
    F: FnOnce() -> R,
{
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
    let raw_result_ptr = unsafe { rb_thread_call_with_gvl(Some(trampoline::<F, R>), closure_ptr) };

    // 5) Convert the returned pointer back into R
    let result_box = unsafe { Box::from_raw(raw_result_ptr as *mut R) };
    *result_box
}
