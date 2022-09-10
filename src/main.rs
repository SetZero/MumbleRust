extern crate core;

mod mumble;
mod mumble_parser;
mod utils;
use utils::networking::NetworkMessage;

use std::error::Error;
use std::net::{SocketAddr, ToSocketAddrs};
use argparse::{ArgumentParser, Store};
use tokio::join;
use tokio::net::TcpStream;
use cpal::traits::{DeviceTrait, HostTrait};
use tokio_native_tls::native_tls::TlsConnector;
use mumble_parser::MumbleParser;

async fn connect(
    server_host: String,
    server_addr: SocketAddr,
    user_name: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let socket = TcpStream::connect(&server_addr).await?;
    let cx = TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let socket = cx.connect(&server_host, socket).await?;
    let mut parser = MumbleParser::new(socket, user_name);
    parser.start().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    handle_audio();
    // Handle command line arguments
    let (server_host, user_name, server_addr) = process_args();

    // Run it
    let result = join!(
        connect(
            server_host,
            server_addr,
            user_name
        )
    );

    match result {
        (Ok(_), ) => println!("Successfully got data!"),
        (Err(err), ) => println!("Something went wrong: {}", err)
    }
}

fn process_args() -> (String, String, SocketAddr) {
    let mut server_host = "89.58.32.239".to_string();
    let mut server_port = 64738u16;
    let mut user_name = "Endor".to_string();
    let server_addr = (server_host.as_ref(), server_port).to_socket_addrs().expect("Failed to parse server address").next().expect("Failed to resolve server address");

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

    (server_host, user_name, server_addr)
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