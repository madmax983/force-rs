
fn main() {
    let csv_data = "id,name,value\n"
        .repeat(10_000)
        .into_bytes();
    let reader = csv_data.as_slice();
    println!("hello");
}
