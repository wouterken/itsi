/// Some helper functionality around shell flags
pub struct Flags<'a> {
    inner: &'a str,
}

impl<'a> Flags<'a> {
    /// Creates a new `Flags` instance
    pub fn new(inner: &'a str) -> Self {
        Self { inner }
    }
}

/// Iterates over a string of flags
impl<'a> Iterator for Flags<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let mut last_was_space = false;

        let last_idx = self
            .inner
            .chars()
            .by_ref()
            .take_while(|c| match c {
                '-' => {
                    if last_was_space {
                        false
                    } else {
                        last_was_space = false;
                        true
                    }
                }
                ' ' => {
                    last_was_space = true;

                    true
                }
                _ => {
                    last_was_space = false;
                    true
                }
            })
            .count();

        let buf = &self.inner[..last_idx].trim();

        if buf.is_empty() {
            None
        } else {
            self.inner = self.inner[last_idx..].trim();
            Some(buf)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_flags() {
        let mut flags = Flags::new("--foo --bar -baz");

        assert_eq!(flags.next(), Some("--foo"));
        assert_eq!(flags.next(), Some("--bar"));
        assert_eq!(flags.next(), Some("-baz"));
        assert_eq!(flags.next(), None);
    }

    #[test]
    fn test_flag_variations() {
        let mut flags = Flags::new("-ltest     --library test");

        assert_eq!(flags.next(), Some("-ltest"));
        assert_eq!(flags.next(), Some("--library test"));
        assert_eq!(flags.next(), None);
    }

    #[test]
    fn test_real_ldflags() {
        let mut flags = Flags::new("-L. -L/Users/ianks/.asdf/installs/ruby/3.1.1/lib -L/opt/homebrew/opt/openssl@1.1/lib -fstack-protector-strong");

        assert_eq!(flags.next(), Some("-L."));
        assert_eq!(
            flags.next(),
            Some("-L/Users/ianks/.asdf/installs/ruby/3.1.1/lib")
        );
        assert_eq!(flags.next(), Some("-L/opt/homebrew/opt/openssl@1.1/lib"));
        assert_eq!(flags.next(), Some("-fstack-protector-strong"));
        assert_eq!(flags.next(), None);
    }

    #[test]
    fn test_dashed_flag_with_dashed_val() {
        let mut flags = Flags::new("-ltest -fsomething-foo bar-val");

        assert_eq!(flags.next(), Some("-ltest"));
        assert_eq!(flags.next(), Some("-fsomething-foo bar-val"));
        assert_eq!(flags.next(), None);
    }
}
