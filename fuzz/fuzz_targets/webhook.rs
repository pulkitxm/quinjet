#![no_main]

mod webhook_parser {
    include!("../../src/webhook_parser.rs");

    use libfuzzer_sys::fuzz_target;

    fuzz_target!(|input: &[u8]| {
        if let Ok(head) = std::str::from_utf8(input) {
            let _delivery = parse_delivery(head);
            let _length = content_length(head);
        }
    });
}
