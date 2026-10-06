use ratatui::buffer::Buffer;
use ratatui::widgets::Widget;
use ratatui_image::picker::{Picker, ProtocolType};

use super::*;

#[test]
#[expect(
    deprecated,
    reason = "the comparison uses the original library encoder"
)]
fn native_encoding_matches_the_original_fit_and_padding() {
    if std::env::var("TERM").is_ok_and(|term| term.starts_with("tmux"))
        || std::env::var("TERM_PROGRAM").is_ok_and(|program| program == "tmux")
    {
        crate::cli::terminal_query::test_without_tmux(
            "ui::image_diff::encoding_tests::native_encoding_matches_the_original_fit_and_padding",
        )
        .unwrap();
        return;
    }
    for font_size in [(10, 20), (9, 18)] {
        let mut original = Picker::from_fontsize(font_size);
        for (width, height, size) in [
            (2, 2, Rect::new(0, 0, 5, 2)),
            (9, 18, Rect::new(0, 0, 5, 2)),
            (127, 65, Rect::new(0, 0, 3, 2)),
            (17, 259, Rect::new(0, 0, 5, 2)),
            (129, 3, Rect::new(0, 0, 1, 1)),
        ] {
            let image = RgbaImage::from_fn(width, height, |x, y| {
                Rgba([
                    u8::try_from(x % 256).unwrap(),
                    u8::try_from(y % 256).unwrap(),
                    128,
                    u8::try_from((x + y) % 256).unwrap(),
                ])
            });
            for (protocol, original_protocol) in [
                (ImageProtocol::Sixel, ProtocolType::Sixel),
                (ImageProtocol::Iterm2, ProtocolType::Iterm2),
            ] {
                original.set_protocol_type(original_protocol);
                let expected = original
                    .new_protocol(
                        DynamicImage::ImageRgba8(image.clone()),
                        size,
                        Resize::Fit(None),
                    )
                    .unwrap();
                let actual = encode_native(
                    DynamicImage::ImageRgba8(image.clone()),
                    protocol,
                    size,
                    ImagePicker {
                        font_size,
                        is_tmux: false,
                    },
                )
                .unwrap();
                assert_eq!(actual.area(), expected.area());
                let mut expected_buffer = Buffer::empty(size);
                let mut actual_buffer = Buffer::empty(size);
                Image::new(&expected).render(size, &mut expected_buffer);
                Image::new(&actual).render(size, &mut actual_buffer);
                assert_eq!(
                    actual_buffer, expected_buffer,
                    "{font_size:?}, {width}x{height}, {protocol:?}"
                );
            }
        }
    }
}

#[test]
fn all_native_protocols_retain_tmux_passthrough_encoding() {
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Sixel,
        ImageProtocol::Iterm2,
    ] {
        let size = Rect::new(0, 0, 2, 2);
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 40, Rgba([80, 120, 160, 255])));
        let encoded = encode_native(
            image,
            protocol,
            size,
            ImagePicker {
                font_size: (10, 20),
                is_tmux: true,
            },
        )
        .unwrap();
        let mut buffer = Buffer::empty(size);
        Image::new(&encoded).render(size, &mut buffer);
        assert!(
            buffer
                .content()
                .iter()
                .any(|cell| cell.symbol().contains("\x1bPtmux;")),
            "{protocol:?}"
        );
    }
}
