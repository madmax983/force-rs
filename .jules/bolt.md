**Remove unnecessary clones from JwtClaims**
**Learning:** `jsonwebtoken::encode` takes claims by reference, meaning there's no need to serialize owned `String` fields. Borrowing `&'a str` fields on local, single-use structs avoids unnecessary allocations during high-frequency tasks like JWT generation.
**Action:** Always check if a struct passed by reference to a third-party crate function actually needs owned data. If it's a short-lived DTO, use lifetimes and `&str` instead of `.clone()`.

**Avoid `.clone()` in `Arc::new()`**
**Learning:** Initializing an `Arc` by cloning the source variable (`Arc::new(val.clone())`) instead of moving it (`Arc::new(val)`) causes a completely unnecessary deep copy and heap allocation.
**Action:** When transferring ownership of a newly created object to an `Arc` where the original variable is no longer needed, pass ownership directly without calling `.clone()`.
