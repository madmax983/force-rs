import re

with open("crates/force/src/api/soql.rs", "r") as f:
    code = f.read()

code = re.sub(
    r'    pub fn try_build\(self\) -> Result<String, ForceError> \{\n        if let Some\(err\) = self\.error \{\n            return Err\(ForceError::InvalidInput\(err\)\);\n        \}\n        if let Some\(err\) = self\.error \{\n            return Err\(ForceError::InvalidInput\(err\)\);\n        \}\n        self\.validate\(\)\?;',
    '''    pub fn try_build(self) -> Result<String, ForceError> {
        self.validate()?;''',
    code, count=1
)

with open("crates/force/src/api/soql.rs", "w") as f:
    f.write(code)
