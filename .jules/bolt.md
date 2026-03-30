**⚡ Bolt: Optimize Data Dictionary Generator String Allocations**
**Learning:** Replaced `format!` and `.join()` calls with `writeln!` and `write!` directly to a pre-allocated `String` buffer. Avoided temporary string heap allocations when generating Markdown table rows.
**Action:** Use `std::fmt::Write` to build large strings iteratively instead of concatenating multiple `String` allocations.

**Avoiding unnecessary manual `.join(",")` implementations**
**Learning:** Calling `.join(",")` directly on a slice of strings (e.g., `&[&str]`) in Rust natively calculates the exact capacity and allocates a single `String` without creating an intermediate `Vec`. Replacing this with a manual `String::with_capacity` and `push_str` loop is a de-optimization that reduces readability.
**Action:** Use `.join(",")` on slices. Only manually manage `String::with_capacity` and `push_str` loops when mapping or filtering an iterator that would otherwise require `.collect::<Vec<_>>().join(",")`.
