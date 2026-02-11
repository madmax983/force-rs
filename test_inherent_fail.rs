mod a {
    pub struct S;
}

mod b {
    use crate::a::S;
    impl S {
        pub fn foo(&self) {}
    }
}

fn main() {
    let s = a::S;
    s.foo();
}
