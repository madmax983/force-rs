use std::fmt::Write;

pub struct ApexGenerationOptions {
    pub generate_before_insert: bool,
    pub generate_after_insert: bool,
    pub generate_before_update: bool,
    pub generate_after_update: bool,
    pub generate_before_delete: bool,
    pub generate_after_delete: bool,
}

impl Default for ApexGenerationOptions {
    fn default() -> Self {
        Self {
            generate_before_insert: true,
            generate_after_insert: true,
            generate_before_update: true,
            generate_after_update: true,
            generate_before_delete: true,
            generate_after_delete: true,
        }
    }
}
