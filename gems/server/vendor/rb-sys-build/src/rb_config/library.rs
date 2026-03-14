/// Represents the kind of library.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LibraryKind {
    Framework,
    Dylib,
    Static,
    None,
}

/// Represents a search path that can be linked with Cargo.
#[derive(Debug, PartialEq, Eq)]
pub struct Library {
    pub kind: LibraryKind,
    pub name: String,
}

impl Library {
    pub fn is_static(&self) -> bool {
        self.kind == LibraryKind::Static
    }
}

impl From<&str> for LibraryKind {
    fn from(s: &str) -> Self {
        match s {
            "framework" => LibraryKind::Framework,
            "dylib" => LibraryKind::Dylib,
            "static" => LibraryKind::Static,
            _ => LibraryKind::None,
        }
    }
}

impl From<&str> for Library {
    fn from(s: &str) -> Self {
        let parts: Vec<_> = s.splitn(2, '=').collect();

        match parts.len() {
            1 => (LibraryKind::None, parts[0]).into(),
            2 => (parts[0], parts[1]).into(),
            _ => panic!("Invalid library specification: {}", s),
        }
    }
}

impl From<String> for Library {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

fn sanitize_library_name(name: &str) -> &str {
    name.trim_end_matches(".lib").trim_start_matches("-l")
}

impl<K, L> From<(K, L)> for Library
where
    K: Into<LibraryKind>,
    L: Into<String>,
{
    fn from((kind, name): (K, L)) -> Self {
        Self {
            kind: kind.into(),
            name: sanitize_library_name(&name.into()).into(),
        }
    }
}

impl std::fmt::Display for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.kind {
            LibraryKind::Framework => write!(f, "framework={}", self.name),
            LibraryKind::Dylib => write!(f, "dylib={}", self.name),
            LibraryKind::Static => write!(f, "static={}", self.name),
            LibraryKind::None => write!(f, "{}", self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_leading_link_flag() {
        let result: Library = "-lfoo".to_string().into();

        assert_eq!(result.name, "foo");
    }

    #[test]
    fn test_trim_trailing_lib_extension() {
        let result: Library = "foo.lib".to_string().into();

        assert_eq!(result.name, "foo");
    }

    #[test]
    fn test_trim_leading_link_flag_and_trailing_lib_extension() {
        let result: Library = "-lfoo.lib".to_string().into();

        assert_eq!(result.name, "foo");
    }

    #[test]
    fn test_display_framework() {
        let result: Library = "framework=foo".to_string().into();

        assert_eq!(result.to_string(), "framework=foo");
    }

    #[test]
    fn test_display_dylib() {
        let result: Library = "dylib=foo".to_string().into();

        assert_eq!(result.to_string(), "dylib=foo");
    }

    #[test]
    fn test_display_static() {
        let result: Library = "static=-lfoo".to_string().into();

        assert_eq!(result.to_string(), "static=foo");
    }

    #[test]
    fn test_display_none() {
        let result: Library = "foo".to_string().into();

        assert_eq!(result.to_string(), "foo");
    }
}
