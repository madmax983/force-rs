pub struct UrlEncodedWriter<'a>(pub &'a mut String);

impl std::fmt::Write for UrlEncodedWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0
            .extend(url::form_urlencoded::byte_serialize(s.as_bytes()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn test_url_encoded_writer() -> Result<(), std::fmt::Error> {
        let mut out = String::new();
        {
            let mut writer = UrlEncodedWriter(&mut out);
            write!(writer, "hello world&foo=bar")?;
        }
        assert_eq!(out, "hello+world%26foo%3Dbar");

        {
            let mut writer = UrlEncodedWriter(&mut out);
            write!(writer, " test")?;
        }
        assert_eq!(out, "hello+world%26foo%3Dbar+test");
        Ok(())
    }
}
