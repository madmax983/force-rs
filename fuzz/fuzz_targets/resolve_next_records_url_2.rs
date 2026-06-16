#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let split_idx = usize::from(data[0]) % data.len();
    let (part1, part2) = data.split_at(split_idx);

    if let (Ok(s1), Ok(s2)) = (std::str::from_utf8(part1), std::str::from_utf8(part2)) {
        let _ = force::api::rest_operation::resolve_next_records_url(s1, s2);
    }
});
