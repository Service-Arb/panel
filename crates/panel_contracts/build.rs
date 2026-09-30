//! `contracts/proto/sa/v1/*.proto` → Rust: prost for the messages, pbjson for their
//! proto3-JSON (protojson) serde impls. The well-known types (`Struct`, `Timestamp`) come
//! from `pbjson-types`, whose serde follows the same mapping.
//!
//! The protos are parsed by protox, in Rust, rather than by `protoc`: the build then needs
//! nothing installed, which the CI runners (plain Ubuntu) and the nix sandbox both rely on.

use std::{fs, path::PathBuf};

use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let proto_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/proto");
	let dir = proto_root.join("sa/v1");
	println!("cargo:rerun-if-changed={}", dir.display());
	let mut protos = Vec::new();
	for entry in fs::read_dir(&dir)? {
		let path = entry?.path();
		if path.extension().is_some_and(|ext| ext == "proto") {
			println!("cargo:rerun-if-changed={}", path.display());
			protos.push(path);
		}
	}
	protos.sort();

	let descriptors = protox::compile(&protos, [&proto_root])?;
	let encoded = descriptors.encode_to_vec();
	prost_build::Config::new()
		.compile_well_known_types()
		.extern_path(".google.protobuf", "::pbjson_types")
		.compile_fds(descriptors)?;

	pbjson_build::Builder::new().register_descriptors(&encoded)?.build(&[".sa"])?;
	Ok(())
}
