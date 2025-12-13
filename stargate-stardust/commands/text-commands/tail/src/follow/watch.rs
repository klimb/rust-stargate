

use crate::args::{FollowMode, Settings};
use crate::follow::files::{FileHandling, PathData};
use crate::paths::{Input, InputKind, MetadataExtTail, PathExtTail};
use crate::{platform, text};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, WatcherKind};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, channel};
use sgcore::display::Quotable;
use sgcore::error::{SGResult, SGSimpleError, set_exit_code};
use sgcore::translate;

use sgcore::show_error;

pub struct WatcherRx {
    watcher: Box<dyn Watcher>,
    receiver: Receiver<Result<notify::Event, notify::Error>>,
}

impl WatcherRx {
    fn new(
        watcher: Box<dyn Watcher>,
        receiver: Receiver<Result<notify::Event, notify::Error>>
    ) -> Self {
        Self { watcher, receiver }
    }

    /// Wrapper for `notify::Watcher::watch` to also add the parent directory of `path` if necessary.
    fn watch_with_parent(&mut self, path: &Path) -> SGResult<()> {
        let mut path = path.to_owned();
        #[cfg(target_os = "linux")]
        if path.is_file() {
            if let Some(parent) = path.parent() {
                #[cfg_attr(not(target_os = "openbsd"), allow(clippy::assigning_clones))]
                if parent.is_dir() {
                    path = parent.to_owned();
                } else {
                    path = PathBuf::from(".");
                }
            } else {
                return Err(SGSimpleError::new(
                    1,
                    translate!("tail-error-cannot-watch-parent-directory", "path" => path.display())
                ));
            }
        }
        if path.is_relative() {
            path = path.canonicalize()?;
        }

        self.watch(&path, RecursiveMode::NonRecursive)?;
        Ok(())
    }

    fn watch(&mut self, path: &Path, mode: RecursiveMode) -> SGResult<()> {
        self.watcher
            .watch(path, mode)
            .map_err(|err| SGSimpleError::new(1, err.to_string()))
    }

    fn unwatch(&mut self, path: &Path) -> SGResult<()> {
        self.watcher
            .unwatch(path)
            .map_err(|err| SGSimpleError::new(1, err.to_string()))
    }
}

pub struct Observer {
    /// Whether --retry was given on the command line
    pub retry: bool,

    /// The [`FollowMode`]
    pub follow: Option<FollowMode>,

    /// Indicates whether to use the fallback `polling` method instead of the
    /// platform specific event driven method. Since `use_polling` is subject to
    /// change during runtime it is moved out of [`Settings`].
    pub use_polling: bool,

    pub watcher_rx: Option<WatcherRx>,
    pub orphans: Vec<PathBuf>,
    pub files: FileHandling,

    pub pid: platform::Pid,
}

impl Observer {
    pub fn new(
        retry: bool,
        follow: Option<FollowMode>,
        use_polling: bool,
        files: FileHandling,
        pid: platform::Pid
    ) -> Self {
        let pid = if platform::supports_pid_checks(pid) {
            pid
        } else {
            0
        };

        Self {
            retry,
            follow,
            use_polling,
            watcher_rx: None,
            orphans: Vec::new(),
            files,
            pid,
        }
    }

    pub fn from(settings: &Settings) -> Self {
        Self::new(
            settings.retry,
            settings.follow,
            settings.use_polling,
            FileHandling::from(settings),
            settings.pid
        )
    }

    pub fn add_path(
        &mut self,
        path: &Path,
        display_name: &str,
        reader: Option<Box<dyn BufRead>>,
        update_last: bool
    ) -> SGResult<()> {
        if self.follow.is_some() {
            let path = if path.is_relative() {
                std::env::current_dir()?.join(path)
            } else {
                path.to_owned()
            };
            let metadata = path.metadata().ok();
            self.files.insert(
                &path,
                PathData::new(reader, metadata, display_name),
                update_last
            );
        }

        Ok(())
    }

    pub fn add_stdin(
        &mut self,
        display_name: &str,
        reader: Option<Box<dyn BufRead>>,
        update_last: bool
    ) -> SGResult<()> {
        if self.follow == Some(FollowMode::Descriptor) {
            return self.add_path(
                &PathBuf::from(text::DEV_STDIN),
                display_name,
                reader,
                update_last
            );
        }

        Ok(())
    }

    pub fn add_bad_path(
        &mut self,
        path: &Path,
        display_name: &str,
        update_last: bool
    ) -> SGResult<()> {
        if self.retry && self.follow.is_some() {
            return self.add_path(path, display_name, None, update_last);
        }

        Ok(())
    }

    pub fn start(&mut self, settings: &Settings) -> SGResult<()> {
        if settings.follow.is_none() {
            return Ok(());
        }

        let (tx, rx) = channel();

        let watcher: Box<dyn Watcher>;
        let watcher_config = notify::Config::default()
            .with_poll_interval(settings.sleep_sec)
            .with_compare_contents(true);
        if self.use_polling || RecommendedWatcher::kind() == WatcherKind::PollWatcher {
            self.use_polling = true;
            watcher = Box::new(notify::PollWatcher::new(tx, watcher_config).unwrap());
        } else {
            let tx_clone = tx.clone();
            match RecommendedWatcher::new(tx, notify::Config::default()) {
                Ok(w) => watcher = Box::new(w),
                Err(e) if e.to_string().starts_with("Too many open files") => {
                    show_error!(
                        "{}",
                        translate!("tail-error-backend-cannot-be-used-too-many-files", "backend" => text::BACKEND)
                    );
                    set_exit_code(1);
                    self.use_polling = true;
                    watcher = Box::new(notify::PollWatcher::new(tx_clone, watcher_config).unwrap());
                }
                Err(e) => return Err(SGSimpleError::new(1, e.to_string())),
            }
        }

        self.watcher_rx = Some(WatcherRx::new(watcher, rx));
        self.init_files(&settings.inputs)?;

        Ok(())
    }

    pub fn follow_descriptor(&self) -> bool {
        self.follow == Some(FollowMode::Descriptor)
    }

    pub fn follow_name(&self) -> bool {
        self.follow == Some(FollowMode::Name)
    }

    pub fn follow_descriptor_retry(&self) -> bool {
        self.follow_descriptor() && self.retry
    }

    pub fn follow_name_retry(&self) -> bool {
        self.follow_name() && self.retry
    }

    fn init_files(&mut self, inputs: &Vec<Input>) -> SGResult<()> {
        if let Some(watcher_rx) = &mut self.watcher_rx {
            for input in inputs {
                match input.kind() {
                    InputKind::Stdin => (),
                    InputKind::File(path) => {
                        #[cfg(all(unix, not(target_os = "linux")))]
                        if !path.is_file() {
                            continue;
                        }
                        let mut path = path.clone();
                        if path.is_relative() {
                            path = std::env::current_dir()?.join(path);
                        }

                        if path.is_tailable() {
                            watcher_rx.watch_with_parent(&path)?;
                        } else if !path.is_orphan() {
                            watcher_rx
                                .watch(path.parent().unwrap(), RecursiveMode::NonRecursive)?;
                        } else {
                            self.orphans.push(path);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::cognitive_complexity)]
    fn handle_event(
        &mut self,
        event: &notify::Event,
        settings: &Settings
    ) -> SGResult<Vec<PathBuf>> {
        use notify::event::*;

        let event_path = event.paths.first().unwrap();
        let mut paths: Vec<PathBuf> = vec![];
        let display_name = self.files.get(event_path).display_name.clone();

        match event.kind {
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any | MetadataKind::WriteTime) | ModifyKind::Data(DataChange::Any) | ModifyKind::Name(RenameMode::To)) |
            EventKind::Create(CreateKind::File | CreateKind::Folder | CreateKind::Any) => {
                if let Ok(new_md) = event_path.metadata() {
                    let is_tailable = new_md.is_tailable();
                    let pd = self.files.get(event_path);
                    if let Some(old_md) = &pd.metadata {
                        if is_tailable {
                            if !old_md.is_tailable() {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-has-become-accessible", "file" => display_name.quote())
                                );
                                self.files.update_reader(event_path)?;
                            } else if pd.reader.is_none() {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-has-appeared-following-new-file", "file" => display_name.quote())
                                );
                                self.files.update_reader(event_path)?;
                            } else if event.kind == EventKind::Modify(ModifyKind::Name(RenameMode::To))
                            || (self.use_polling && !old_md.file_id_eq(&new_md)) {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-has-been-replaced-following-new-file", "file" => display_name.quote())
                                );
                                self.files.update_reader(event_path)?;
                            } else if old_md.got_truncated(&new_md)? {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-file-truncated", "file" => display_name)
                                );
                                self.files.update_reader(event_path)?;
                            }
                            paths.push(event_path.clone());
                        } else if !is_tailable && old_md.is_tailable() {
                            if pd.reader.is_some() {
                                self.files.reset_reader(event_path);
                            } else {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-replaced-with-untailable-file", "file" => display_name.quote())
                                );
                            }
                        }
                    } else if is_tailable {
                        show_error!(
                            "{}",
                            translate!("tail-status-has-appeared-following-new-file", "file" => display_name.quote())
                        );
                        self.files.update_reader(event_path)?;
                        paths.push(event_path.clone());
                    } else if settings.retry {
                        if self.follow_descriptor() {
                            show_error!(
                                "{}",
                                translate!("tail-status-replaced-with-untailable-file-giving-up", "file" => display_name.quote())
                            );
                            let _ = self.watcher_rx.as_mut().unwrap().watcher.unwatch(event_path);
                            self.files.remove(event_path);
                            if self.files.no_files_remaining(settings) {
                                return Err(SGSimpleError::new(1, translate!("tail-no-files-remaining")));
                            }
                        } else {
                            show_error!(
                                "{}",
                                translate!("tail-status-replaced-with-untailable-file", "file" => display_name.quote())
                            );
                        }
                    }
                    self.files.update_metadata(event_path, Some(new_md));
                }
            }
            EventKind::Remove(RemoveKind::File | RemoveKind::Any)

                | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                if self.follow_name() {
                    if settings.retry {
                        if let Some(old_md) = self.files.get_mut_metadata(event_path) {
                            if old_md.is_tailable() && self.files.get(event_path).reader.is_some() {
                                show_error!(
                                    "{}",
                                    translate!("tail-status-file-became-inaccessible", "file" => display_name.quote(), "become_inaccessible" => translate!("tail-become-inaccessible"), "no_such_file" => translate!("tail-no-such-file-or-directory"))
                                );
                            }
                        }
                        if event_path.is_orphan() && !self.orphans.contains(event_path) {
                            show_error!("{}", translate!("tail-status-directory-containing-watched-file-removed"));
                            show_error!(
                                "{}",
                                translate!("tail-status-backend-cannot-be-used-reverting-to-polling", "backend" => text::BACKEND)
                            );
                            self.orphans.push(event_path.clone());
                            let _ = self.watcher_rx.as_mut().unwrap().unwatch(event_path);
                        }
                    } else {
                        show_error!(
                            "{}",
                            translate!("tail-status-file-no-such-file", "file" => display_name, "no_such_file" => translate!("tail-no-such-file-or-directory"))
                        );
                        if !self.files.files_remaining() && self.use_polling {
                            return Err(SGSimpleError::new(1, translate!("tail-no-files-remaining")));
                        }
                    }
                    self.files.reset_reader(event_path);
                } else if self.follow_descriptor_retry() {
                    let _ = self.watcher_rx.as_mut().unwrap().unwatch(event_path);
                    self.files.remove(event_path);
                } else if self.use_polling && event.kind == EventKind::Remove(RemoveKind::Any) {
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {

                if self.follow_descriptor() {
                    let new_path = event.paths.last().unwrap();
                    paths.push(new_path.clone());

                    let new_data = PathData::from_other_with_path(self.files.remove(event_path), new_path);
                    self.files.insert(
                        new_path,
                        new_data,
                        self.files.get_last().unwrap() == event_path
                    );

                    let _ = self.watcher_rx.as_mut().unwrap().unwatch(event_path);
                    self.watcher_rx.as_mut().unwrap().watch_with_parent(new_path)?;
                }
            }
            _ => {}
        }
        Ok(paths)
    }
}

#[allow(clippy::cognitive_complexity)]
pub fn follow(mut observer: Observer, settings: &Settings) -> SGResult<()> {
    if observer.files.no_files_remaining(settings) && !observer.files.only_stdin_remaining() {
        return Err(SGSimpleError::new(1, translate!("tail-no-files-remaining")));
    }

    let mut process = platform::ProcessChecker::new(observer.pid);

    let mut timeout_counter = 0;

    loop {
        let mut _read_some = false;

        if settings.follow.is_some() && observer.pid != 0 && process.is_dead() {
            break;
        }

        if observer.follow_name_retry() {
            for new_path in &observer.orphans {
                if new_path.exists() {
                    let pd = observer.files.get(new_path);
                    let md = new_path.metadata().unwrap();
                    if md.is_tailable() && pd.reader.is_none() {
                        show_error!(
                            "{}",
                            translate!("tail-status-has-appeared-following-new-file", "file" => pd.display_name.quote())
                        );
                        observer.files.update_metadata(new_path, Some(md));
                        observer.files.update_reader(new_path)?;
                        _read_some = observer.files.tail_file(new_path, settings.verbose)?;
                        observer
                            .watcher_rx
                            .as_mut()
                            .unwrap()
                            .watch_with_parent(new_path)?;
                    }
                }
            }
        }

        let rx_result = observer
            .watcher_rx
            .as_mut()
            .unwrap()
            .receiver
            .recv_timeout(settings.sleep_sec);

        if rx_result.is_ok() {
            timeout_counter = 0;
        }

        let mut paths = vec![];
        match rx_result {
            Ok(Ok(event)) => {
                if let Some(event_path) = event.paths.first() {
                    if observer.files.contains_key(event_path) {
                        paths = observer.handle_event(&event, settings)?;
                    }
                }
            }
            Ok(Err(notify::Error {
                kind: notify::ErrorKind::Io(ref e),
                paths,
            })) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(event_path) = paths.first() {
                    if observer.files.contains_key(event_path) {
                        let _ = observer
                            .watcher_rx
                            .as_mut()
                            .unwrap()
                            .watcher
                            .unwatch(event_path);
                    }
                }
            }
            Ok(Err(notify::Error {
                kind: notify::ErrorKind::MaxFilesWatch,
                ..
            })) => {
                return Err(SGSimpleError::new(
                    1,
                    translate!("tail-error-backend-resources-exhausted", "backend" => text::BACKEND)
                ));
            }
            Ok(Err(e)) => {
                return Err(SGSimpleError::new(
                    1,
                    translate!("tail-error-notify-error", "error" => e)
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timeout_counter += 1;
            }
            Err(e) => {
                return Err(SGSimpleError::new(
                    1,
                    translate!("tail-error-recv-timeout-error", "error" => e)
                ));
            }
        }

        if observer.use_polling && settings.follow.is_some() {
            paths = observer.files.keys().cloned().collect::<Vec<_>>();
        }

        for path in &paths {
            _read_some = observer.files.tail_file(path, settings.verbose)?;
        }

        if timeout_counter == settings.max_unchanged_stats {
        }
    }

    Ok(())
}

