import re

with open('crates/force/src/client/mod.rs', 'r') as f:
    content = f.read()

# Replace bulk implementation with a mutant
content = re.sub(r'pub fn bulk\(&self\) -> crate::api::bulk::BulkHandler<A> \{\s*crate::api::bulk::BulkHandler::new\(Arc::clone\(&self\.inner\)\)\s*\}', r'pub fn bulk(&self) -> crate::api::bulk::BulkHandler<A> { crate::api::bulk::BulkHandler::new(Default::default()) }', content, flags=re.DOTALL)

with open('crates/force/src/client/mod.rs', 'w') as f:
    f.write(content)
