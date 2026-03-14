use crate::{
    debug_log, rb_config,
    utils::{is_msvc, shellsplit},
};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    env,
    ffi::{OsStr, OsString},
    fs,
    hash::Hasher,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const WELL_KNOWN_WRAPPERS: &[&str] = &["sccache", "cachepot"];

#[derive(Default, Debug)]
pub struct Build {
    files: Vec<PathBuf>,
    flags: Vec<String>,
}

impl Build {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default_cflags() -> Vec<String> {
        let mut cflags = vec![];

        if cfg!(target_os = "openbsd") {
            cflags.push("-fdeclspec".into());
        } else {
            cflags.push("-fms-extensions".into());
        };

        cflags
    }

    pub fn file(&mut self, file: PathBuf) {
        println!("cargo:rerun-if-changed={}", file.display());
        self.files.push(file);
    }

    pub fn try_compile(self, name: &str) -> Result<()> {
        let compiler = get_compiler();
        let archiver = get_archiver();
        let out_dir = PathBuf::from(env::var("OUT_DIR")?).join("cc");
        fs::create_dir_all(&out_dir)?;
        let rb = rb_config();

        let object_files = self.compile_each_file(compiler, &rb, &out_dir)?;
        debug_log!("INFO: compiled object files: {:?}", object_files);
        let (lib_path, lib_name) =
            self.archive_object_files(archiver.copied(), name, &out_dir, object_files)?;
        if let Err(e) = self.strip_archived_objects(archiver, &lib_path) {
            debug_log!("WARN: failed to strip archived objects: {:?}", e);
        }

        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static={}", lib_name);

        Ok(())
    }

    fn compile_each_file(
        &self,
        compiler: Command,
        rb: &rb_config::RbConfig,
        out_dir: &Path,
    ) -> Result<HashSet<PathBuf>> {
        self.files
            .iter()
            .map(|f| self.compile_file(f, compiler.copied(), rb, out_dir))
            .collect()
    }

    fn compile_file(
        &self,
        f: &Path,
        compiler: Command,
        rb: &rb_config::RbConfig,
        out_dir: &Path,
    ) -> Result<PathBuf> {
        let mut hasher = DefaultHasher::new();
        hasher.write(fs::read(f)?.as_slice());

        let object_file = out_dir
            .join(hasher.finish().to_string())
            .with_extension("o");

        let mut cmd = compiler;
        cmd.args(get_include_args(rb))
            .arg("-c")
            .arg(f)
            .args(&rb.cflags)
            .args(get_common_args())
            .args(&self.flags)
            .args(get_output_file_flag(&object_file));

        run_command(cmd)?;

        Ok(object_file)
    }

    fn archive_object_files(
        &self,
        archiver: Command,
        name: &str,
        out_dir: &Path,
        object_files: HashSet<PathBuf>,
    ) -> Result<(PathBuf, String)> {
        let mut cmd = archiver;
        let mut hasher = DefaultHasher::new();
        object_files
            .iter()
            .for_each(|f| hasher.write(f.to_str().expect("non-utf8 filename").as_bytes()));
        let lib_name = format!("{}-{}", name, hasher.finish());
        let lib_filename = format!("lib{}.a", lib_name);
        let dst = out_dir.join(lib_filename);

        // The argument structure differs for MSVC and GCC.
        if is_msvc() {
            cmd.arg(format!("/OUT:{}", dst.display()));
            cmd.args(&object_files);
        } else {
            cmd.env("ZERO_AR_DATE", "1").arg("crs").arg(&dst);
            cmd.args(&object_files);
        }

        run_command(cmd)?;

        // The Rust compiler will look for libfoo.a and foo.lib, but the
        // MSVC linker will also be passed foo.lib, so be sure that both
        // exist for now.
        if is_msvc() {
            let lib_dst = dst.with_file_name(format!("{}.lib", lib_name));
            let _ = fs::remove_file(&lib_dst);
            match fs::hard_link(&dst, &lib_dst).or_else(|_| {
                // if hard-link fails, just copy (ignoring the number of bytes written)
                fs::copy(&dst, &lib_dst).map(|_| ())
            }) {
                Ok(_) => (),
                Err(_) => {
                    return Err(
                        "Could not copy or create a hard-link to the generated lib file.".into(),
                    );
                }
            };
        }

        Ok((dst, lib_name))
    }

    fn strip_archived_objects(&self, archiver: Command, libpath: &Path) -> Result<()> {
        let mut cmd = archiver;

        if is_msvc() {
            cmd.arg("/LTCG").arg(libpath);
        } else {
            cmd.arg("s").arg(libpath);
        }

        run_command(cmd)?;

        Ok(())
    }
}

fn get_include_args(rb: &rb_config::RbConfig) -> Vec<String> {
    let mut args = vec![];
    if let Some(include_dir) = rb.get("rubyhdrdir") {
        args.push(format!("-I{}", include_dir));
    }
    if let Some(arch_include_dir) = rb.get("rubyarchhdrdir") {
        args.push(format!("-I{}", arch_include_dir));
    }
    if let Some(internal_include_dir) = rb.get("rubyhdrdir") {
        args.push(format!("-I{}/include/internal", internal_include_dir));
    }
    if let Some(impl_include_dir) = rb.get("rubyhdrdir") {
        args.push(format!("-I{}/include/impl", impl_include_dir));
    }

    args
}

fn get_common_args() -> Vec<String> {
    fn add_debug_flags(flags: &mut Vec<String>) {
        match env::var("DEBUG") {
            Ok(val) if val == "true" => {
                if is_msvc() {
                    flags.push("-Z7".into());
                } else if cfg!(target_os = "linux") {
                    flags.push("-gdwarf-4".into());
                } else {
                    flags.push("-gdwarf-2".into());
                }
            }
            _ => {}
        }
    }

    fn add_opt_level(flags: &mut Vec<String>) {
        if let Ok(val) = env::var("OPT_LEVEL") {
            match val.as_str() {
                // Msvc uses /O1 to enable all optimizations that minimize code size.
                "z" | "s" | "1" if is_msvc() => flags.push("-O1".into()),
                // -O3 is a valid value for gcc and clang compilers, but not msvc. Cap to /O2.
                "2" | "3" if is_msvc() => flags.push("-O2".into()),
                lvl => flags.push(format!("-O{}", lvl)),
            }
        }
    }

    fn add_compiler_flags(flags: &mut Vec<String>) {
        if !is_msvc() {
            flags.push("-ffunction-sections".into());
            flags.push("-fdata-sections".into());
            flags.push("-fPIC".into());
            flags.push("-fno-omit-frame-pointer".into());
        }

        flags.extend(Build::default_cflags());
    }

    let mut items = vec![];

    add_debug_flags(&mut items);
    add_compiler_flags(&mut items);
    add_opt_level(&mut items);

    items
}

fn get_compiler() -> Command {
    let cmd = get_tool("CC", "cc");
    let cmd_program = cmd.get_program().to_str().unwrap_or_default();
    let already_wrapped = WELL_KNOWN_WRAPPERS.iter().any(|w| cmd_program.contains(w));

    match get_tool_from_rb_config_or_env("CC_WRAPPER") {
        Some(wrapper) if !wrapper.is_empty() && !already_wrapped => {
            debug_log!("INFO: using CC_WRAPPER ({:?})", wrapper);
            cmd.wrapped(wrapper)
        }
        _ => match rustc_wrapper_fallback() {
            Some(wrapper) if !already_wrapped => cmd.wrapped(wrapper),
            _ => cmd,
        },
    }
}

pub fn rustc_wrapper_fallback() -> Option<String> {
    let rustc_wrapper = std::env::var("RUSTC_WRAPPER").ok()?;
    rustc_wrapper_fallback_detect(rustc_wrapper)
}

pub fn rustc_wrapper_fallback_detect(rustc_wrapper: String) -> Option<String> {
    let wrapper_path = Path::new(&rustc_wrapper);
    let wrapper_stem = wrapper_path.file_stem()?;

    if WELL_KNOWN_WRAPPERS.contains(&wrapper_stem.to_str()?) {
        debug_log!("INFO: using RUSTC_WRAPPER ({:?})", rustc_wrapper);
        Some(rustc_wrapper)
    } else {
        None
    }
}

fn get_archiver() -> Command {
    let cmd = get_tool("AR", "ar");

    if cmd.get_program() == "libtool" {
        new_command("ar")
    } else {
        cmd
    }
}

fn get_tool(env_var: &str, default: &str) -> Command {
    let tool_args = get_tool_from_rb_config_or_env(env_var)
        .unwrap_or_else(|| panic!("no {} tool found", env_var));

    let mut tool_args = shellsplit(tool_args).into_iter();
    let tool = tool_args.next().unwrap_or_else(|| default.to_string());

    fn tool_exists(tool_name: &str) -> std::io::Result<bool> {
        let path = PathBuf::from(tool_name);

        if path.is_file() {
            return Ok(true);
        }

        match Command::new(tool_name).spawn() {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    let mut cmd = if tool_exists(&tool).unwrap_or(false) {
        debug_log!("[INFO] using {tool} for {env_var}");
        new_command(&tool)
    } else {
        debug_log!("[WARN] {tool} tool not found, falling back to {default}");
        new_command(default)
    };

    cmd.args(tool_args.clone());

    debug_log!("INFO: found {:?} tool ({:?})", env_var, &cmd);

    cmd
}

fn get_tool_from_rb_config_or_env(env_var: &str) -> Option<String> {
    let rb = rb_config();

    get_tool_from_env(env_var)
        .filter(|s| !s.is_empty())
        .or_else(|| rb.get(env_var))
}

fn get_tool_from_env(env_var: &str) -> Option<String> {
    let target_slug = env::var("TARGET").ok()?.replace('-', "_");
    let env_var_with_target = format!("{}_{}", env_var, target_slug);

    println!("cargo:rerun-if-env-changed={}", env_var);
    println!("cargo:rerun-if-env-changed={}", env_var_with_target);

    env::var(env_var)
        .or_else(|_| env::var(env_var_with_target))
        .ok()
}

fn run_command(mut cmd: Command) -> Result<ExitStatus> {
    debug_log!("INFO: running command ({:?})", cmd);
    let status = cmd.status()?;

    if !status.success() {
        Err(format!("Command '{:?}' failed with status: {}", cmd, status).into())
    } else {
        Ok(status)
    }
}

fn new_command(name: &str) -> Command {
    let mut cmd = Command::new(name);
    cmd.stderr(Stdio::inherit()).stdout(Stdio::inherit());
    cmd
}

fn get_output_file_flag(file: &Path) -> Vec<OsString> {
    if is_msvc() {
        vec![format!("-Fo{}", file.display()).into()]
    } else {
        vec!["-o".into(), file.into()]
    }
}

pub trait CommandExt {
    fn copied(&self) -> Command;
    fn wrapped<W: AsRef<OsStr>>(&self, wrapper: W) -> Command;
}

impl CommandExt for Command {
    fn copied(&self) -> Command {
        let mut cmd = Command::new(self.get_program());
        cmd.args(self.get_args());

        for (k, v) in self.get_envs() {
            if let Some(v) = v {
                cmd.env(k, v);
            } else {
                cmd.env_remove(k);
            }
        }
        cmd
    }

    fn wrapped<W: AsRef<OsStr>>(&self, wrapper: W) -> Command {
        let mut new_cmd = Command::new(wrapper);

        new_cmd.arg(self.get_program());

        for arg in self.get_args() {
            new_cmd.arg(arg);
        }

        for (k, v) in self.get_envs() {
            if let Some(v) = v {
                new_cmd.env(k, v);
            } else {
                new_cmd.env_remove(k);
            }
        }

        new_cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_wrapper_detect() {
        let wrapper = "/usr/local/bin/sccache";
        assert_eq!(
            rustc_wrapper_fallback_detect(wrapper.to_owned()),
            Some(wrapper.to_owned())
        );
    }
}
