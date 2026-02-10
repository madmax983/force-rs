import sys
import os

filepath = sys.argv[1]
feature = sys.argv[2]

with open(filepath, 'r') as f:
    content = f.read()

if "#![cfg(feature" in content:
    print(f"Skipping {filepath}, already guarded")
    sys.exit(0)

# Check if main returns Result
has_result = "-> anyhow::Result<()>" in content or "-> Result<()>" in content

new_content = f"""#![cfg(feature = "{feature}")]

{content}
"""

# Append dummy main for when feature is disabled
# But wait, #![cfg] at top level effectively removes the module content.
# If this is an example, it is treated as a bin.
# A bin must have a main function.
# If I use #![cfg(feature = "bulk")], the file is empty when feature is missing -> error "main function not found".
# So I cannot use #![cfg] at top level.

# I must wrap the main function.
# Or better, rename main to real_main and add a wrapper.

wrapped_content = f"""#[cfg(feature = "{feature}")]
{content}

#[cfg(not(feature = "{feature}"))]
fn main() {{
    println!("This example requires the '{feature}' feature.");
}}
"""

# This naive wrapping puts imports inside the cfg block which is fine,
# but "content" has "fn main". I need to rename it or just guard it?
# If I guard the imports and the main function, it works.
# But "content" is the whole file.

# Let's try to just prepend the cfg check to main and imports?
# That's hard to parse.

# Alternative: use a main that calls inner main.
# But imports are top level.

# Plan B: Just wrap the whole file content in `#[cfg(feature = "...")] mod example { ... }`
# and call `example::main()`? No, main must be top level.

# Let's stick to:
# #[cfg(feature = "bulk")]
# mod example {
#    <original content>
# }
#
# #[cfg(feature = "bulk")]
# fn main() -> anyhow::Result<()> {
#    example::main()
# }
#
# #[cfg(not(feature = "bulk"))]
# fn main() {}

# But original content has `fn main`. `example::main` works.
# But original content might have `use` statements that conflict if not in mod?
# Actually putting everything in a mod is safe.

with open(filepath, 'w') as f:
    f.write(f'#[cfg(feature = "{feature}")]\nmod example {{\n')
    for line in content.splitlines():
        f.write(f'    {line}\n')
    f.write('}\n\n')
    f.write(f'#[cfg(feature = "{feature}")]\n')
    f.write('fn main() -> anyhow::Result<()> {\n')
    f.write('    example::main()\n')
    f.write('}\n\n')
    f.write(f'#[cfg(not(feature = "{feature}"))]\n')
    f.write('fn main() {}\n')

print(f"Wrapped {filepath}")
