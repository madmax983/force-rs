#![no_main]

use force::api::SoqlQueryBuilder;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = SoqlQueryBuilder::new().from(s).try_build();
        let _ = SoqlQueryBuilder::new()
            .from("Account")
            .select(&[s])
            .try_build();
        let _ = SoqlQueryBuilder::new()
            .from("Account")
            .where_eq(s, "value")
            .try_build();
    }
});
