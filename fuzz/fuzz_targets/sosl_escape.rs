#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if !s.trim().is_empty() {
            let _ = force::api::rest::SearchQueryBuilder::new()
                .find(s)
                .in_all_fields()
                .returning("Account", &["Id"])
                .build();
        }
    }
});
