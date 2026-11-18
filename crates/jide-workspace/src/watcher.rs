use crate::error::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::{Event, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

pub struct WorkspaceWatcher {
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    receiver: Receiver<WatchEvent>,
}

impl WorkspaceWatcher {
    pub fn new(paths: &[PathBuf]) -> Result<Self> {
        Self::new_with_wakeup(paths, || {})
    }

    pub fn new_with_wakeup<F>(paths: &[PathBuf], wakeup: F) -> Result<Self>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let (tx, rx) = unbounded();

        let mut debouncer = new_debouncer(
            Duration::from_millis(200),
            None,
            move |res: DebounceEventResult| {
                if let Ok(events) = res {
                    let mut sent_any = false;
                    for event in events {
                        Self::handle_event(&event.event, &tx);
                        sent_any = true;
                    }
                    if sent_any {
                        wakeup();
                    }
                }
            },
        )?;

        for path in paths {
            if path.exists() {
                debouncer.watch(path, RecursiveMode::Recursive)?;
            }
        }

        Ok(Self {
            _debouncer: debouncer,
            receiver: rx,
        })
    }

    fn handle_event(event: &Event, tx: &Sender<WatchEvent>) {
        use notify::EventKind;

        match event.kind {
            EventKind::Create(_) => {
                for path in &event.paths {
                    let _ = tx.send(WatchEvent::Created(path.clone()));
                }
            }
            EventKind::Modify(_) => {
                if event.paths.len() >= 2 {
                    let _ = tx.send(WatchEvent::Renamed {
                        from: event.paths[0].clone(),
                        to: event.paths[1].clone(),
                    });
                } else {
                    for path in &event.paths {
                        let _ = tx.send(WatchEvent::Modified(path.clone()));
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in &event.paths {
                    let _ = tx.send(WatchEvent::Removed(path.clone()));
                }
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn events(&self) -> &Receiver<WatchEvent> {
        &self.receiver
    }
}
