use std::path::Path;

use crossbeam_channel::{Receiver, TryRecvError};

use crate::git::ProjectGroup;

pub(super) struct ProjectLoader {
    receiver: Option<Receiver<Vec<ProjectGroup>>>,
}

impl ProjectLoader {
    pub(super) fn start(path: &Path) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let root = path.to_path_buf();
        let _thread = std::thread::spawn(move || {
            drop(sender.send(crate::state::load_recent_projects(&root)));
        });
        Self {
            receiver: Some(receiver),
        }
    }

    pub(super) const fn ready() -> Self {
        Self { receiver: None }
    }

    pub(super) const fn is_loading(&self) -> bool {
        self.receiver.is_some()
    }

    pub(super) fn poll(&mut self) -> Option<Vec<ProjectGroup>> {
        match self.receiver.as_ref()?.try_recv() {
            Ok(groups) => {
                self.receiver = None;
                Some(groups)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                Some(Vec::new())
            }
        }
    }

    #[cfg(test)]
    pub(super) const fn waiting(receiver: Receiver<Vec<ProjectGroup>>) -> Self {
        Self {
            receiver: Some(receiver),
        }
    }
}
