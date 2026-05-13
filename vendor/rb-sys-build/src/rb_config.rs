use std::{
    collections::{hash_map::Keys, HashMap},
    env,
    path::PathBuf,
    process::Command,
};

use regex::Regex;
mod flags;
mod library;
mod search_path;

use library::*;
use search_path::*;
use std::ffi::OsString;

use crate::{
    debug_log, memoize,
    utils::{is_msvc, shellsplit},
};

use self::flags::Flags;

/// Extracts structured information from raw compiler/linker flags to make
/// compiling Ruby gems easier.
#[derive(Debug, PartialEq, Eq)]
pub struct RbConfig {
    pub search_paths: Vec<SearchPath>,
    pub libs: Vec<Library>,
    pub link_args: Vec<String>,
    pub cflags: Vec<String>,
    pub blocklist_lib: Vec<String>,
    pub blocklist_link_arg: Vec<String>,
    use_rpath: bool,
    value_map: HashMap<String, String>,
}

impl Default for RbConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl RbConfig {
    /// Creates a new, blank `RbConfig`. You likely want to use `RbConfig::current()` instead.
    pub(crate) fn new() -> RbConfig {
        RbConfig {
            blocklist_lib: vec![],
            blocklist_link_arg: vec![],
            search_paths: Vec::new(),
            libs: Vec::new(),
            link_args: Vec::new(),
            cflags: Vec::new(),
            value_map: HashMap::new(),
            use_rpath: false,
        }
    }

    /// All keys in the `RbConfig`'s value map.
    pub fn all_keys(&self) -> Keys<'_, String, String> {
        self.value_map.keys()
    }

    /// Instantiates a new `RbConfig` for the current Ruby.
    pub fn current() -> RbConfig {
        println!("cargo:rerun-if-env-changed=RUBY");

        let mut rbconfig = RbConfig::new();

        // Never use the current Ruby's RbConfig if we're cross compiling, or
        // else bad things happen
        let parsed = if rbconfig.is_cross_compiling() {
            HashMap::new()
        } else {
            let output = memoize!(String: {
                let ruby = env::var_os("RUBY").unwrap_or_else(|| OsString::from("ruby"));

                let config = Command::new(ruby)
                    .arg("--disable-gems")
                    .arg("-rrbconfig")
                    .arg("-e")
                    .arg("print RbConfig::CONFIG.map {|kv| kv.join(\"\x1F\")}.join(\"\x1E\")")
                    .output()
                    .unwrap_or_else(|e| panic!("ruby not found: {}", e));
                if !config.status.success() {
                    panic!("non-zero exit status while dumping RbConfig: {:?}", config);
                }
                String::from_utf8(config.stdout).expect("RbConfig value not UTF-8!")
            });

            let mut parsed = HashMap::new();
            for line in output.split('\x1E') {
                let mut parts = line.splitn(2, '\x1F');
                if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                    parsed.insert(key.to_owned(), val.to_owned());
                }
            }
            parsed
        };

        parsed.get("cflags").map(|f| rbconfig.push_cflags(f));
        parsed.get("DLDFLAGS").map(|f| rbconfig.push_dldflags(f));

        rbconfig.value_map = parsed;

        rbconfig
    }

    /// Pushes the `LIBRUBYARG` flags so Ruby will be linked.
    pub fn link_ruby(&mut self, is_static: bool) -> &mut Self {
        let Some(libdir) = self.get("libdir") else {
            return self;
        };

        self.push_search_path(libdir.as_str());
        self.push_dldflags(&format!("-L{}", libdir));

        let librubyarg = if is_static {
            self.get("LIBRUBYARG_STATIC")
        } else {
            self.get("LIBRUBYARG_SHARED")
        };

        let librubyarg = match librubyarg {
            Some(lib) => lib,
            None => {
                debug_log!("WARN: LIBRUBYARG not found in RbConfig, skipping linking Ruby");
                return self;
            }
        };

        if is_msvc() {
            for lib in librubyarg.split_whitespace() {
                self.push_library(lib);
            }

            let mut to_link: Vec<String> = vec![];

            if let Some(libs) = self.get("LIBS") {
                to_link.extend(libs.split_whitespace().map(|s| s.to_string()));
            }

            if let Some(libs) = self.get("LOCAL_LIBS") {
                to_link.extend(libs.split_whitespace().map(|s| s.to_string()));
            }

            for lib in to_link {
                self.push_library(lib);
            }
        } else {
            self.push_dldflags(&librubyarg);

            if cfg!(unix) {
                self.use_rpath();
            }
        }

        self
    }

    /// Get the name for libruby-static (i.e. `ruby.3.1-static`).
    pub fn libruby_static_name(&self) -> String {
        let Some(lib) = self.get("LIBRUBY_A") else {
            return format!("{}-static", self.libruby_so_name());
        };

        lib.trim_start_matches("lib")
            .trim_end_matches(".a")
            .to_string()
    }

    /// Get the name for libruby (i.e. `ruby.3.1`)
    pub fn libruby_so_name(&self) -> String {
        self.get("RUBY_SO_NAME")
            .unwrap_or_else(|| "ruby".to_string())
    }

    /// Get the platform for the current ruby.
    pub fn platform(&self) -> String {
        self.get("platform")
            .unwrap_or_else(|| self.get("arch").expect("arch not found"))
    }

    /// Filter the libs, removing the ones that are not needed.
    pub fn blocklist_lib(&mut self, name: &str) -> &mut RbConfig {
        self.blocklist_lib.push(name.to_string());
        self
    }

    /// Blocklist a link argument.
    pub fn blocklist_link_arg(&mut self, name: &str) -> &mut RbConfig {
        self.blocklist_link_arg.push(name.to_string());
        self
    }

    /// Returns the current ruby program version.
    pub fn ruby_version_slug(&self) -> String {
        let ver = if let Some(progv) = self.get("RUBY_PROGRAM_VERSION") {
            progv
        } else if let Some(major_minor) = self.major_minor() {
            format!(
                "{}.{}.{}",
                major_minor.0,
                major_minor.1,
                self.get("TEENY").unwrap_or_else(|| "0".to_string())
            )
        } else if let Some(fallback) = self.get("ruby_version") {
            fallback
        } else {
            panic!("RUBY_PROGRAM_VERSION not found")
        };

        format!("{}-{}-{}", self.ruby_engine(), self.platform(), ver)
    }

    /// Get the CPPFLAGS from the RbConfig, making sure to subsitute variables.
    pub fn cppflags(&self) -> Vec<String> {
        if let Some(cppflags) = self.get("CPPFLAGS") {
            let flags = self.subst_shell_variables(&cppflags);
            shellsplit(flags)
        } else {
            vec![]
        }
    }

    /// Returns true if the current Ruby is cross compiling.
    pub fn is_cross_compiling(&self) -> bool {
        if let Some(cross) = self.get("CROSS_COMPILING") {
            cross == "yes" || cross == "1"
        } else {
            false
        }
    }

    /// Returns the value of the given key from the either the matching
    /// `RBCONFIG_{key}` environment variable or `RbConfig::CONFIG[{key}]` hash.
    pub fn get(&self, key: &str) -> Option<String> {
        self.try_rbconfig_env(key)
            .or_else(|| self.try_value_map(key))
    }

    /// Enables the use of rpath for linking.
    pub fn use_rpath(&mut self) -> &mut RbConfig {
        self.use_rpath = true;
        self
    }

    /// Push cflags string
    pub fn push_cflags(&mut self, cflags: &str) -> &mut Self {
        for flag in shellsplit(cflags) {
            if !self.cflags.contains(&flag) {
                self.cflags.push(flag.to_string());
            }
        }

        self
    }

    /// Get major/minor version tuple of Ruby
    pub fn major_minor(&self) -> Option<(u32, u32)> {
        let major = self.get("MAJOR").map(|v| v.parse::<u32>())?.ok()?;
        let minor = self.get("MINOR").map(|v| v.parse::<u32>())?.ok()?;
        Some((major, minor))
    }

    /// Get the rb_config output for cargo
    pub fn cargo_args(&self) -> Vec<String> {
        let mut result = vec![];

        let mut search_paths = vec![];

        for search_path in &self.search_paths {
            result.push(format!("cargo:rustc-link-search={}", search_path));
            search_paths.push(search_path.name.as_str());
        }

        for lib in &self.libs {
            if !self.blocklist_lib.iter().any(|b| lib.name.contains(b)) {
                result.push(format!("cargo:rustc-link-lib={}", lib));
            }

            if self.use_rpath && !lib.is_static() {
                result.push(format!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib));
            }
        }

        for link_arg in &self.link_args {
            if !self.blocklist_link_arg.iter().any(|b| link_arg == b) {
                result.push(format!("cargo:rustc-link-arg={}", link_arg));
            }
        }

        result
    }

    /// Print to rb_config output for cargo
    pub fn print_cargo_args(&self) {
        let cargo_args = self.cargo_args();

        for arg in &cargo_args {
            println!("{}", arg);
        }

        debug_log!("INFO: printing cargo args ({:?})", cargo_args);

        let encoded_cargo_args = cargo_args.join("\x1E");
        let encoded_cargo_args = encoded_cargo_args.replace('\n', "\x1F");

        println!("cargo:encoded_cargo_args={}", encoded_cargo_args);
    }

    /// Adds items to the rb_config based on a string from LDFLAGS/DLDFLAGS
    pub fn push_dldflags(&mut self, input: &str) -> &mut Self {
        let input = self.subst_shell_variables(input);
        let split_args = Flags::new(input.as_str());

        let search_path_regex = Regex::new(r"^-L\s*(?P<name>.*)$").unwrap();
        let lib_regex_short = Regex::new(r"^-l\s*(?P<name>\w+\S+)$").unwrap();
        let lib_regex_long = Regex::new(r"^--library=(?P<name>\w+\S+)$").unwrap();
        let dynamic_lib_regex = Regex::new(r"^-l\s*:lib(?P<name>\S+).(so|dylib|dll)$").unwrap();
        let framework_regex_short = Regex::new(r"^-F\s*(?P<name>.*)$").unwrap();
        let framework_regex_long = Regex::new(r"^-framework\s*(?P<name>.*)$").unwrap();

        for arg in split_args {
            let arg = arg.trim().to_owned();

            if let Some(name) = capture_name(&search_path_regex, &arg) {
                self.push_search_path(name.as_str());
            } else if let Some(name) = capture_name(&lib_regex_long, &arg) {
                self.push_library(name);
            } else if let Some(name) = capture_name(&lib_regex_short, &arg) {
                if name.contains("ruby") && name.contains("-static") {
                    self.push_library((LibraryKind::Static, name));
                } else {
                    self.push_library(name);
                }
            } else if let Some(name) = capture_name(&dynamic_lib_regex, &arg) {
                self.push_library((LibraryKind::Dylib, name));
            } else if let Some(name) = capture_name(&framework_regex_short, &arg) {
                self.push_search_path((SearchPathKind::Framework, name));
            } else if let Some(name) = capture_name(&framework_regex_long, &arg) {
                self.push_library((LibraryKind::Framework, name));
            } else {
                self.push_link_arg(arg);
            }
        }

        self
    }

    /// Sets a value for a key
    pub fn set_value_for_key(&mut self, key: &str, value: String) {
        self.value_map.insert(key.to_owned(), value);
    }

    // Check if has ABI version
    pub fn has_ruby_dln_check_abi(&self) -> bool {
        let Some((major, minor)) = self.major_minor() else {
            return false;
        };

        let patchlevel = self
            .get("PATCHLEVEL")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(-1);

        // Ruby has ABI version on version 3.2 and later only on development
        // versions
        (major > 3 || (major == 3 && minor >= 2))
            && patchlevel == -1
            && !cfg!(target_family = "windows")
    }

    /// The RUBY_ENGINE we are building for
    pub fn ruby_engine(&self) -> RubyEngine {
        if let Some(engine) = self.get("ruby_install_name") {
            match engine.as_str() {
                "ruby" => RubyEngine::Mri,
                "jruby" => RubyEngine::JRuby,
                "truffleruby" => RubyEngine::TruffleRuby,
                _ => RubyEngine::Mri, // not sure how stable this is, so default to MRI to avoid breaking things
            }
        } else {
            RubyEngine::Mri
        }
    }

    // Examines the string from shell variables and expands them with values in the value_map
    fn subst_shell_variables(&self, input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().enumerate();

        while let Some((_, c)) = chars.next() {
            if c == '$' {
                if let Some((i, c)) = chars.next() {
                    if c == '(' {
                        let start = i + 1;
                        let mut end = start;

                        for (i, c) in chars.by_ref() {
                            if c == ')' {
                                end = i;
                                break;
                            }
                        }

                        let key = &input[start..end];

                        if let Some(val) = self.get(key) {
                            result.push_str(&val);
                        } else if let Some(val) = env::var_os(key) {
                            result.push_str(&val.to_string_lossy());
                        } else {
                            // Consume whitespace
                            chars.next();
                        }
                    } else {
                        result.push(c);
                    }
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    pub fn have_ruby_header<T: AsRef<str>>(&self, header: T) -> bool {
        let Some(ruby_include_dir) = self.get("rubyhdrdir") else {
            return false;
        };
        PathBuf::from(ruby_include_dir)
            .join(header.as_ref())
            .exists()
    }

    fn push_search_path<T: Into<SearchPath>>(&mut self, path: T) -> &mut Self {
        let path = path.into();

        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }

        self
    }

    fn push_library<T: Into<Library>>(&mut self, lib: T) -> &mut Self {
        let lib = lib.into();

        if !self.libs.contains(&lib) {
            self.libs.push(lib);
        }

        self
    }

    fn push_link_arg<T: Into<String>>(&mut self, arg: T) -> &mut Self {
        let arg = arg.into();

        if !self.link_args.contains(&arg) {
            self.link_args.push(arg);
        }

        self
    }

    fn try_value_map(&self, key: &str) -> Option<String> {
        self.value_map
            .get(key)
            .map(|val| val.trim_matches('\n').to_owned())
    }

    fn try_rbconfig_env(&self, key: &str) -> Option<String> {
        let key = format!("RBCONFIG_{}", key);
        println!("cargo:rerun-if-env-changed={}", key);
        env::var(key).map(|v| v.trim_matches('\n').to_owned()).ok()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RubyEngine {
    Mri,
    TruffleRuby,
    JRuby,
}

impl std::fmt::Display for RubyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RubyEngine::Mri => write!(f, "mri"),
            RubyEngine::TruffleRuby => write!(f, "truffleruby"),
            RubyEngine::JRuby => write!(f, "jruby"),
        }
    }
}

fn capture_name(regex: &Regex, arg: &str) -> Option<String> {
    regex
        .captures(arg)
        .map(|cap| cap.name("name").unwrap().as_str().trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Mutex, vec};

    lazy_static::lazy_static! {
        static ref ENV_LOCK: Mutex<()> = Mutex::new(());
    }

    fn with_locked_env<F, T>(f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _guard = ENV_LOCK.lock().unwrap();
        f()
    }

    #[test]
    fn test_extract_lib_search_paths() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L/usr/local/lib -L/usr/lib");
        assert_eq!(
            rb_config.search_paths,
            vec!["/usr/local/lib".into(), "/usr/lib".into()]
        );
    }

    #[test]
    fn test_search_path_basic() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L/usr/local/lib");

        assert_eq!(rb_config.search_paths, vec!["native=/usr/local/lib".into()]);
    }

    #[test]
    fn test_search_path_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L /usr/local/lib");

        assert_eq!(rb_config.search_paths, vec!["/usr/local/lib".into()]);
    }

    #[test]
    fn test_search_path_space_in_path() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L/usr/local/my lib");

        assert_eq!(
            rb_config.search_paths,
            vec!["native=/usr/local/my lib".into()]
        );
    }

    #[test]
    fn test_simple_lib() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-lfoo");

        assert_eq!(rb_config.libs, ["foo".into()]);
    }

    #[test]
    fn test_lib_with_nonascii() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-lws2_32");

        assert_eq!(rb_config.libs, ["ws2_32".into()]);
    }

    #[test]
    fn test_simple_lib_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-l foo");

        assert_eq!(rb_config.libs, ["foo".into()]);
    }

    #[test]
    fn test_verbose_lib_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("--library=foo");

        assert_eq!(rb_config.libs, ["foo".into()]);
    }

    #[test]
    fn test_dylib_with_colon_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-l :libssp.dylib");

        assert_eq!(rb_config.libs, ["dylib=ssp".into()]);
    }

    #[test]
    fn test_so_with_colon_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-l :libssp.so");

        assert_eq!(rb_config.libs, ["dylib=ssp".into()]);
    }

    #[test]
    fn test_dll_with_colon_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-l :libssp.dll");

        assert_eq!(rb_config.libs, ["dylib=ssp".into()]);
    }

    #[test]
    fn test_framework() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-F/some/path");

        assert_eq!(rb_config.search_paths, ["framework=/some/path".into()]);
    }

    #[test]
    fn test_framework_space() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-F /some/path");

        assert_eq!(
            rb_config.search_paths,
            [SearchPath {
                kind: SearchPathKind::Framework,
                name: "/some/path".into(),
            }]
        );
    }

    #[test]
    fn test_framework_arg_real() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-framework CoreFoundation");

        assert_eq!(
            rb_config.libs,
            [Library {
                kind: LibraryKind::Framework,
                name: "CoreFoundation".into(),
            }]
        );
    }

    #[test]
    fn test_libruby_static() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-lruby.3.1-static");

        assert_eq!(
            rb_config.cargo_args(),
            ["cargo:rustc-link-lib=static=ruby.3.1-static"]
        );
    }

    #[test]
    fn test_libruby_dynamic() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-lruby.3.1");

        assert_eq!(rb_config.cargo_args(), ["cargo:rustc-link-lib=ruby.3.1"]);
    }

    #[test]
    fn test_non_lib_dash_l() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("test_rubygems_20220413-976-lemgf9/prefix");

        assert_eq!(
            rb_config.link_args,
            vec!["test_rubygems_20220413-976-lemgf9/prefix"]
        );
    }

    #[test]
    fn test_real_dldflags() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L/Users/ianks/.asdf/installs/ruby/3.1.1/lib -L/opt/homebrew/opt/openssl@1.1/lib -Wl,-undefined,dynamic_lookup -Wl,-multiply_defined,suppress");

        assert_eq!(
            rb_config.link_args,
            vec![
                "-Wl,-undefined,dynamic_lookup",
                "-Wl,-multiply_defined,suppress"
            ]
        );
        assert_eq!(
            rb_config.search_paths,
            vec![
                SearchPath {
                    kind: SearchPathKind::Native,
                    name: "/Users/ianks/.asdf/installs/ruby/3.1.1/lib".to_string()
                },
                SearchPath {
                    kind: SearchPathKind::Native,
                    name: "/opt/homebrew/opt/openssl@1.1/lib".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_crazy_cases() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-F   /something -l:libssp.a -static-libgcc ");

        assert_eq!(rb_config.link_args, vec!["-l:libssp.a", "-static-libgcc"]);
        assert_eq!(
            rb_config.search_paths,
            vec![SearchPath {
                kind: SearchPathKind::Framework,
                name: "/something".to_string()
            },]
        );
    }

    #[test]
    fn test_printing_cargo_args() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-L/Users/ianks/.asdf/installs/ruby/3.1.1/lib");
        rb_config.push_dldflags("-lfoo");
        rb_config.push_dldflags("-static-libgcc");
        let result = rb_config.cargo_args();

        assert_eq!(
            vec![
                "cargo:rustc-link-search=native=/Users/ianks/.asdf/installs/ruby/3.1.1/lib",
                "cargo:rustc-link-lib=foo",
                "cargo:rustc-link-arg=-static-libgcc"
            ],
            result
        );
    }

    #[test]
    fn test_use_rpath() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-lfoo");

        assert_eq!(vec!["cargo:rustc-link-lib=foo"], rb_config.cargo_args());

        rb_config.use_rpath();

        assert_eq!(
            vec![
                "cargo:rustc-link-lib=foo",
                "cargo:rustc-link-arg=-Wl,-rpath,foo"
            ],
            rb_config.cargo_args()
        );
    }

    #[test]
    fn test_link_mswin() {
        with_locked_env(|| {
            let old_var = env::var("TARGET").ok();
            env::set_var("TARGET", "x86_64-pc-windows-msvc");

            let mut rb_config = RbConfig::new();
            rb_config.set_value_for_key("LIBRUBYARG_SHARED", "x64-vcruntime140-ruby320.lib".into());
            rb_config.set_value_for_key("libdir", "D:/ruby-mswin/lib".into());
            rb_config.set_value_for_key("LIBS", "user32.lib".into());
            rb_config.link_ruby(false);

            assert_eq!(
                vec![
                    "cargo:rustc-link-search=native=D:/ruby-mswin/lib",
                    "cargo:rustc-link-lib=x64-vcruntime140-ruby320",
                    "cargo:rustc-link-lib=user32",
                ],
                rb_config.cargo_args()
            );

            if let Some(old_var) = old_var {
                env::set_var("TARGET", old_var);
            } else {
                env::remove_var("TARGET");
            }
        })
    }

    #[test]
    fn test_link_static() {
        with_locked_env(|| {
            let mut rb_config = RbConfig::new();
            rb_config.set_value_for_key("LIBRUBYARG_STATIC", "-lruby-static".into());
            rb_config.set_value_for_key("libdir", "/opt/ruby".into());

            rb_config.link_ruby(true);

            assert_eq!(
                vec![
                    "cargo:rustc-link-search=native=/opt/ruby",
                    "cargo:rustc-link-lib=static=ruby-static",
                ],
                rb_config.cargo_args()
            );
        });
    }

    #[test]
    fn test_prioritizes_rbconfig_env() {
        with_locked_env(|| {
            env::set_var("RBCONFIG_libdir", "/foo");
            let rb_config = RbConfig::new();

            assert_eq!(rb_config.get("libdir"), Some("/foo".into()));

            env::remove_var("RBCONFIG_libdir");
        });
    }

    #[test]
    fn test_never_loads_shell_rbconfig_if_cross_compiling() {
        with_locked_env(|| {
            env::set_var("RBCONFIG_CROSS_COMPILING", "yes");

            let rb_config = RbConfig::current();

            assert!(rb_config.value_map.is_empty());
        });
    }

    #[test]
    fn test_loads_shell_rbconfig_if_not_cross_compiling() {
        with_locked_env(|| {
            env::set_var("RBCONFIG_CROSS_COMPILING", "no");

            let rb_config = RbConfig::current();

            assert!(!rb_config.value_map.is_empty());
        });
    }

    #[test]
    fn test_libstatic() {
        let mut rb_config = RbConfig::new();
        rb_config.push_dldflags("-l:libssp.a");

        assert_eq!(rb_config.link_args, ["-l:libssp.a".to_string()]);
    }

    #[test]
    fn test_link_arg_blocklist() {
        let mut rb_config = RbConfig::new();
        rb_config.blocklist_link_arg("-Wl,--compress-debug-sections=zlib");
        rb_config.blocklist_link_arg("-s");
        rb_config.push_dldflags(
            "-lfoo -Wl,--compress-debug-sections=zlib -s -somethingthatshouldnotbeblocked",
        );

        assert_eq!(
            vec![
                "cargo:rustc-link-lib=foo",
                "cargo:rustc-link-arg=-somethingthatshouldnotbeblocked"
            ],
            rb_config.cargo_args()
        );
    }

    #[test]
    fn test_has_ruby_dln_check_abi() {
        // Helper to create RbConfig with specific version
        fn make_config(major: &str, minor: &str, patchlevel: &str) -> RbConfig {
            let mut rb_config = RbConfig::new();
            rb_config.set_value_for_key("MAJOR", major.into());
            rb_config.set_value_for_key("MINOR", minor.into());
            rb_config.set_value_for_key("PATCHLEVEL", patchlevel.into());
            rb_config
        }

        // Ruby 3.1.x (any patchlevel) - too old
        assert!(!make_config("3", "1", "-1").has_ruby_dln_check_abi());
        assert!(!make_config("3", "1", "0").has_ruby_dln_check_abi());

        // Ruby 3.2.0-dev (patchlevel -1) - should have ABI check
        #[cfg(not(target_family = "windows"))]
        assert!(make_config("3", "2", "-1").has_ruby_dln_check_abi());

        // Ruby 3.2.0 release (patchlevel 0) - no ABI check
        assert!(!make_config("3", "2", "0").has_ruby_dln_check_abi());

        // Ruby 3.3.0-dev - should have ABI check
        #[cfg(not(target_family = "windows"))]
        assert!(make_config("3", "3", "-1").has_ruby_dln_check_abi());

        // Ruby 4.0.0-dev - should have ABI check (this was the bug!)
        #[cfg(not(target_family = "windows"))]
        assert!(make_config("4", "0", "-1").has_ruby_dln_check_abi());

        // Ruby 4.0.0 release - no ABI check
        assert!(!make_config("4", "0", "0").has_ruby_dln_check_abi());

        // Ruby 4.1.0-dev - should have ABI check
        #[cfg(not(target_family = "windows"))]
        assert!(make_config("4", "1", "-1").has_ruby_dln_check_abi());

        // Ruby 2.7.x - too old
        assert!(!make_config("2", "7", "-1").has_ruby_dln_check_abi());
    }
}
