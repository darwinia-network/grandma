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
use subrpcer::{grandpa, state, system};
use tungstenite::{connect, Message, WebSocket};
// --- grandma ---
use primitives::*;

static mut SS58_PREFIX: u8 = 42;

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
				.value_name("URI")
				.about("The nodes WS(S) address"),
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
	let uri = app_args.value_of("ws").unwrap();
	let log = match app_args.value_of("log").unwrap() {
		"all" => 0,
		"voted" => 1,
		"unvoted" => 2,
		_ => unreachable!(),
	};

	run(uri, log);
}

fn run(uri: &str, log: u8) {
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
	subscribe(&mut ws);

	println!("{} {}", "Connected to".green(), spec_name.green());

	let mut votes_map = HashMap::new();

	loop {
		if let Ok(rpc_result) =
			serde_json::from_slice::<RpcResult>(&ws.read_message().unwrap().into_data())
		{
			match rpc_result.method.as_str() {
				"state_storage" => {
					let state_storage_rpc = rpc_result.into_inner::<StateStoreRpc>();
					let session_queued_keys = state_storage_rpc.item_of(0).1;
					let queued_keys: QueuedKeys =
						Decode::decode(&mut &*array_bytes::bytes_unchecked(session_queued_keys))
							.unwrap();

					votes_map.clear();

					for (stash, SessionKeys { grandpa, .. }) in queued_keys {
						votes_map.insert(grandpa, (stash, 0u32));
					}
				}
				"grandpa_justifications" => {
					let GrandpaJustification { round, commit } = Decode::decode(
						&mut &*array_bytes::bytes_unchecked(&rpc_result.into_inner::<String>()),
					)
					.unwrap();

					for SignedPrecommit { id, .. } in commit.precommits {
						votes_map.entry(id).and_modify(|(_, votes)| *votes += 1);
					}

					let mut voted = 0;

					for (_, (stash, votes)) in votes_map.iter() {
						let mut print = || {
							println!(
								"{}{}{}{:4}{}",
								"validator: ".magenta(),
								if *votes > 0 {
									voted += 1;

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
						"{}{}/{}",
						"votes    : ".magenta(),
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

	if let Ok(rpc_result) = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data())
	{
		rpc_result["result"]["specName"].as_str().unwrap().into()
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

	if let Ok(rpc_result) = serde_json::from_slice::<Value>(&ws.read_message().unwrap().into_data())
	{
		rpc_result["result"]["ss58Format"].as_u64().unwrap() as _
	} else {
		42
	}
}

fn set_ss58_prefix(ss58_prefix: u8) {
	unsafe {
		SS58_PREFIX = ss58_prefix;
	}
}

fn subscribe<Stream>(ws: &mut WebSocket<Stream>)
where
	Stream: Read + Write,
{
	ws.write_message(Message::from(
		serde_json::to_vec(&grandpa::subscribe_justifications()).unwrap(),
	))
	.unwrap();
	ws.write_message(Message::from(
		serde_json::to_vec(&state::subscribe_storage(vec![array_bytes::hex_str(
			"0x",
			substorager::storage_key(b"Session", b"QueuedKeys"),
		)]))
		.unwrap(),
	))
	.unwrap();
}
