use magnus::value::{BoxValue, ReprValue};
use magnus::{Fiber, IntoValue, Object};
use magnus::{Ruby, Value};
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::heap_value::HeapValue;

#[derive(Clone, PartialEq)]
pub struct HeapFiber(HeapValue<Fiber>, i64);

impl From<magnus::Fiber> for HeapFiber {
    fn from(value: magnus::Fiber) -> Self {
        let id = value.hash().unwrap().to_i64().unwrap();
        HeapFiber(HeapValue(BoxValue::new(value)), id)
    }
}

impl Deref for HeapFiber {
    type Target = Fiber;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl HeapFiber {
    pub fn inner(self) -> Fiber {
        *self.0
    }

    pub fn id(&self) -> i64 {
        self.1
    }
}

impl IntoValue for HeapFiber {
    fn into_value_with(self, _: &Ruby) -> Value {
        self.0.into_value()
    }
}

cfg_if::cfg_if! {
  if #[cfg(debug_assertions)] {
    impl Debug for HeapFiber {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let raw_id = self.1;
            let short_name = if raw_id == 0 {
                "main".to_string()
            } else {
                format!("F{}", raw_id)
            };
            let full_str = format!("{:?}", self.0);
            let (path_line, state) = parse_fiber_debug(&full_str);
            if path_line.is_empty() && state.is_empty() {
                write!(f, "{}", short_name)
            } else if !path_line.is_empty() && !state.is_empty() {
                write!(f, "{}({}:{})>", short_name, path_line, state)
            } else if !path_line.is_empty() {
                write!(f, "{}({})", short_name, path_line)
            } else {
                write!(f, "{}({})", short_name, state)
            }
        }
    }

    fn parse_fiber_debug(full_str: &str) -> (&str, &str) {
        if let Some(open_paren) = full_str.find(" (") {
            if let Some(space_pos) = full_str.find(' ') {
                if space_pos < open_paren {
                    let path_part = &full_str[space_pos + 1..open_paren];
                    let rest = &full_str[open_paren + 2..]; // skip " ("
                    if let Some(closing_paren) = rest.find(')') {
                        let state = &rest[..closing_paren];
                        return (path_part, state);
                    }
                    return (path_part, "");
                }
            }
            return ("", &full_str[open_paren + 2..]);
        }

        ("", "")
    }

  }
  else if #[cfg(not(debug_assertions))] {
    impl Debug for HeapFiber {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            let raw_id = self.id();
            write!(f, "F({})", raw_id)
        }
    }

  }
}
