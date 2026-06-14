#![no_main]

use force::http::parse_api_error;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_api_error(400, s);
    }
});
