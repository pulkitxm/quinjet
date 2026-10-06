use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

use super::{ImageProtocol, ImageRaster, NativeDisplay, prepare_native};

const MAX_PENDING_IMAGES: usize = 4;
const MAX_OFFSCREEN_IMAGES: usize = 4;

thread_local! {
    pub(super) static IMAGE_PREPARATION: RefCell<ImagePreparation> = RefCell::new(ImagePreparation::default());
}

#[derive(Clone)]
pub(super) struct ImageKey {
    pub(super) raster: Arc<ImageRaster>,
    pub(super) protocol: ImageProtocol,
    pub(super) width: u16,
    pub(super) height: u16,
}

impl ImageKey {
    fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.raster, &other.raster)
            && self.protocol == other.protocol
            && self.width == other.width
            && self.height == other.height
    }
}

pub(super) struct EncodedImage {
    pub(super) key: ImageKey,
    pub(super) display: Option<NativeDisplay>,
}

#[derive(Default)]
pub(super) struct ImagePreparation {
    pub(super) encoded: Vec<EncodedImage>,
    wanted: Vec<ImageKey>,
    pending: Vec<PreparationRequest>,
    encoder: Option<Encoder>,
    unavailable: bool,
}

impl ImagePreparation {
    pub(super) fn display(&mut self, key: ImageKey) -> Option<&mut EncodedImage> {
        if !self.wanted.iter().any(|wanted| wanted.matches(&key)) {
            self.wanted.push(key.clone());
        }
        if let Some(position) = self
            .encoded
            .iter()
            .position(|entry| entry.key.matches(&key))
        {
            return self.encoded.get_mut(position);
        }
        if self.unavailable || self.pending.iter().any(|request| request.key.matches(&key)) {
            return None;
        }
        if self.pending.len() == MAX_PENDING_IMAGES {
            let position = self.pending.iter().position(|request| {
                !self
                    .wanted
                    .iter()
                    .any(|wanted| wanted.matches(&request.key))
            })?;
            let request = self.pending.remove(position);
            request.ticket.cancel();
        }
        self.pending.push(PreparationRequest {
            key,
            ticket: Arc::new(PreparationTicket::default()),
        });
        None
    }

    fn begin_frame(&mut self) {
        self.wanted.clear();
    }

    fn finish_frame(&mut self) {
        self.trim_cache();
        self.pending.retain(|request| {
            if self
                .wanted
                .iter()
                .any(|wanted| wanted.matches(&request.key))
            {
                true
            } else {
                request.ticket.cancel();
                false
            }
        });
        if !self
            .pending
            .iter()
            .any(|request| request.ticket.unclaimed())
        {
            return;
        }
        if self.encoder.is_none() {
            self.encoder = Encoder::start(prepare_native);
            self.unavailable = self.encoder.is_none();
        }
        if let Some(encoder) = &self.encoder {
            encoder.submit(self.pending.clone());
        } else {
            for request in self.pending.drain(..) {
                request.ticket.cancel();
            }
        }
    }

    fn drain(&mut self) -> bool {
        let mut ready = false;
        while let Some(completed) = self
            .encoder
            .as_ref()
            .and_then(|encoder| encoder.completed.try_recv().ok())
        {
            ready |= self.accept(completed);
        }
        ready
    }

    fn accept(&mut self, completed: PreparationResult) -> bool {
        let Some(position) = self.pending.iter().position(|request| {
            Arc::ptr_eq(&request.ticket, &completed.request.ticket)
                && request.key.matches(&completed.request.key)
        }) else {
            return false;
        };
        let request = self.pending.remove(position);
        request.ticket.cancel();
        if !self
            .wanted
            .iter()
            .any(|wanted| wanted.matches(&request.key))
        {
            return false;
        }
        self.encoded.push(EncodedImage {
            key: request.key,
            display: completed.display,
        });
        self.trim_cache();
        true
    }

    fn trim_cache(&mut self) {
        let mut remove = self
            .encoded
            .iter()
            .filter(|entry| !self.wanted.iter().any(|wanted| wanted.matches(&entry.key)))
            .count()
            .saturating_sub(MAX_OFFSCREEN_IMAGES);
        self.encoded.retain(|entry| {
            if remove > 0 && !self.wanted.iter().any(|wanted| wanted.matches(&entry.key)) {
                remove -= 1;
                false
            } else {
                true
            }
        });
    }
}

impl Drop for ImagePreparation {
    fn drop(&mut self) {
        for request in &self.pending {
            request.ticket.cancel();
        }
    }
}

pub(in crate::ui) struct PreparationFrame;

impl PreparationFrame {
    pub(in crate::ui) fn begin() -> Self {
        IMAGE_PREPARATION.with(|preparation| preparation.borrow_mut().begin_frame());
        Self
    }
}

impl Drop for PreparationFrame {
    fn drop(&mut self) {
        IMAGE_PREPARATION.with(|preparation| preparation.borrow_mut().finish_frame());
    }
}

pub(crate) fn image_preparation_ready() -> bool {
    IMAGE_PREPARATION.with(|preparation| preparation.borrow_mut().drain())
}

#[derive(Default)]
struct PreparationTicket {
    claimed: AtomicBool,
    cancelled: AtomicBool,
}

impl PreparationTicket {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn valid(&self) -> bool {
        !self.cancelled.load(Ordering::Acquire)
    }

    fn unclaimed(&self) -> bool {
        !self.claimed.load(Ordering::Acquire)
    }

    fn claim(&self) -> bool {
        self.valid()
            && self
                .claimed
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
    }
}

#[derive(Clone)]
struct PreparationRequest {
    key: ImageKey,
    ticket: Arc<PreparationTicket>,
}

struct PreparationResult {
    request: PreparationRequest,
    display: Option<NativeDisplay>,
}

struct Encoder {
    requests: Sender<Vec<PreparationRequest>>,
    queued: Receiver<Vec<PreparationRequest>>,
    completed: Receiver<PreparationResult>,
}

impl Encoder {
    fn start(
        prepare: impl FnMut(&ImageKey) -> Option<NativeDisplay> + Send + 'static,
    ) -> Option<Self> {
        let (requests, queued) = bounded(1);
        let (complete, completed) = bounded(MAX_PENDING_IMAGES);
        let work = queued.clone();
        let _thread = thread::Builder::new()
            .name("image-encoder".to_owned())
            .spawn(move || encode_images(&work, &complete, prepare))
            .ok()?;
        Some(Self {
            requests,
            queued,
            completed,
        })
    }

    fn submit(&self, requests: Vec<PreparationRequest>) {
        if let Err(TrySendError::Full(requests)) = self.requests.try_send(requests) {
            drop(self.queued.try_recv());
            drop(self.requests.try_send(requests));
        }
    }
}

fn encode_images(
    requests: &Receiver<Vec<PreparationRequest>>,
    completed: &Sender<PreparationResult>,
    mut prepare: impl FnMut(&ImageKey) -> Option<NativeDisplay>,
) {
    while let Ok(batch) = requests.recv() {
        for request in batch {
            if !request.ticket.claim() {
                continue;
            }
            let display = prepare(&request.key);
            if request.ticket.valid()
                && completed
                    .send(PreparationResult { request, display })
                    .is_err()
            {
                return;
            }
        }
    }
}

#[cfg(test)]
mod benchmark;
#[cfg(test)]
mod tests;
