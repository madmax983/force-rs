use std::collections::HashMap;

fn main() {
    let v = vec![("a", 1), ("b", 2), ("c", 3)];
    let map: HashMap<&str, i32> = v.into_iter().collect();
    println!("{:?}", map.capacity());
}
