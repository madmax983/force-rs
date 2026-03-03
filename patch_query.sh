cat << 'INNER_EOF' > patch.py
import re

with open("crates/force/src/api/rest/query.rs", "r") as f:
    content = f.read()

# Replace token logic
old_logic = """        // Get token to access instance URL
        let token = self.inner.token_manager.get_token_arc().await?;
        let instance_url = token.instance_url();"""

new_logic = """        // Get instance URL directly from session
        let instance_url = self.inner.instance_url().await?;"""

if old_logic in content:
    content = content.replace(old_logic, new_logic)
    with open("crates/force/src/api/rest/query.rs", "w") as f:
        f.write(content)
    print("Patched query.rs successfully")
else:
    print("Could not find old logic in query.rs")
INNER_EOF
python3 patch.py
