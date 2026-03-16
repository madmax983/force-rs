import re

with open('crates/force/src/api/bulk/csv.rs', 'r') as f:
    content = f.read()

new_code = """
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
"""

content = re.sub(r'let Err\(err\) = result else \{.*?panic!\("Expected InvalidData IO error from LimitReader, got something else"\);', new_code.strip(), content, flags=re.DOTALL)

with open('crates/force/src/api/bulk/csv.rs', 'w') as f:
    f.write(content)
