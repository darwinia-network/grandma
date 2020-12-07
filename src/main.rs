// --- std ---
use std::{
	collections::HashMap,
	fmt::{Debug, Display, Formatter, Result as FmtResult},
	hash::Hasher,
};
// --- crates.io ---
use base58::ToBase58;
use blake2_rfc::blake2b::Blake2b;
use byteorder::{ByteOrder, LittleEndian};
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use colored::Colorize;
use parity_scale_codec::Decode;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use tungstenite::{connect, Message};
use twox_hash::XxHash;

type BlockNumber = u32;
type QueuedKeys = Vec<(AccountId, SessionKeys)>;

fn main() {
	let app = App::new(crate_name!())
		.version(crate_version!())
		.author(crate_authors!())
		.about(crate_description!())
		.arg(
			Arg::new("ws")
				.long("ws")
				.required(true)
				.takes_value(true)
				.value_name("URI:PORT")
				.about("The nodes WS address"),
		)
		.arg(
			Arg::new("log")
				.long("log")
				.takes_value(true)
				.value_name("LEVEL")
				.possible_values(&["all", "voted", "unvoted"])
				.default_value("all")
				.about("The log level"),
		);
	let app_args = app.get_matches();
	let ws = app_args.value_of("ws").unwrap();
	let log = match app_args.value_of("log").unwrap() {
		"all" => 0,
		"voted" => 1,
		"unvoted" => 2,
		_ => unreachable!(),
	};

	run(ws, log);
}

fn run(ws: &str, log: u8) {
	let ws = if ws.starts_with("ws://") {
		ws.to_owned()
	} else if ws.starts_with("wss://") {
		ws.to_owned()
	} else {
		format!("ws://{}", ws)
	};
	let (mut socket, _) = connect(ws).unwrap();

	println!("{}", "Connected to Darwinia".green());

	socket
		.write_message(Message::Text(
			r#"{"id":2,"jsonrpc":"2.0","method":"grandpa_subscribeJustifications","params":[]}"#
				.into(),
		))
		.unwrap();
	socket
		.write_message(Message::Text(
			format!(
				r#"{{"id":2,"jsonrpc":"2.0","method":"state_subscribeStorage","params":[["{}"]]}}"#,
				format!(
					"0x{}{}",
					hex(&twox_128(b"Session")),
					hex(&twox_128(b"QueuedKeys"))
				)
			)
			.into(),
		))
		.unwrap();

	let mut votes_map = HashMap::new();

	loop {
		if let Ok(rpc_result) =
			serde_json::from_slice::<RpcResult>(&socket.read_message().unwrap().into_data())
		{
			match rpc_result.method.as_str() {
				"state_storage" => {
					let state_storage_rpc = rpc_result.into_inner::<StateStoreRpc>();
					let session_queued_keys = state_storage_rpc.item_of(0).1;
					let queued_keys: QueuedKeys =
						Decode::decode(&mut &*bytes(session_queued_keys)).unwrap();

					votes_map.clear();

					for (stash, SessionKeys { grandpa, .. }) in queued_keys {
						votes_map.insert(grandpa, (stash, 0u32));
					}
				}
				"grandpa_justifications" => {
					let GrandpaJustification { round, commit } =
						Decode::decode(&mut &*bytes(&rpc_result.into_inner::<String>())).unwrap();

					for SignedPrecommit { id, .. } in commit.precommits {
						votes_map.entry(id).and_modify(|(_, votes)| *votes += 1);
					}

					for (_, (stash, votes)) in votes_map.iter() {
						let print = || {
							println!(
								"{}{}{}{:4}{}",
								"validator: ".magenta(),
								if *votes > 0 {
									stash.to_string().green()
								} else {
									stash.to_string().red()
								},
								" [".cyan(),
								votes.to_string().cyan(),
								" vote(s)]".cyan(),
							);
						};

						match log {
							0 => print(),
							1 if votes > &0 => print(),
							2 if votes == &0 => print(),
							_ => (),
						}
					}

					println!("{}{}", "round    : ".magenta(), round.to_string().cyan());
					println!(
						"{}{}",
						"total    : ".magenta(),
						votes_map.len().to_string().cyan()
					);
					println!(
						"{}",
						"=========================================================================="
							.yellow()
					);
				}
				_ => (),
			}
		}
	}
}

#[derive(Debug, Deserialize)]
struct RpcResult {
	method: String,
	params: Value,
}
impl RpcResult {
	const RESULT: &'static str = "result";

	fn into_inner<T: DeserializeOwned>(self) -> T {
		serde_json::from_value(self.params[Self::RESULT].to_owned()).unwrap()
	}
}

#[derive(Debug, Deserialize)]
struct StateStoreRpc {
	block: String,
	changes: Vec<Vec<String>>,
}
impl StateStoreRpc {
	fn key_of(&self, i: usize) -> &str {
		&self.changes[i][0]
	}

	fn value_of(&self, i: usize) -> &str {
		&self.changes[i][1]
	}

	fn item_of(&self, i: usize) -> (&str, &str) {
		(self.key_of(i), self.value_of(i))
	}
}

#[derive(Debug, Decode)]
struct SessionKeys {
	babe: AccountId,
	grandpa: AccountId,
	im_online: AccountId,
	authority_discovery: AccountId,
}

#[derive(Debug, Decode)]
struct GrandpaJustification {
	round: u64,
	commit: Commit,
}

#[derive(Debug, Decode)]
struct Commit {
	target_hash: Hash,
	target_number: BlockNumber,
	precommits: Vec<SignedPrecommit>,
}

#[derive(Debug, Decode)]
struct SignedPrecommit {
	precommit: Precommit,
	signature: Signature,
	id: AccountId,
}

#[derive(Debug, Decode)]
struct Precommit {
	target_hash: Hash,
	target_number: BlockNumber,
}

#[derive(Decode)]
struct Hash([u8; 32]);
impl Debug for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Hash({})", hex(&self.0))
	}
}
impl Display for Hash {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "{}", hex(&self.0))
	}
}

#[derive(Decode, Eq, PartialEq, Hash)]
struct AccountId([u8; 32]);
impl Debug for AccountId {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "AccountId({})", into_ss58(&self.0, 18))
	}
}
impl Display for AccountId {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "{}", into_ss58(&self.0, 18))
	}
}

#[derive(Decode)]
struct Signature([u8; 64]);
impl Debug for Signature {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(f, "Signature(omitted)")
	}
}

fn hex(s: &[u8]) -> String {
	s.iter()
		.map(|c| format!("{:02x}", c))
		.collect::<Vec<_>>()
		.join("")
}

fn bytes(s: &str) -> Vec<u8> {
	let s = s.trim_start_matches("0x");

	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
		.collect()
}

fn twox_128(data: &[u8]) -> [u8; 16] {
	let mut dest: [u8; 16] = [0; 16];
	let r0 = {
		let mut h0 = XxHash::with_seed(0);
		h0.write(data);

		h0.finish()
	};
	let r1 = {
		let mut h1 = XxHash::with_seed(1);
		h1.write(data);

		h1.finish()
	};

	LittleEndian::write_u64(&mut dest[0..8], r0);
	LittleEndian::write_u64(&mut dest[8..16], r1);

	dest
}

fn into_ss58(bytes: &[u8], network: u8) -> String {
	let mut account = {
		let mut data = vec![network];
		data.extend(bytes);

		data
	};

	let blake2b = {
		let mut context = Blake2b::new(64);
		context.update(b"SS58PRE");
		context.update(&account);

		context.finalize()
	};
	account.extend(&blake2b.as_bytes()[0..2]);

	account.to_base58()
}
