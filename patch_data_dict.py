def process_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    old_code = '''            if include_usage {
                if let Some(usage) = usage_map.get(&field.name) {
                    let _ = writeln!(md, " | {:.1}% |", usage.percentage);
                } else {
                    md.push_str(" | N/A |\\n");
                }
            } else {
                md.push_str(" |\\n");
            }'''

    new_code = '''            if !include_usage {
                md.push_str(" |\\n");
                continue;
            }

            if let Some(usage) = usage_map.get(&field.name) {
                let _ = writeln!(md, " | {:.1}% |", usage.percentage);
            } else {
                md.push_str(" | N/A |\\n");
            }'''

    content = content.replace(old_code, new_code)

    with open(filepath, 'w') as f:
        f.write(content)

process_file('crates/force/src/experimental/data_dictionary.rs')
