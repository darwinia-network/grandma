// --- std ---
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
// --- crates.io ---
use parity_scale_codec::Decode;
use serde::{de::DeserializeOwned, Deserialize, Deserializer};
use serde_json::Value;
use subgrandpa::{
	GrandpaJustification as GenericGrandpaJustification, RoundState as GenericRoundState,
	SignedPrecommit as GenericSignedPrecommit,
};
// --- grandma ---
use crate::SS58_PREFIX;

pub type BlockNumber = u32;
pub type GrandpaJustification =
	GenericGrandpaJustification<Hash, BlockNumber, Signature, AccountId>;
pub type SignedPrecommit = GenericSignedPrecommit<Hash, BlockNumber, Signature, AccountId>;
pub type RoundState = GenericRoundState<AccountId>;

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

#[derive(Decode)]
pub struct Hash(pub [u8; 32]);
impl Debug for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Hash({})", array_bytes::bytes2hex("0x", &self.0))
	}
}
impl Display for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "{}", array_bytes::bytes2hex("0x", &self.0))
	}
}

#[derive(Clone, Eq, PartialEq, Hash, Decode)]
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
impl<'de> Deserialize<'de> for AccountId {
	fn deserialize<D>(deserializer: D) -> Result<AccountId, D::Error>
	where
		D: Deserializer<'de>,
	{
		Ok(AccountId(array_bytes::dyn2array!(
			subcryptor::into_public_key(String::deserialize(deserializer)?),
			32
		)))
	}
}

#[derive(Decode)]
pub struct Signature([u8; 64]);
impl Debug for Signature {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Signature(omitted)")
	}
}
