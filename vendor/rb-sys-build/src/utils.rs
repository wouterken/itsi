use crate::debug_log;

/// Check if current platform is mswin.
pub fn is_msvc() -> bool {
    if let Ok(target) = std::env::var("TARGET") {
        target.contains("msvc")
    } else {
        false
    }
}

/// Check if current platform is mswin or mingw.
pub fn is_mswin_or_mingw() -> bool {
    if let Ok(target) = std::env::var("TARGET") {
        target.contains("msvc") || target.contains("pc-windows-gnu")
    } else {
        false
    }
}

/// Splits shell words.
pub fn shellsplit<S: AsRef<str>>(s: S) -> Vec<String> {
    let s = s.as_ref();
    match shell_words::split(s) {
        Ok(v) => v,
        Err(e) => {
            debug_log!("WARN: shellsplit failed: {}", e);
            s.split_whitespace().map(Into::into).collect()
        }
    }
}

#[macro_export]
macro_rules! memoize {
    ($type:ty: $val:expr) => {{
        static INIT: std::sync::Once = std::sync::Once::new();
        static mut VALUE: Option<$type> = None;
        #[allow(static_mut_refs)]
        unsafe {
            INIT.call_once(|| {
                VALUE = Some($val);
            });
            VALUE.as_ref().unwrap()
        }
    }};
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        eprintln!($($arg)*);
    };
}
