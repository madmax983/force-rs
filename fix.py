import re
import os

filepath = 'crates/force/src/api/rest/query_stream.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace('pub struct QueryStream<T, A: Authenticator, O: RestOperation<A> + Clone> {', 'pub struct QueryStream<T, A: Authenticator, O: RestOperation<A> + Clone> {\n    _auth: std::marker::PhantomData<A>,')
content = content.replace('pub fn new(client: O, soql: impl Into<String>) -> Self {', 'pub fn new(client: O, soql: impl Into<String>) -> Self {')
content = content.replace('        Self {\n            client,', '        Self {\n            _auth: std::marker::PhantomData,\n            client,')
with open(filepath, 'w') as f:
    f.write(content)
