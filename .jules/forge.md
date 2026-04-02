**[Extract HTTP client wrappers]**
**Learning:** When refactoring common request patterns across HTTP API traits (like `RestOperation`), avoid blindly extracting them into public trait default methods if URL resolution semantics (relative vs. absolute paths) differ between implementors. This can pollute the public API or cause silent runtime regressions where paths are double-resolved.
**Action:** Extract the shared logic into private, module-level standalone functions taking `&(impl Trait + ?Sized)` to safely share logic without polluting the public API.
