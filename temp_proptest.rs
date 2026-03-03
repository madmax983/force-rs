use std::convert::TryFrom;
use std::time::Duration;
fn main() {
    let result = (u64::MAX as u128) * 2;
    println!("{}", result);
}
