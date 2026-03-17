//! Build script for `force-pubsub`: compiles the Salesforce Pub/Sub proto.

// Build scripts don't need doc comments on every item.
#![allow(missing_docs)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    let mut prost_config = prost_build::Config::new();
    prost_config.protoc_executable(protoc);

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos_with_config(prost_config, &["proto/pubsub_api.proto"], &["proto"])?;

    Ok(())
}
