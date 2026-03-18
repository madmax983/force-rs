pub struct UrlEncodedWriter<'a>(pub &'a mut String);

impl std::fmt::Write for UrlEncodedWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0
            .extend(url::form_urlencoded::byte_serialize(s.as_bytes()));
        Ok(())
    }
}
