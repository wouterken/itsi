/// Represents the kind of search path.
#[derive(Debug, PartialEq, Eq)]
pub enum SearchPathKind {
    Native,
    Framework,
}

/// Represents a library taht can be linked with Cargo.
#[derive(Debug, PartialEq, Eq)]
pub struct SearchPath {
    pub kind: SearchPathKind,
    pub name: String,
}

impl From<&str> for SearchPathKind {
    fn from(s: &str) -> Self {
        match s {
            "framework" => SearchPathKind::Framework,
            "native" => SearchPathKind::Native,
            _ => panic!("Unknown lib kind: {}", s),
        }
    }
}

impl From<&str> for SearchPath {
    fn from(s: &str) -> Self {
        let parts: Vec<_> = s.split('=').collect();

        match parts.len() {
            1 => (SearchPathKind::Native, parts[0]).into(),
            2 => (parts[0], parts[1]).into(),
            _ => panic!("Invalid library specification: {}", s),
        }
    }
}

impl<K, T> From<(K, T)> for SearchPath
where
    K: Into<SearchPathKind>,
    T: Into<String>,
{
    fn from((kind, name): (K, T)) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
        }
    }
}

impl std::fmt::Display for SearchPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            SearchPathKind::Framework => write!(f, "framework={}", self.name),
            SearchPathKind::Native => write!(f, "native={}", self.name),
        }
    }
}
