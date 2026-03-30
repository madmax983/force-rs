**⚡ Bolt: Optimize Data Dictionary Generator String Allocations**
**Learning:** Replaced `format!` and `.join()` calls with `writeln!` and `write!` directly to a pre-allocated `String` buffer. Avoided temporary string heap allocations when generating Markdown table rows.
**Action:** Use `std::fmt::Write` to build large strings iteratively instead of concatenating multiple `String` allocations.

**[Remove `format!` within `push_str` loops]
**Learning:** Using `out.push_str(&format!(...))` inside loops creates unnecessary intermediate heap allocations. Using `std::fmt::Write` macros (`write!`, `writeln!`) directly into a pre-allocated `String` buffer completely eliminates these allocations.
**Action:** Always prefer `write!` and `writeln!` over `push_str(&format!(...))` when building large strings incrementally.
