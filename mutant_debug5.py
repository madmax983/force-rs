with open('crates/force/src/client/mod.rs', 'r') as f:
    for line in f:
        if 'mod tests' in line:
            print("Tests found")
