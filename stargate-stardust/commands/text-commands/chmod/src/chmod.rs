

use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use thiserror::Error;
use sgcore::display::Quotable;
use sgcore::error::{ExitCode, SGError, SGResult, SGSimpleError, SGUsageError, set_exit_code};
use sgcore::fs::display_permissions_unix;
use sgcore::libc::mode_t;
use sgcore::mode;
use sgcore::perms::{TraverseSymlinks, configure_symlink_and_recursion};

#[cfg(target_os = "linux")]
use sgcore::safe_traversal::DirFd;
use sgcore::{format_usage, show, show_error};

use sgcore::translate;

#[derive(Debug, Error)]
enum ChmodError {
    #[error("{}", translate!("chmod-error-cannot-stat", "file" => _0.quote()))]
    CannotStat(String),
    #[error("{}", translate!("chmod-error-dangling-symlink", "file" => _0.quote()))]
    DanglingSymlink(String),
    #[error("{}", translate!("chmod-error-no-such-file", "file" => _0.quote()))]
    NoSuchFile(String),
    #[error("{}", translate!("chmod-error-preserve-root", "file" => _0.quote()))]
    PreserveRoot(String),
    #[error("{}", translate!("chmod-error-permission-denied", "file" => _0.quote()))]
    PermissionDenied(String),
    #[error("{}", translate!("chmod-error-new-permissions", "file" => _0.clone(), "actual" => _1.clone(), "expected" => _2.clone()))]
    NewPermissions(String, String, String),
}

impl SGError for ChmodError {}

mod options {
    pub const HELP: &str = "help";
    pub const CHANGES: &str = "changes";
    pub const QUIET: &str = "quiet";
    pub const VERBOSE: &str = "verbose";
    pub const NO_PRESERVE_ROOT: &str = "no-preserve-root";
    pub const PRESERVE_ROOT: &str = "preserve-root";
    pub const REFERENCE: &str = "RFILE";
    pub const RECURSIVE: &str = "recursive";
    pub const MODE: &str = "MODE";
    pub const FILE: &str = "FILE";
}

/// Extract negative modes (starting with '-') from the rest of the arguments.
///
/// This is mainly required for GNU compatibility, where "non-positional negative" modes are used
/// as the actual positional MODE. Some examples of these cases are:
/// * "chmod -w -r file", which is the same as "chmod -w,-r file"
/// * "chmod -w file -r", which is the same as "chmod -w,-r file"
///
/// These can currently not be handled by clap.
/// Therefore it might be possible that a pseudo MODE is inserted to pass clap parsing.
/// The pseudo MODE is later replaced by the extracted (and joined) negative modes.
fn extract_negative_modes(mut args: impl sgcore::Args) -> (Option<String>, Vec<OsString>) {
    let (parsed_cmode_vec, pre_double_hyphen_args): (Vec<OsString>, Vec<OsString>) =
        args.by_ref().take_while(|a| a != "--").partition(|arg| {
            let arg = if let Some(arg) = arg.to_str() {
                arg.to_string()
            } else {
                return false;
            };
            arg.len() >= 2
                && arg.starts_with('-')
                && matches!(
                    arg.chars().nth(1).unwrap(),
                    'r' | 'w' | 'x' | 'X' | 's' | 't' | 'u' | 'g' | 'o' | '0'..='7'
                )
        });

    let mut clean_args = Vec::new();
    if !parsed_cmode_vec.is_empty() {
        clean_args.push("w".into());
    }
    clean_args.extend(pre_double_hyphen_args);

    if let Some(arg) = args.next() {
        clean_args.push("--".into());
        clean_args.push(arg);
    }
    clean_args.extend(args);

    let parsed_cmode = Some(
        parsed_cmode_vec
            .iter()
            .map(|s| s.to_str().unwrap())
            .collect::<Vec<&str>>()
            .join(",")
    )
    .filter(|s| !s.is_empty());
    (parsed_cmode, clean_args)
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let (parsed_cmode, args) = extract_negative_modes(args.skip(1));
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "fattr"])?;

    let changes = matches.get_flag(options::CHANGES);
    let quiet = matches.get_flag(options::QUIET);
    let verbose = matches.get_flag(options::VERBOSE);
    let preserve_root = matches.get_flag(options::PRESERVE_ROOT);
    let fmode = match matches.get_one::<OsString>(options::REFERENCE) {
        Some(fref) => match fs::metadata(fref) {
            Ok(meta) => Some(meta.mode() & 0o7777),
            Err(_) => {
                return Err(ChmodError::CannotStat(fref.to_string_lossy().to_string()).into());
            }
        },
        None => None,
    };

    let modes = matches.get_one::<String>(options::MODE);
    let cmode = if let Some(parsed_cmode) = parsed_cmode {
        parsed_cmode
    } else {
        modes.unwrap().to_owned()
    };
    let mut files: Vec<OsString> = matches
        .get_many::<OsString>(options::FILE)
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    let cmode = if fmode.is_some() {
        files.push(OsString::from(cmode));
        None
    } else {
        Some(cmode)
    };

    if files.is_empty() {
        return Err(SGUsageError::new(
            1,
            translate!("chmod-error-missing-operand")
        ));
    }

    let (recursive, dereference, traverse_symlinks) =
        configure_symlink_and_recursion(&matches, TraverseSymlinks::First)?;

    let chmoder = Chmoder {
        changes,
        quiet,
        verbose,
        preserve_root,
        recursive,
        fmode,
        cmode,
        traverse_symlinks,
        dereference,
    };

    chmoder.chmod(&files)
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about(translate!("chmod-about"))
        .override_usage(format_usage(&translate!("chmod-usage")))
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .args_override_self(true)
        .infer_long_args(true)
        .no_binary_name(true)
        .disable_help_flag(true)
        .after_help(translate!("chmod-after-help"))
        .arg(
            Arg::new(options::HELP)
                .long(options::HELP)
                .help(translate!("chmod-help-print-help"))
                .action(ArgAction::Help)
        )
        .arg(
            Arg::new(options::CHANGES)
                .long(options::CHANGES)
                .short('c')
                .help(translate!("chmod-help-changes"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::QUIET)
                .long(options::QUIET)
                .visible_alias("silent")
                .short('f')
                .help(translate!("chmod-help-quiet"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::VERBOSE)
                .long(options::VERBOSE)
                .short('v')
                .help(translate!("chmod-help-verbose"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::NO_PRESERVE_ROOT)
                .long(options::NO_PRESERVE_ROOT)
                .help(translate!("chmod-help-no-preserve-root"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::PRESERVE_ROOT)
                .long(options::PRESERVE_ROOT)
                .help(translate!("chmod-help-preserve-root"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::RECURSIVE)
                .long(options::RECURSIVE)
                .short('R')
                .help(translate!("chmod-help-recursive"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::REFERENCE)
                .long("reference")
                .value_hint(clap::ValueHint::FilePath)
                .value_parser(clap::value_parser!(OsString))
                .help(translate!("chmod-help-reference"))
        )
        .arg(
            Arg::new(options::MODE).required_unless_present(options::REFERENCE),
        )
        .arg(
            Arg::new(options::FILE)
                .required_unless_present(options::MODE)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath)
                .value_parser(clap::value_parser!(OsString))
        )
        .args(sgcore::perms::common_args())
}

struct Chmoder {
    changes: bool,
    quiet: bool,
    verbose: bool,
    preserve_root: bool,
    recursive: bool,
    fmode: Option<u32>,
    cmode: Option<String>,
    traverse_symlinks: TraverseSymlinks,
    dereference: bool,
}

impl Chmoder {
    /// Calculate the new mode based on the current mode and the chmod specification.
    /// Returns (`new_mode`, `naively_expected_new_mode`) for symbolic modes, or (`new_mode`, `new_mode`) for numeric/reference modes.
    fn calculate_new_mode(&self, current_mode: u32, is_dir: bool) -> SGResult<(u32, u32)> {
        match self.fmode {
            Some(mode) => Ok((mode, mode)),
            None => {
                let cmode_unwrapped = self.cmode.clone().unwrap();
                let mut new_mode = current_mode;
                let mut naively_expected_new_mode = current_mode;

                for mode in cmode_unwrapped.split(',') {
                    let result = if mode.chars().any(|c| c.is_ascii_digit()) {
                        mode::parse_numeric(new_mode, mode, is_dir).map(|v| (v, v))
                    } else {
                        mode::parse_symbolic(new_mode, mode, mode::get_umask(), is_dir).map(|m| {
                            let naive_mode =
                                mode::parse_symbolic(naively_expected_new_mode, mode, 0, is_dir)
                                    .unwrap();
                            (m, naive_mode)
                        })
                    };

                    match result {
                        Ok((mode, naive_mode)) => {
                            new_mode = mode;
                            naively_expected_new_mode = naive_mode;
                        }
                        Err(f) => {
                            return if self.quiet {
                                Err(ExitCode::new(1))
                            } else {
                                Err(SGSimpleError::new(1, f))
                            };
                        }
                    }
                }
                Ok((new_mode, naively_expected_new_mode))
            }
        }
    }

    /// Report permission changes based on verbose and changes flags
    fn report_permission_change(&self, file_path: &Path, old_mode: u32, new_mode: u32) {
        if self.verbose || self.changes {
            let current_permissions = display_permissions_unix(old_mode as mode_t, false);
            let new_permissions = display_permissions_unix(new_mode as mode_t, false);

            if new_mode != old_mode {
                println!(
                    "mode of {} changed from {:04o} ({}) to {:04o} ({})",
                    file_path.quote(),
                    old_mode,
                    current_permissions,
                    new_mode,
                    new_permissions
                );
            } else if self.verbose {
                println!(
                    "mode of {} retained as {:04o} ({})",
                    file_path.quote(),
                    old_mode,
                    current_permissions
                );
            }
        }
    }

    /// Handle symlinks during directory traversal based on traversal mode
    #[cfg(not(target_os = "linux"))]
    fn handle_symlink_during_traversal(
        &self,
        path: &Path,
        is_command_line_arg: bool
    ) -> SGResult<()> {
        let should_follow_symlink = match self.traverse_symlinks {
            TraverseSymlinks::All => true,
            TraverseSymlinks::First => is_command_line_arg,
            TraverseSymlinks::None => false,
        };

        if !should_follow_symlink {
            return self.chmod_file_internal(path, false);
        }

        match fs::metadata(path) {
            Ok(meta) if meta.is_dir() => self.walk_dir_with_context(path, false),
            Ok(_) => {
                self.chmod_file(path)
            }
            Err(_) => {
                self.chmod_file_internal(path, false)
            }
        }
    }

    fn chmod(&self, files: &[OsString]) -> SGResult<()> {
        let mut r = Ok(());

        for filename in files {
            let file = Path::new(filename);
            if !file.exists() {
                if file.is_symlink() {
                    if !self.dereference && !self.recursive {
                        continue;
                    }
                    if self.recursive && self.traverse_symlinks == TraverseSymlinks::None {
                        continue;
                    }

                    if !self.quiet {
                        show!(ChmodError::DanglingSymlink(
                            filename.to_string_lossy().to_string()
                        ));
                        set_exit_code(1);
                    }

                    if self.verbose {
                        println!(
                            "{}",
                            translate!("chmod-verbose-failed-dangling", "file" => filename.to_string_lossy().quote())
                        );
                    }
                } else if !self.quiet {
                    show!(ChmodError::NoSuchFile(
                        filename.to_string_lossy().to_string()
                    ));
                }
                set_exit_code(1);
                continue;
            } else if !self.dereference && file.is_symlink() {
                continue;
            }
            if self.recursive && self.preserve_root && file == Path::new("/") {
                return Err(ChmodError::PreserveRoot("/".to_string()).into());
            }
            if self.recursive {
                r = self.walk_dir_with_context(file, true);
            } else {
                r = self.chmod_file(file).and(r);
            }
        }
        r
    }

    #[cfg(not(target_os = "linux"))]
    fn walk_dir_with_context(&self, file_path: &Path, is_command_line_arg: bool) -> SGResult<()> {
        let mut r = self.chmod_file(file_path);

        let should_follow_symlink = match self.traverse_symlinks {
            TraverseSymlinks::All => true,
            TraverseSymlinks::First => is_command_line_arg,
            TraverseSymlinks::None => false,
        };

        if (!file_path.is_symlink() || should_follow_symlink) && file_path.is_dir() {
            for dir_entry in file_path.read_dir()? {
                let path = match dir_entry {
                    Ok(entry) => entry.path(),
                    Err(err) => {
                        r = r.and(Err(err.into()));
                        continue;
                    }
                };
                if path.is_symlink() {
                    r = self.handle_symlink_during_recursion(&path).and(r);
                } else {
                    r = self.walk_dir_with_context(path.as_path(), false).and(r);
                }
            }
        }
        r
    }

    #[cfg(target_os = "linux")]
    fn walk_dir_with_context(&self, file_path: &Path, is_command_line_arg: bool) -> SGResult<()> {
        let mut r = self.chmod_file(file_path);

        let should_follow_symlink = match self.traverse_symlinks {
            TraverseSymlinks::All => true,
            TraverseSymlinks::First => is_command_line_arg,
            TraverseSymlinks::None => false,
        };

        if (!file_path.is_symlink() || should_follow_symlink) && file_path.is_dir() {
            match DirFd::open(file_path) {
                Ok(dir_fd) => {
                    r = self.safe_traverse_dir(&dir_fd, file_path).and(r);
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::PermissionDenied {
                        r = r.and(Err(ChmodError::PermissionDenied(
                            file_path.to_string_lossy().to_string()
                        )
                        .into()));
                    } else {
                        r = r.and(Err(err.into()));
                    }
                }
            }
        }
        r
    }

    #[cfg(target_os = "linux")]
    fn safe_traverse_dir(&self, dir_fd: &DirFd, dir_path: &Path) -> SGResult<()> {
        let mut r = Ok(());

        let entries = dir_fd.read_dir()?;

        let should_follow_symlink = self.traverse_symlinks == TraverseSymlinks::All;

        for entry_name in entries {
            let entry_path = dir_path.join(&entry_name);

            let dir_meta = dir_fd.metadata_at(&entry_name, should_follow_symlink);
            let Ok(meta) = dir_meta else {
                let e = dir_meta.unwrap_err();
                let error = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    ChmodError::PermissionDenied(entry_path.to_string_lossy().to_string()).into()
                } else {
                    e.into()
                };
                r = r.and(Err(error));
                continue;
            };

            if entry_path.is_symlink() {
                r = self
                    .handle_symlink_during_safe_recursion(&entry_path, dir_fd, &entry_name)
                    .and(r);
            } else {
                r = self
                    .safe_chmod_file(&entry_path, dir_fd, &entry_name, meta.mode() & 0o7777)
                    .and(r);

                if meta.is_dir() {
                    r = self.walk_dir_with_context(&entry_path, false).and(r);
                }
            }
        }
        r
    }

    #[cfg(target_os = "linux")]
    fn handle_symlink_during_safe_recursion(
        &self,
        path: &Path,
        dir_fd: &DirFd,
        entry_name: &std::ffi::OsStr
    ) -> SGResult<()> {
        match self.traverse_symlinks {
            TraverseSymlinks::All => {
                match fs::metadata(path) {
                    Ok(meta) if meta.is_dir() => self.walk_dir_with_context(path, false),
                    Ok(meta) => {
                        self.safe_chmod_file(path, dir_fd, entry_name, meta.mode() & 0o7777)
                    }
                    Err(_) => {
                        self.chmod_file_internal(path, false)
                    }
                }
            }
            TraverseSymlinks::First | TraverseSymlinks::None => {
                self.chmod_file_internal(path, false)
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn safe_chmod_file(
        &self,
        file_path: &Path,
        dir_fd: &DirFd,
        entry_name: &std::ffi::OsStr,
        current_mode: u32
    ) -> SGResult<()> {
        let (new_mode, _) = self.calculate_new_mode(current_mode, file_path.is_dir())?;

        let follow_symlinks = self.dereference;
        if let Err(_e) = dir_fd.chmod_at(entry_name, new_mode, follow_symlinks) {
            if self.verbose {
                println!(
                    "failed to change mode of {} to {:o}",
                    file_path.quote(),
                    new_mode
                );
            }
            return Err(
                ChmodError::PermissionDenied(file_path.to_string_lossy().to_string()).into()
            );
        }

        self.report_permission_change(file_path, current_mode, new_mode);

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn handle_symlink_during_recursion(&self, path: &Path) -> SGResult<()> {
        self.handle_symlink_during_traversal(path, false)
    }

    fn chmod_file(&self, file: &Path) -> SGResult<()> {
        self.chmod_file_internal(file, self.dereference)
    }

    fn chmod_file_internal(&self, file: &Path, dereference: bool) -> SGResult<()> {
        use sgcore::perms::get_metadata;

        let metadata = get_metadata(file, dereference);

        let fperm = match metadata {
            Ok(meta) => meta.mode() & 0o7777,
            Err(err) => {
                return if file.is_symlink() && !dereference {
                    if self.verbose {
                        println!(
                            "neither symbolic link {} nor referent has been changed",
                            file.quote()
                        );
                    }
                    Ok(())
                } else if err.kind() == std::io::ErrorKind::PermissionDenied {
                    Err(ChmodError::PermissionDenied(file.to_string_lossy().to_string()).into())
                } else {
                    Err(ChmodError::CannotStat(file.to_string_lossy().to_string()).into())
                };
            }
        };

        let (new_mode, naively_expected_new_mode) =
            self.calculate_new_mode(fperm, file.is_dir())?;

        match self.fmode {
            Some(mode) => self.change_file(fperm, mode, file)?,
            None => {
                if file.is_symlink() && !dereference {
                    if self.verbose {
                        println!(
                            "neither symbolic link {} nor referent has been changed",
                            file.quote()
                        );
                    }
                } else {
                    self.change_file(fperm, new_mode, file)?;
                }
                if (new_mode & !naively_expected_new_mode) != 0 {
                    return Err(ChmodError::NewPermissions(
                        file.to_string_lossy().to_string(),
                        display_permissions_unix(new_mode as mode_t, false),
                        display_permissions_unix(naively_expected_new_mode as mode_t, false)
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    fn change_file(&self, fperm: u32, mode: u32, file: &Path) -> Result<(), i32> {
        if fperm == mode {
            self.report_permission_change(file, fperm, mode);
            Ok(())
        } else if let Err(err) = fs::set_permissions(file, fs::Permissions::from_mode(mode)) {
            if !self.quiet {
                show_error!("{err}");
            }
            if self.verbose {
                println!(
                    "failed to change mode of file {} from {fperm:04o} ({}) to {mode:04o} ({})",
                    file.quote(),
                    display_permissions_unix(fperm as mode_t, false),
                    display_permissions_unix(mode as mode_t, false)
                );
            }
            Err(1)
        } else {
            self.report_permission_change(file, fperm, mode);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_negative_modes() {
        let (c, a) = extract_negative_modes(["-w", "-r", "file"].iter().map(OsString::from));
        assert_eq!(c, Some("-w,-r".to_string()));
        assert_eq!(a, ["w", "file"]);

        let (c, a) = extract_negative_modes(["-w", "file", "-r"].iter().map(OsString::from));
        assert_eq!(c, Some("-w,-r".to_string()));
        assert_eq!(a, ["w", "file"]);

        let (c, a) = extract_negative_modes(["-w", "--", "-r", "f"].iter().map(OsString::from));
        assert_eq!(c, Some("-w".to_string()));
        assert_eq!(a, ["w", "--", "-r", "f"]);

        let (c, a) = extract_negative_modes(["--", "-r", "file"].iter().map(OsString::from));
        assert_eq!(c, None);
        assert_eq!(a, ["--", "-r", "file"]);
    }
}

