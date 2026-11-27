use crate::screenshots;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime},
};

type Stamp = (u64, Option<SystemTime>);

pub struct ScreenshotWatch {
    watcher: Option<RecommendedWatcher>,
    events: Receiver<notify::Result<Event>>,
    paths: HashSet<PathBuf>,
    pending: HashMap<PathBuf, (Stamp, Instant)>,
    refresh_at: Instant,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = fs::metadata(path).ok()?;
    (metadata.is_file() && metadata.len() > 0).then(|| (metadata.len(), metadata.modified().ok()))
}

impl ScreenshotWatch {
    pub fn new() -> Self {
        let (sender, events) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(|error| eprintln!("cannot watch screenshot folders: {error}"))
        .ok();
        let mut watch = Self {
            watcher,
            events,
            paths: HashSet::new(),
            pending: HashMap::new(),
            refresh_at: Instant::now(),
        };
        watch.refresh(Instant::now());
        watch
    }

    fn refresh(&mut self, now: Instant) {
        self.refresh_at = now + Duration::from_secs(5);
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        let folders: HashSet<_> = screenshots::known_dirs().into_iter().collect();
        for removed in self.paths.difference(&folders) {
            let _ = watcher.unwatch(removed);
        }
        self.paths.retain(|path| folders.contains(path));
        for added in &folders {
            if !self.paths.contains(added)
                && watcher.watch(added, RecursiveMode::NonRecursive).is_ok()
            {
                self.paths.insert(added.to_owned());
            }
        }
    }

    pub fn poll(&mut self, root: &Path, enabled: bool) -> Vec<PathBuf> {
        let now = Instant::now();
        if now >= self.refresh_at {
            self.refresh(now);
        }
        for event in self.events.try_iter().take(256) {
            let event = match event {
                Ok(event) => event,
                Err(error) => {
                    eprintln!("screenshot watcher: {error}");
                    continue;
                }
            };
            if !enabled || !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                continue;
            }
            for path in event.paths {
                if screenshots::inside(root, &path) || !screenshots::screenshot(&path) {
                    continue;
                }
                if let Some(info) = stamp(&path) {
                    self.pending
                        .insert(path, (info, now + Duration::from_millis(180)));
                }
            }
        }
        if !enabled {
            self.pending.clear();
            return Vec::new();
        }

        let mut ready = Vec::new();
        self.pending.retain(|path, (info, deadline)| {
            if now < *deadline {
                return true;
            }
            let Some(current) = stamp(path) else {
                return false;
            };
            if current != *info {
                *info = current;
                *deadline = now + Duration::from_millis(180);
                return true;
            }
            if fs::File::open(path).is_ok() {
                ready.push(path.clone());
                return false;
            }
            now < *deadline + Duration::from_secs(2)
        });
        ready
    }
}
