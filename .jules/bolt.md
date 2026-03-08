**Remove unnecessary clones from JwtClaims**
**Learning:** `jsonwebtoken::encode` takes claims by reference, meaning there's no need to serialize owned `String` fields. Borrowing `&'a str` fields on local, single-use structs avoids unnecessary allocations during high-frequency tasks like JWT generation.
**Action:** Always check if a struct passed by reference to a third-party crate function actually needs owned data. If it's a short-lived DTO, use lifetimes and `&str` instead of `.clone()`.
