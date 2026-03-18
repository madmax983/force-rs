with open('crates/force/src/client/mod.rs', 'r') as f:
    for line in f:
        if 'bulk' in line.lower() or 'default' in line.lower():
            print(line.strip())
