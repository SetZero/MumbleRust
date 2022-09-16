extern crate core;

mod mumble;
mod mumble_parser;
mod utils;

use std::error::Error;
use argparse::{ArgumentParser, Store};
use tokio::join;
use cpal::traits::{DeviceTrait, HostTrait};
use crate::mumble_parser::MumbleParser;
use crate::mumble_parser::network::{Network, TCPClient};

async fn connect(
    server_host: String,
    server_port: u16,
    user_name: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let network = Network::new();
    let mut parser = MumbleParser::new(Box::new(network));
    parser.connect(server_host, server_port, user_name).await.expect("TODO: panic message"); // TODO: Call network connect in this function
    loop {}
}

#[tokio::main]
async fn main() {
    handle_audio();
    // Handle command line arguments
    let (server_host, server_port, user_name) = process_args();

    // Run it
    let result = join!(
        connect(
            server_host,
            server_port,
            user_name
        )
    );

    match result {
        (Ok(_), ) => println!("Successfully got audio data!"),
        (Err(err), ) => println!("Something went wrong: {}", err)
    }
}

fn process_args() -> (String, u16, String) {
    let mut server_host = "89.58.32.239".to_string();
    let mut server_port = 64738u16;
    let mut user_name = "Endor".to_string();

    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Connect to a mumble server.");
        ap.refer(&mut server_host)
            .add_option(&["--host"], Store,
                        "hostname");
        ap.refer(&mut server_port)
            .add_option(&["-p", "--port"], Store,
                        "server port");
        ap.refer(&mut user_name)
            .add_option(&["-u", "--user"], Store,
                        "username");
        ap.parse_args_or_exit();
    }

    (server_host, server_port, user_name)
}

fn handle_audio() {
    let available_hosts = cpal::available_hosts();
    println!("Available hosts:\n  {:?}", available_hosts);
    for host_id in available_hosts {
        println!("{}", host_id.name());
        let host = cpal::host_from_id(host_id).unwrap();

        let default_in = host.default_input_device().map(|e| e.name().unwrap());
        let default_out = host.default_output_device().map(|e| e.name().unwrap());
        println!("  Default Input Device:\n    {:?}", default_in);
        println!("  Default Output Device:\n    {:?}", default_out);
    }
}