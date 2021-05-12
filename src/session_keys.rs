// --- crates.io ---
use parity_scale_codec::Decode;
// --- grandma ---
use crate::primitives::AccountId;

pub type QueuedKeys<SK> = Vec<(AccountId, SK)>;

pub trait SessionKeys: Decode {
	fn grandpa(&self) -> &AccountId;
}

#[derive(Debug, Decode)]
pub struct PolkadotSessionKeys {
	pub grandpa: AccountId,
	pub babe: AccountId,
	pub im_online: AccountId,
	pub para_validator: AccountId,
	pub para_assignment: AccountId,
	pub authority_discovery: AccountId,
}
impl SessionKeys for PolkadotSessionKeys {
	fn grandpa(&self) -> &AccountId {
		&self.grandpa
	}
}

#[derive(Debug, Decode)]
pub struct DarwiniaSessionKeys {
	pub babe: AccountId,
	pub grandpa: AccountId,
	pub im_online: AccountId,
	pub authority_discovery: AccountId,
}
impl SessionKeys for DarwiniaSessionKeys {
	fn grandpa(&self) -> &AccountId {
		&self.grandpa
	}
}
