pub(crate) mod store {
    pub(crate) mod pg {
        pub(crate) mod checkpoint {
            pub struct CheckpointState;
        }
        pub use checkpoint::CheckpointState;
    }
}
pub use store::pg::CheckpointState;

fn main() {}
