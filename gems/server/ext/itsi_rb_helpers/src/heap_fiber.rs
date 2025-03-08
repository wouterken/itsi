use magnus::rb_sys::AsRawValue;
use magnus::value::BoxValue;
use magnus::{Fiber, IntoValue};
use magnus::{Ruby, Value};
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

use crate::heap_value::HeapValue;

#[derive(Clone)]
pub struct HeapFiber(HeapValue<Fiber>);

static FIBER_NAMES: LazyLock<RwLock<HashMap<u64, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static FIBER_COUNTER: AtomicU64 = AtomicU64::new(0);

fn get_fiber_name(id: u64) -> String {
    let mut map = FIBER_NAMES.write().unwrap();
    if let Some(existing_name) = map.get(&id) {
        return existing_name.clone();
    }

    // Not in map yet – assign a new name
    let new_idx = FIBER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = if new_idx == 0 {
        "FMain".to_string()
    } else {
        format!("F{}", new_idx)
    };
    map.insert(id, name.clone());
    name
}

impl From<magnus::Fiber> for HeapFiber {
    fn from(value: magnus::Fiber) -> Self {
        HeapFiber(HeapValue(BoxValue::new(value)))
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

impl Debug for HeapFiber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let raw_id = self.0.as_raw();
        let short_name = get_fiber_name(raw_id);
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
}

impl IntoValue for HeapFiber {
    fn into_value_with(self, _: &Ruby) -> Value {
        self.0.into_value()
    }
}
