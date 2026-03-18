import re

with open('crates/force/src/client/mod.rs', 'r') as f:
    content = f.read()

content = content.replace('crate::api::bulk::BulkHandler::new(Default::default())', 'crate::api::bulk::BulkHandler::new(Arc::clone(&self.inner))')

with open('crates/force/src/client/mod.rs', 'w') as f:
    f.write(content)
