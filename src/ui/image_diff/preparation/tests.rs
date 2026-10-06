use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::git::diff::ImageSide;
use crate::theme::{Appearance, Theme, ThemeName};
use crate::ui::image_diff::tests::{image_line, preview};
use crate::ui::image_diff::{ImageDrawState, PreparationFrame, Rect, draw_image_line};

struct ControlledEncoder {
    requests: Receiver<Vec<PreparationRequest>>,
    completed: Sender<PreparationResult>,
}

impl ControlledEncoder {
    fn install() -> Self {
        let (requests, queued) = bounded(1);
        let (completed, results) = bounded(MAX_IMAGES);
        IMAGE_PREPARATION.with(|preparation| {
            *preparation.borrow_mut() = ImagePreparation {
                encoded: Vec::new(),
                wanted: Vec::new(),
                pending: Vec::new(),
                encoder: Some(Encoder {
                    requests,
                    queued: queued.clone(),
                    completed: results,
                }),
                unavailable: false,
            };
        });
        Self {
            requests: queued,
            completed,
        }
    }

    fn finish(&self, request: PreparationRequest) {
        let display = prepare_native(&request.key);
        self.completed
            .send(PreparationResult { request, display })
            .expect("test completion");
    }
}

#[test]
fn pending_images_render_halfblocks_before_native_completion_and_request_repaint() {
    for (protocol, marker) in [
        (ImageProtocol::Kitty, "\x1b_G"),
        (ImageProtocol::Iterm2, "]1337;File=inline=1"),
        (ImageProtocol::Sixel, "\x1bP"),
    ] {
        let encoder = ControlledEncoder::install();
        let mut terminal = Terminal::new(TestBackend::new(32, 4)).expect("test terminal");
        let image = preview(ImageSide::New);
        let mut last_row = image.clone();
        last_row.row = 1;
        let rows = [image_line(image), image_line(last_row)];
        let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);
        let render = |frame: &mut ratatui::Frame<'_>| {
            let _images = PreparationFrame::begin();
            let mut state = ImageDrawState::new(true);
            for (row, line) in rows.iter().enumerate() {
                let row = u16::try_from(row).expect("test row");
                draw_image_line(
                    frame,
                    Rect::new(0, row, 32, 1),
                    line,
                    4 - row,
                    &theme,
                    protocol,
                    &mut state,
                );
            }
        };
        let _frame = terminal.draw(render).expect("pending frame");
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((0, 1))
                .expect("fallback row")
                .symbol(),
            "▀"
        );
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .all(|cell| !cell.symbol().contains('\x1b')),
            "pending {protocol:?} must render without native payloads"
        );
        assert!(
            !image_preparation_ready(),
            "pending image has no completion"
        );
        let requests = encoder.requests.try_recv().expect("pending requests");
        assert_eq!(requests.len(), 1, "one request per raster and size");
        for request in requests {
            encoder.finish(request);
        }
        assert!(
            image_preparation_ready(),
            "native completion requests repaint"
        );
        assert!(
            !image_preparation_ready(),
            "completion requests one repaint"
        );
        let _frame = terminal.draw(render).expect("native frame");
        assert!(
            terminal
                .backend()
                .buffer()
                .cell((0, 0))
                .expect("native row")
                .symbol()
                .contains(marker),
            "completed {protocol:?} renders its native payload"
        );
        assert!(
            encoder.requests.is_empty(),
            "cached frames do not enqueue another encoding"
        );
    }
}

#[test]
fn resize_raster_and_protocol_churn_coalesces_and_rejects_old_generation() {
    let encoder = ControlledEncoder::install();
    let first = key(5, ImageProtocol::Kitty);
    request_frame(std::slice::from_ref(&first));
    let original = encoder
        .requests
        .try_recv()
        .expect("first request")
        .remove(0);
    let replacement = key(4, ImageProtocol::Iterm2);
    let alternate = key(3, ImageProtocol::Sixel);
    for pass in 0..1_000_u16 {
        let protocol = match pass % 3 {
            0 => ImageProtocol::Kitty,
            1 => ImageProtocol::Iterm2,
            _ => ImageProtocol::Sixel,
        };
        let mut next = replacement.clone();
        next.raster = Arc::clone(if pass % 2 == 0 {
            &replacement.raster
        } else {
            &alternate.raster
        });
        next.width = pass % 10 + 1;
        next.height = pass % 5 + 1;
        next.protocol = protocol;
        request_frame(&[next]);
        IMAGE_PREPARATION.with(|preparation| {
            let preparation = preparation.borrow();
            assert_eq!(
                preparation.pending.len(),
                1,
                "only the current dimensions remain pending"
            );
            assert_eq!(
                preparation.wanted.len(),
                1,
                "only the current raster remains desired"
            );
            assert!(
                preparation.encoded.len() <= MAX_IMAGES,
                "encoded cache is bounded"
            );
            assert_eq!(
                preparation.encoder.as_ref().expect("encoder").queued.len(),
                1,
                "one coalesced batch is queued"
            );
        });
    }
    request_frame(std::slice::from_ref(&first));
    encoder.finish(original);
    assert!(
        !image_preparation_ready(),
        "an obsolete ticket cannot fill an identical newer request"
    );
    IMAGE_PREPARATION.with(|preparation| {
        assert!(
            preparation.borrow().encoded.is_empty(),
            "stale results do not enter the cache"
        );
    });
    let current = encoder.requests.try_recv().expect("current request");
    for request in current {
        encoder.finish(request);
    }
    assert!(
        image_preparation_ready(),
        "the current generation becomes ready"
    );
    IMAGE_PREPARATION.with(|preparation| {
        let preparation = preparation.borrow();
        assert_eq!(
            preparation.encoded.len(),
            1,
            "only the current generation is cached"
        );
        assert!(
            preparation
                .encoded
                .first()
                .expect("current image")
                .key
                .matches(&first),
            "raster identity, protocol, and dimensions match"
        );
    });
}

#[test]
fn images_leaving_the_frame_cancel_pending_completions() {
    let encoder = ControlledEncoder::install();
    request_frame(&[key(5, ImageProtocol::Kitty)]);
    let request = encoder
        .requests
        .try_recv()
        .expect("pending request")
        .remove(0);
    request_frame(&[]);
    assert!(
        !request.ticket.valid(),
        "leaving the viewport cancels the ticket"
    );
    encoder.finish(request);
    assert!(
        !image_preparation_ready(),
        "offscreen completion does not request a repaint"
    );
    IMAGE_PREPARATION.with(|preparation| {
        let preparation = preparation.borrow();
        assert!(
            preparation.pending.is_empty(),
            "offscreen requests are removed"
        );
        assert!(
            preparation.encoded.is_empty(),
            "offscreen results are rejected"
        );
    });
}

#[test]
fn failed_preparation_keeps_fallback_without_repeating_work() {
    let encoder = ControlledEncoder::install();
    let invalid = ImageKey {
        raster: Arc::new(ImageRaster {
            width: 2,
            height: 2,
            rgba: vec![255; 3],
        }),
        protocol: ImageProtocol::Kitty,
        width: 5,
        height: 2,
    };
    request_frame(std::slice::from_ref(&invalid));
    let request = encoder
        .requests
        .try_recv()
        .expect("invalid request")
        .remove(0);
    encoder.finish(request);
    assert!(
        !image_preparation_ready(),
        "failed encoding does not change the fallback"
    );
    request_frame(std::slice::from_ref(&invalid));
    assert!(
        encoder.requests.is_empty(),
        "failed identities are not encoded repeatedly"
    );
    IMAGE_PREPARATION.with(|preparation| {
        let preparation = preparation.borrow();
        assert!(preparation.pending.is_empty(), "failed work is removed");
        assert!(
            preparation
                .encoded
                .first()
                .expect("failed cache entry")
                .display
                .is_none(),
            "failed encoding remains a halfblock fallback"
        );
    });
}

#[test]
fn desired_jobs_and_encoded_cache_stay_within_four_images() {
    let encoder = ControlledEncoder::install();
    for _ in 0..3 {
        let images = (1..=6)
            .map(|width| key(width, ImageProtocol::Kitty))
            .collect::<Vec<_>>();
        request_frame(&images);
        let requests = encoder.requests.try_recv().expect("bounded requests");
        assert_eq!(requests.len(), MAX_IMAGES, "visible preparation is capped");
        for request in requests {
            encoder.finish(request);
        }
        assert!(image_preparation_ready(), "current images complete");
        IMAGE_PREPARATION.with(|preparation| {
            let preparation = preparation.borrow();
            assert_eq!(
                preparation.encoded.len(),
                MAX_IMAGES,
                "cache eviction preserves its bound"
            );
            assert_eq!(
                preparation.wanted.len(),
                MAX_IMAGES,
                "desired identities remain bounded"
            );
            assert!(preparation.pending.is_empty(), "completed work is removed");
        });
    }
}

#[test]
fn a_single_encoder_keeps_only_the_latest_batch_while_a_job_is_in_flight() {
    let (started, starts) = bounded(1);
    let (release, releases) = bounded(1);
    let encoder = Encoder::start(move |key| {
        started
            .send((key.clone(), thread::current().id()))
            .expect("encoding started");
        releases
            .recv_timeout(Duration::from_secs(10))
            .expect("encoding released");
        prepare_native(key)
    })
    .expect("test encoder thread");
    IMAGE_PREPARATION.with(|preparation| {
        *preparation.borrow_mut() = ImagePreparation {
            encoded: Vec::new(),
            wanted: Vec::new(),
            pending: Vec::new(),
            encoder: Some(encoder),
            unavailable: false,
        };
    });
    let initial = key(5, ImageProtocol::Kitty);
    request_frame(&[initial]);
    let (_, encoder_thread) = starts
        .recv_timeout(Duration::from_secs(10))
        .expect("initial encoding");
    assert_ne!(
        encoder_thread,
        thread::current().id(),
        "encoding runs off the rendering thread"
    );
    let mut latest = key(3, ImageProtocol::Iterm2);
    for pass in 0..1_000_u16 {
        latest.width = pass % 10 + 1;
        latest.protocol = if pass % 2 == 0 {
            ImageProtocol::Kitty
        } else {
            ImageProtocol::Iterm2
        };
        request_frame(std::slice::from_ref(&latest));
    }
    release.send(()).expect("release stale encoding");
    let (started_key, next_thread) = starts
        .recv_timeout(Duration::from_secs(10))
        .expect("latest encoding");
    assert!(
        started_key.matches(&latest),
        "the encoder skips superseded batches"
    );
    assert_eq!(
        encoder_thread, next_thread,
        "one worker services every request"
    );
    release.send(()).expect("release current encoding");
    IMAGE_PREPARATION.with(|preparation| {
        let mut preparation = preparation.borrow_mut();
        let completed = preparation
            .encoder
            .as_ref()
            .expect("encoder")
            .completed
            .recv_timeout(Duration::from_secs(10))
            .expect("current completion");
        assert!(
            preparation.accept(completed),
            "only the current image is published"
        );
        assert_eq!(
            preparation.encoded.len(),
            1,
            "the obsolete in-flight image was dropped"
        );
        assert!(
            preparation
                .encoded
                .first()
                .expect("current image")
                .key
                .matches(&latest),
            "the current identity is cached"
        );
        assert!(
            preparation
                .encoder
                .as_ref()
                .expect("encoder")
                .completed
                .is_empty(),
            "stale encodings do not publish results"
        );
        *preparation = ImagePreparation::default();
    });
}

fn key(width: u16, protocol: ImageProtocol) -> ImageKey {
    ImageKey {
        raster: Arc::new(ImageRaster {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        }),
        protocol,
        width,
        height: 2,
    }
}

fn request_frame(images: &[ImageKey]) {
    let _frame = PreparationFrame::begin();
    IMAGE_PREPARATION.with(|preparation| {
        let mut preparation = preparation.borrow_mut();
        for image in images {
            let _display = preparation.display(image.clone());
        }
    });
}
