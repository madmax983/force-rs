import sys
import re

filepath = sys.argv[1]

with open(filepath, 'r') as f:
    lines = f.readlines()

doc_lines = []
code_lines = []
in_mod_example = False

for line in lines:
    stripped = line.strip()
    if stripped.startswith("mod example {"):
        in_mod_example = True
        code_lines.append(line)
        continue

    if in_mod_example and stripped.startswith("//!"):
        doc_lines.append(line)
    else:
        code_lines.append(line)

# Now reconstruct the file content
# Docs should go at the very top
new_content = "".join(doc_lines)
# Then the rest of the code
new_content += "".join(code_lines)

# But wait, the previous wrapper put #[cfg(...)] at the top.
# #[cfg(feature = "bulk")]
# mod example { ... }
#
# If I just prepend docs, it will be:
# //! Docs
# #[cfg(feature = "bulk")]
# mod example { ... }
#
# This is correct. The docs apply to the crate (example binary).

with open(filepath, 'w') as f:
    f.write(new_content)

print(f"Hoisted docs for {filepath}")
