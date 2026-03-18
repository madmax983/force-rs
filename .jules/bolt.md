**⚡ Bolt: Optimize Data Dictionary Generator String Allocations**
**Learning:** Replaced `format!` and `.join()` calls with `writeln!` and `write!` directly to a pre-allocated `String` buffer. Avoided temporary string heap allocations when generating Markdown table rows.
**Action:** Use `std::fmt::Write` to build large strings iteratively instead of concatenating multiple `String` allocations.
