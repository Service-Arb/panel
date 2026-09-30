//! `contracts/proto/sa/v1/*.proto` → Rust: prost for the messages, pbjson for their
//! proto3-JSON (protojson) serde impls. The well-known types (`Struct`, `Timestamp`) come
//! from `pbjson-types`, whose serde follows the same mapping.
//!
//! `contracts/proto/concierge/v1/*.proto` (vendored from concierge, see its README) → tonic
//! clients and servers for the sign-in: the panel is a relying party of concierge.
//!
//! The protos are parsed by protox, in Rust, rather than by `protoc`: the build then needs
//! nothing installed, which the CI runners (plain Ubuntu) and the nix sandbox both rely on.

use std::{
	fs,
	path::{Path, PathBuf},
};

use prost::Message;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let proto_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/proto");
	sa(&proto_root)?;
	concierge(&proto_root)?;
	Ok(())
}

fn protos_in(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
	println!("cargo:rerun-if-changed={}", dir.display());
	let mut protos = Vec::new();
	for entry in fs::read_dir(dir)? {
		let path = entry?.path();
		if path.extension().is_some_and(|ext| ext == "proto") {
			println!("cargo:rerun-if-changed={}", path.display());
			protos.push(path);
		}
	}
	protos.sort();
	Ok(protos)
}

fn sa(proto_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let protos = protos_in(&proto_root.join("sa/v1"))?;
	let descriptors = protox::compile(&protos, [proto_root])?;
	let encoded = descriptors.encode_to_vec();
	prost_build::Config::new()
		.compile_well_known_types()
		.extern_path(".google.protobuf", "::pbjson_types")
		.compile_fds(descriptors)?;

	pbjson_build::Builder::new().register_descriptors(&encoded)?.build(&[".sa"])?;
	Ok(())
}

fn concierge(proto_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let protos = protos_in(&proto_root.join("concierge/v1"))?;
	let descriptors = protox::compile(&protos, [proto_root])?;
	tonic_prost_build::configure().build_client(true).build_server(true).compile_fds(descriptors)?;
	Ok(())
}
