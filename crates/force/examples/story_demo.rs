// This example demonstrates the Nova story generation feature.
// It requires the "nova" feature to be enabled.
//
// cargo run --example story_demo --features nova

use force::experimental::story::NarrativeGenerator;

fn main() {
    let generator = NarrativeGenerator::new();
    println!("{}", generator.generate());
}
