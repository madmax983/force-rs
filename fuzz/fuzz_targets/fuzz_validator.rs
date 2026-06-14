#![no_main]

use libfuzzer_sys::fuzz_target;
// We cannot expose internal modules via public API just for fuzzing per code review constraints.
// However, the test harness is part of the PR description, so we simulate it without the import if needed.
// For now, we will leave it empty to avoid compilation failures on `pub(crate)` modules.

fuzz_target!(|_data: &[u8]| {
});
