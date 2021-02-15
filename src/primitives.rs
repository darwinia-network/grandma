// --- std ---
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
// --- crates.io ---
use parity_scale_codec::Decode;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
// --- grandma ---
use crate::SS58_PREFIX;

pub type BlockNumber = u32;
pub type QueuedKeys = Vec<(AccountId, SessionKeys)>;
pub type GrandpaJustification =
	subgrandpa::GrandpaJustification<Hash, BlockNumber, Signature, AccountId>;
pub type SignedPrecommit = subgrandpa::SignedPrecommit<Hash, BlockNumber, Signature, AccountId>;

#[derive(Debug, Deserialize)]
pub struct RpcResult {
	pub method: String,
	pub params: Value,
}
impl RpcResult {
	const RESULT: &'static str = "result";

	pub fn into_inner<T: DeserializeOwned>(self) -> T {
		serde_json::from_value(self.params[Self::RESULT].to_owned()).unwrap()
	}
}

#[derive(Debug, Deserialize)]
pub struct StateStoreRpc {
	pub block: String,
	pub changes: Vec<Vec<String>>,
}
impl StateStoreRpc {
	pub fn key_of(&self, i: usize) -> &str {
		&self.changes[i][0]
	}

	pub fn value_of(&self, i: usize) -> &str {
		&self.changes[i][1]
	}

	pub fn item_of(&self, i: usize) -> (&str, &str) {
		(self.key_of(i), self.value_of(i))
	}
}

#[derive(Debug, Decode)]
pub struct SessionKeys {
	pub babe: AccountId,
	pub grandpa: AccountId,
	pub im_online: AccountId,
	pub authority_discovery: AccountId,
}

#[derive(Decode)]
pub struct Hash(pub [u8; 32]);
impl Debug for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Hash({})", array_bytes::hex_str("0x", &self.0))
	}
}
impl Display for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "{}", array_bytes::hex_str("0x", &self.0))
	}
}

#[derive(Decode, Eq, PartialEq, Hash)]
pub struct AccountId([u8; 32]);
impl Debug for AccountId {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		unsafe {
			write!(
				f,
				"AccountId({})",
				subcryptor::into_ss58_address(&self.0, SS58_PREFIX)
			)
		}
	}
}
impl Display for AccountId {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		unsafe { write!(f, "{}", subcryptor::into_ss58_address(&self.0, SS58_PREFIX)) }
	}
}

#[derive(Decode)]
pub struct Signature([u8; 64]);
impl Debug for Signature {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Signature(omitted)")
	}
}
