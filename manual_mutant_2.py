with open('crates/force/src/client/mod.rs', 'r') as f:
    c = f.read()

# Let's replace the actual body of bulk:
c = c.replace('crate::api::bulk::BulkHandler::new(Arc::clone(&self.inner))', 'crate::api::bulk::BulkHandler::new(Default::default())')

with open('crates/force/src/client/mod.rs', 'w') as f:
    f.write(c)
