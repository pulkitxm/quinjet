use std::cell::Cell;

use super::*;
use crate::git::diff::detect_protocol_from_vars;

#[test]
fn native_inference_and_unknown_terminals_do_not_invoke_queries() {
    for (key, value, expected) in [
        ("TERM", "xterm-kitty", ProtocolType::Kitty),
        ("TERM_PROGRAM", "ghostty", ProtocolType::Kitty),
        ("TERM_PROGRAM", "iTerm.app", ProtocolType::Iterm2),
        ("TERM_PROGRAM", "WezTerm", ProtocolType::Iterm2),
        ("TERM", "xterm-sixel", ProtocolType::Sixel),
        ("TERM", "xterm-256color", ProtocolType::Halfblocks),
    ] {
        let inferred = detect_protocol_from_vars([(key, value)]);
        let called = Cell::new(false);
        let (picker, protocol) = choose_picker(inferred, "", None, || {
            called.set(true);
            Ok(None)
        })
        .unwrap();
        assert!(!called.get(), "{key}={value} must not negotiate");
        assert_eq!(picker.protocol_type(), expected);
        assert_eq!(protocol, inferred);
    }
}

#[test]
fn explicit_overrides_bypass_queries() {
    for (override_value, expected) in [
        ("kitty", ProtocolType::Kitty),
        ("ITERM2", ProtocolType::Iterm2),
        ("sixel", ProtocolType::Sixel),
        ("halfblocks", ProtocolType::Halfblocks),
        ("unknown", ProtocolType::Kitty),
    ] {
        let inferred = detect_protocol_from_vars([
            ("TERM", "xterm-kitty"),
            ("QUINJET_IMAGE_PROTOCOL", override_value),
        ]);
        let called = Cell::new(false);
        let (picker, _) = choose_picker(inferred, override_value, None, || {
            called.set(true);
            Ok(None)
        })
        .unwrap();
        assert!(!called.get(), "{override_value} must not negotiate");
        assert_eq!(picker.protocol_type(), expected);
    }
}

#[test]
fn unknown_environment_auto_accepts_kitty_and_sixel_records() {
    let inferred = detect_protocol_from_vars([("TERM", "xterm-256color")]);
    for (bytes, expected) in [
        (b"kitty 9 18\n".as_slice(), ProtocolType::Kitty),
        (b"sixel 12 24\n".as_slice(), ProtocolType::Sixel),
    ] {
        for override_value in ["auto", "AUTO"] {
            let called = Cell::new(false);
            let (picker, protocol) = choose_picker(inferred, override_value, None, || {
                called.set(true);
                Ok(PickerRecord::parse(bytes))
            })
            .unwrap();
            assert!(called.get(), "explicit auto must negotiate");
            assert_eq!(picker.protocol_type(), expected);
            assert!(protocol.is_native());
            assert_eq!(
                picker.font_size(),
                if expected == ProtocolType::Kitty {
                    (9, 18)
                } else {
                    (12, 24)
                }
            );
        }
    }
}

#[test]
fn auto_keeps_native_inference_and_uses_queried_font_size() {
    for inferred in [
        ImageProtocol::Kitty,
        ImageProtocol::Iterm2,
        ImageProtocol::Sixel,
    ] {
        let (picker, protocol) = choose_picker(inferred, "auto", None, || {
            Ok(PickerRecord::parse(b"halfblocks 9 18\n"))
        })
        .unwrap();
        assert_eq!(protocol, inferred);
        assert_eq!(picker.font_size(), (9, 18));
    }
}

#[test]
fn invalid_and_partial_records_use_inference_and_ioctl_fallback() {
    let invalid: &[&[u8]] = &[
        b"",
        b"kitty 9 18",
        b"kitty 9\n",
        b"kitty 0 18\n",
        b"kitty 9 0\n",
        b"kitty -1 18\n",
        b"kitty 65536 18\n",
        b"kitty 9 invalid\n",
        b"unknown 9 18\n",
        b"kitty 9 18 error\n",
        b"kitty 9 18\nsixel 9 18\n",
        b"kitty 9\n18\n",
        b"kitty 9 18\n\n",
        b"kitty 9 18\r\n",
        b" kitty 9 18\n",
        b"\xff 9 18\n",
    ];
    let size = window();
    for bytes in invalid {
        assert!(PickerRecord::parse(bytes).is_none(), "{bytes:?}");
        for inferred in [ImageProtocol::Halfblocks, ImageProtocol::Kitty] {
            let (picker, protocol) = choose_picker(inferred, "auto", Some(&size), || {
                Ok(PickerRecord::parse(bytes))
            })
            .unwrap();
            assert_eq!(protocol, inferred, "{bytes:?}");
            assert_eq!(picker.font_size(), (9, 18), "{bytes:?}");
        }
    }
    let oversized = vec![b' '; MAX_RECORD_BYTES + 1];
    assert!(PickerRecord::parse(&oversized).is_none());
}

#[test]
fn records_round_trip_all_protocols() {
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Iterm2,
        ImageProtocol::Sixel,
        ImageProtocol::Halfblocks,
    ] {
        let record = PickerRecord {
            protocol,
            font_size: (9, 18),
        };
        let mut bytes = Vec::new();
        record.write(&mut bytes).unwrap();
        assert!(bytes.len() <= MAX_RECORD_BYTES);
        assert_eq!(PickerRecord::parse(&bytes), Some(record));
    }
}

#[test]
fn picker_uses_window_pixel_geometry_for_native_protocols() {
    let size = window();
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Iterm2,
        ImageProtocol::Sixel,
    ] {
        assert_eq!(
            picker_for_window(protocol, Some(&size)).font_size(),
            (9, 18)
        );
    }
}

#[test]
fn picker_falls_back_for_missing_or_invalid_window_geometry() {
    assert_eq!(
        picker_for_window(ImageProtocol::Kitty, None).font_size(),
        (10, 20)
    );
    for (columns, rows, width, height) in [
        (80, 24, 0, 0),
        (0, 24, 720, 432),
        (80, 0, 720, 432),
        (80, 24, 0, 432),
        (80, 24, 720, 0),
        (80, 24, 79, 432),
        (80, 24, 720, 23),
    ] {
        let size = WindowSize {
            columns,
            rows,
            width,
            height,
        };
        assert_eq!(
            picker_for_window(ImageProtocol::Kitty, Some(&size)).font_size(),
            (10, 20)
        );
    }
}

#[test]
fn helper_mode_requires_its_single_internal_argument() {
    for arguments in [
        Vec::new(),
        vec!["--help"],
        vec![HELPER_ARGUMENT, "--help"],
        vec!["tui", HELPER_ARGUMENT],
    ] {
        assert!(!helper_requested(arguments.into_iter().map(OsString::from)));
    }
    let argument = OsString::from(HELPER_ARGUMENT);
    assert!(helper_requested(std::iter::once(argument)));
}

const fn window() -> WindowSize {
    WindowSize {
        columns: 80,
        rows: 24,
        width: 720,
        height: 432,
    }
}
