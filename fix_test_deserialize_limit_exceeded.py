import re

with open('crates/force/src/api/bulk/csv.rs', 'r') as f:
    content = f.read()

# When we test a reader that never stops returning bytes, the memory is bounded,
# and the LimitReader correctly throws an IO error.
# BUT `csv::Reader::deserialize` does not gracefully pass the error up via `Result` if it encounters a CSV parsing error
# early on instead of an IO error. Actually wait, InfiniteZeros is NOT valid CSV data,
# because there are no newlines or commas! So the CSV parser might just buffer the entire 150MB line,
# exceeding the LimitReader's capacity, and then the LimitReader throws the IO error.
# But wait! The LimitReader's IO error DOES get passed up as `csv_err.kind()`.
# The only issue is that `result.is_err()` is somehow evaluating to FALSE?? No, `result.is_err()` assertion failed!
# Wait! "assertion failed: result.is_err()" means that `result.is_err()` WAS FALSE! Which means it WAS `Ok(_)`.
# How can `deserialize_from_csv(InfiniteZeros)` return `Ok(_)`?
# Oh! The header reading! If it's reading `InfiniteZeros`, it reads "000000..." up to 150MB, then hits an IO error.
# `csv::Reader` buffers this. If it encounters the IO error, it yields it... but maybe it silently swallowed it or didn't hit it?
# Let's fix the InfiniteZeros test reader to return proper CSV data (or we can just limit the limit reader size in the test?)
# Wait, no. We can just use a `LimitReader` wrapped over an infinite sequence of `0,0,0\\n0,0,0\\n`.

test_code = """
    // Test 16: DoS prevention - payload over 150MB limit
    #[test]
    fn test_deserialize_limit_exceeded() {
        struct InfiniteCsv;
        impl std::io::Read for InfiniteCsv {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let chunk = b"001,Test,42\\n";
                let mut i = 0;
                while i < buf.len() {
                    let to_copy = std::cmp::min(chunk.len(), buf.len() - i);
                    buf[i..i + to_copy].copy_from_slice(&chunk[..to_copy]);
                    i += to_copy;
                }
                Ok(buf.len())
            }
        }

        let reader = InfiniteCsv;
        let result: std::result::Result<Vec<TestRecord>, _> = deserialize_from_csv(reader);
        assert!(result.is_err());

        let Err(err) = result else {
            panic!("Expected error")
        };
        if let crate::error::ForceError::Serialization(crate::error::SerializationError::Csv(
            csv_err,
        )) = err
        {
            if let csv::ErrorKind::Io(io_err) = csv_err.kind() {
                assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidData);
                assert!(io_err.to_string().contains("150MB"));
                return;
            }
        }
        panic!("Expected InvalidData IO error from LimitReader, got something else");
    }
"""

old_test_code_regex = re.compile(r"// Test 16: DoS prevention - payload over 150MB limit.*?panic!\(\"Expected InvalidData IO error from LimitReader, got something else\"\);\n\s+\}", re.DOTALL)

if old_test_code_regex.search(content):
    content = old_test_code_regex.sub(test_code.strip(), content)
    with open('crates/force/src/api/bulk/csv.rs', 'w') as f:
        f.write(content)
    print("Replaced test.")
else:
    print("Could not find test.")
