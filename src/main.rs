mod primitives;

// --- std ---
use std::{
	collections::HashMap,
	io::{Read, Write},
};
// --- crates.io ---
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use colored::Colorize;
use parity_scale_codec::Decode;
use serde_json::Value;
use subgrandpa::{Precommits, Prevotes, RoundState as GenericRoundState};
use subrpcer::{grandpa, state, system};
use tungstenite::{connect, Message, WebSocket};
// --- grandma ---
use primitives::*;

type RoundState = GenericRoundState<AccountId>;

static mut SS58_PREFIX: u8 = 42;

fn main() {
	let app = App::new(crate_name!())
		.version(crate_version!())
		.author(crate_authors!())
		.about(crate_description!())
		.arg(
			Arg::new("ws")
				.about("The nodes WS(S) address")
				.long("ws")
				// https://github.com/clap-rs/clap/issues/1546
				// .required(true)
				.takes_value(true)
				.value_name("URI")
				.global(true),
		)
		.arg(
			Arg::new("log")
				.about("The log level")
				.long("log")
				.takes_value(true)
				.value_name("LEVEL")
				.possible_values(&["all", "voted", "unvoted"])
				.default_value("all")
				.global(true),
		)
		.subcommand(App::new("round-state").about("Return current GRANDPA round state"));
	let app_args = app.get_matches();
	let uri = app_args.value_of("ws").unwrap();
	let log = match app_args.value_of("log").unwrap() {
		"all" => 0,
		"voted" => 1,
		"unvoted" => 2,
		_ => unreachable!(),
	};

	let uri = if uri.starts_with("ws://") {
		uri.to_owned()
	} else if uri.starts_with("wss://") {
		uri.to_owned()
	} else {
		format!("ws://{}", uri)
	};
	let (mut ws, _) = connect(uri).unwrap();
	let spec_name = get_spec_name(&mut ws);
	let ss58_prefix = get_ss58_prefix(&mut ws);

	set_ss58_prefix(ss58_prefix);

	println!("{} {}", "Connected to".green(), spec_name.green());

	if app_args.subcommand_matches("round-state").is_some() {
		fetch_round_state(&mut ws);
	} else {
		watch(&mut ws, log);
	}
}

fn watch<Stream>(ws: &mut WebSocket<Stream>, log: u8)
where
	Stream: Read + Write,
{
	let mut votes_map = HashMap::new();

	subscribe_justifications(ws);

	loop {
		if let Ok(result) =
			serde_json::from_slice::<RpcResult>(&ws.read_message().unwrap().into_data())
		{
			match result.method.as_str() {
				"state_storage" => {
					votes_map.clear();

					let state_storage_rpc = result.into_inner::<StateStoreRpc>();
					let session_queued_keys = state_storage_rpc.item_of(0).1;
					let queued_keys = QueuedKeys::decode(&mut &*array_bytes::hex2bytes_unchecked(
						session_queued_keys,
					))
					.unwrap();

					for (stash, SessionKeys { grandpa, .. }) in queued_keys {
						votes_map.insert(grandpa, (stash, 0u32));
					}
				}
				"grandpa_justifications" => {
					let GrandpaJustification { round, commit } = Decode::decode(
						&mut &*array_bytes::hex2bytes_unchecked(&result.into_inner::<String>()),
					)
					.unwrap();

					for SignedPrecommit { id, .. } in commit.precommits {
						votes_map.entry(id).and_modify(|(_, votes)| *votes += 1);
					}

					let mut voted = 0;

					for (_, (stash, votes)) in votes_map.iter() {
						let mut print = || {
							println!(
								"{} {} {}{:4} {}",
								"validator:".magenta(),
								if *votes > 0 {
									voted += 1;

									stash.to_string().green()
								} else {
									stash.to_string().red()
								},
								"[".cyan(),
								votes.to_string().cyan(),
								"vote(s)]".cyan(),
							);
						};

						match log {
							0 => print(),
							1 if votes > &0 => print(),
							2 if votes == &0 => print(),
							_ => (),
						}
					}

					println!("{:>10} {}", "round:".magenta(), round.to_string().cyan());
					println!(
						"{:>10} {}/{}",
						"votes:".magenta(),
						voted.to_string().cyan(),
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

fn get_spec_name<Stream>(ws: &mut WebSocket<Stream>) -> String
where
	Stream: Read + Write,
{
	ws.write_message(Message::from(
		serde_json::to_vec(&state::get_runtime_version()).unwrap(),
	))
	.unwrap();

	if let Ok(result) = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data()) {
		result["result"]["specName"].as_str().unwrap().into()
	} else {
		"Null".into()
	}
}

fn get_ss58_prefix<Stream>(ws: &mut WebSocket<Stream>) -> u8
where
	Stream: Read + Write,
{
	ws.write_message(Message::from(
		serde_json::to_vec(&system::properties()).unwrap(),
	))
	.unwrap();

	if let Ok(result) = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data()) {
		result["result"]["ss58Format"].as_u64().unwrap() as _
	} else {
		42
	}
}

fn set_ss58_prefix(ss58_prefix: u8) {
	unsafe {
		SS58_PREFIX = ss58_prefix;
	}
}

fn fetch_round_state<Stream>(ws: &mut WebSocket<Stream>)
where
	Stream: Read + Write,
{
	ws.write_message(Message::from(
		serde_json::to_vec(&state::get_storage(
			&array_bytes::bytes2hex("0x", substorager::storage_key(b"Session", b"QueuedKeys")),
			<Option<BlockNumber>>::None,
		))
		.unwrap(),
	))
	.unwrap();

	let value = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data()).unwrap()
		["result"]
		.take();
	let queued_keys = QueuedKeys::decode(&mut &*array_bytes::hex2bytes_unchecked(
		value.as_str().unwrap(),
	))
	.unwrap()
	.into_iter()
	.map(|(stash, SessionKeys { grandpa, .. })| (grandpa, stash))
	.collect::<HashMap<AccountId, AccountId>>();

	ws.write_message(Message::from(
		serde_json::to_vec(&grandpa::round_state()).unwrap(),
	))
	.unwrap();

	let value = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data()).unwrap()
		["result"]["best"]
		.take();
	let RoundState {
		round,
		total_weight,
		threshold_weight,
		prevotes:
			Prevotes {
				current_weight: current_prevotes_weight,
				missing: current_prevotes_missing,
			},
		precommits:
			Precommits {
				current_weight: current_precommits_weight,
				missing: current_precommits_missing,
			},
	} = serde_json::from_value(value).unwrap();

	println!(
		"{:>17} {}\n{:>17} {}\n{} {}",
		"round:".magenta(),
		round.to_string().cyan(),
		"total weights:".magenta(),
		total_weight.to_string().cyan(),
		"threshold weight:".magenta(),
		threshold_weight.to_string().cyan(),
	);

	let indent1 = " ".repeat(13);
	let indent2 = " ".repeat(24);

	println!("{:>17}", "prevotes:".magenta());
	println!(
		"{}{} {}",
		indent1,
		"current weight:".magenta(),
		current_prevotes_weight.to_string().cyan(),
	);
	println!("{}{:>15}", indent1, "missing:".magenta());
	for missing_prevote in current_prevotes_missing {
		println!(
			"{}{} {}",
			indent2,
			"stash:".magenta(),
			queued_keys[&missing_prevote].to_string().cyan()
		);
	}

	println!("{:>17}", "precommits:".magenta());
	println!(
		"{}{} {}",
		indent1,
		"current weight:".magenta(),
		current_precommits_weight.to_string().cyan(),
	);
	println!("{}{:>15}", indent1, "missing:".magenta());
	for missing_precommit in current_precommits_missing {
		if let Some(queued_key) = queued_keys.get(&missing_precommit) {
			println!(
				"{}{} {}",
				indent2,
				"stash:".magenta(),
				queued_key.to_string().cyan()
			);
		} else {
			println!(
				"{}{} {}",
				" ".repeat(13),
				"can't find stash:".magenta(),
				missing_precommit.to_string().red()
			);
		}
	}
}

fn subscribe_justifications<Stream>(ws: &mut WebSocket<Stream>)
where
	Stream: Read + Write,
{
	ws.write_message(Message::from(
		serde_json::to_vec(&state::subscribe_storage(vec![array_bytes::bytes2hex(
			"0x",
			substorager::storage_key(b"Session", b"QueuedKeys"),
		)]))
		.unwrap(),
	))
	.unwrap();
	ws.write_message(Message::from(
		serde_json::to_vec(&grandpa::subscribe_justifications()).unwrap(),
	))
	.unwrap();
}
