Oh, I see: `BulkHandler::new(Default::default())` did NOT compile! I made a syntax error in `manual_mutant.py` replacing it and expected it to compile but actually it might have caused cargo test to FAIL with a COMPILATION error, but wait! My script output showed tests passed!
Wait, if my script showed tests passed, it means it DID compile?
Wait, if it compiled, THEN my manual mutation compiled, and `cargo test` ran, AND passed?
Let's see what `manual_mutant.py` did.
"content = re.sub(r'pub fn bulk...', ...)"

Wait! If the regex `re.sub` failed to match, it would NOT have changed the file! So it tested the unmodified file.
Let me verify if `manual_mutant.py` actually modified the file.
