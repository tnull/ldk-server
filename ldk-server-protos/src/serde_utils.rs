// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Custom serde serializers for proto types.
//!
//! These are used via `#[serde(serialize_with = "...")]` attributes on generated
//! proto fields to produce human-readable output (hex strings for bytes, enum
//! names for integer enum fields).

use std::fmt::Write;

use serde::Serializer;

/// Serializes `Option<prost::bytes::Bytes>` as a hex string (or null).
pub fn serialize_opt_bytes_hex<S>(
	value: &Option<bytes::Bytes>, serializer: S,
) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	match value {
		Some(bytes) => {
			let hex = bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
				let _ = write!(acc, "{b:02x}");
				acc
			});
			serializer.serialize_some(&hex)
		},
		None => serializer.serialize_none(),
	}
}
