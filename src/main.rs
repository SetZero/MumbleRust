mod mumble;

use std::error::Error;
use std::net::{SocketAddr, ToSocketAddrs};
use num_derive::FromPrimitive;
use protobuf::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::join;
use tokio::net::TcpStream;
use byteorder::{BigEndian, ByteOrder};
use tokio_native_tls::native_tls::TlsConnector;
use crate::mumble::mumble::Version;

struct MessageInfo {
    pub message_type: u16,
    pub length: usize,
}

#[allow(dead_code)]
#[derive(FromPrimitive)]
enum NetworkMessage {
    Version = 0,
    UDPTunnel = 1,
    Authenticate = 2,
    Ping = 3,
    Reject = 4,
    ServerSync = 5,
    ChannelRemove = 6,
    ChannelState = 7,
    UserRemove = 8,
    UserState = 9,
    BanList = 10,
    TextMessage = 11,
    PermissionDenied = 12,
    ACL = 13,
    QueryUsers = 14,
    CryptSetup = 15,
    ContextActionModify = 16,
    ContextAction = 17,
    UserList = 18,
    VoiceTarget = 19,
    PermissionQuery = 20,
    CodecVersion = 21,
    UserStats = 22,
    RequestBlob = 23,
    ServerConfig = 24,
    SuggestConfig = 25,
}

fn serialize_message(message: NetworkMessage, buffer: &[u8]) -> Vec<u8> {
    let length: u32 = buffer.len() as u32;
    let encoded_msg = message as u16;
    let mut new_buffer = vec![0; (length + 6) as usize];
    BigEndian::write_u16(&mut new_buffer, encoded_msg);
    BigEndian::write_u32(&mut new_buffer[2..], length);
    new_buffer[6..].copy_from_slice(buffer);

    new_buffer
}

fn deserialize_message(buffer: &[u8]) -> Result<MessageInfo, &'static str> {
    if buffer.len() >= 6 {
        let message = BigEndian::read_u16(&buffer);
        let length = BigEndian::read_u32(&buffer[2..]) as usize;
        Ok(MessageInfo { message_type: message, length })
    } else {
        Err("Invalid message format")
    }
}

fn write_version() -> Result<impl AsRef<[u8]>, ()> {
    let version = mumble::mumble::Version {
        version: Some((1 << 16) | (3 << 8)),
        release: Some(String::from("1.3.0")),
        os: Some(String::from("Rust")),
        os_version: Some(String::from("11")),
        special_fields: Default::default(),
    };

    match &version.write_to_bytes() {
        Ok(data) => {
            Ok(serialize_message(NetworkMessage::Version, data))
        }
        Err(_) => Err(())
    }
}

fn write_auth(username: String) -> Result<impl AsRef<[u8]>, ()> {
    let auth = mumble::mumble::Authenticate {
        opus: Some(true),
        celt_versions: vec![-2147483637, -2147483632],
        password: None,
        tokens: vec![],
        username: Some(username),
        special_fields: Default::default(),
    };

    match &auth.write_to_bytes() {
        Ok(data) => {
            Ok(serialize_message(NetworkMessage::Authenticate, data))
        }
        Err(_) => Err(())
    }
}

async fn connect(
    server_host: String,
    server_addr: SocketAddr,
    user_name: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let socket = TcpStream::connect(&server_addr).await?;
    let cx = TlsConnector::builder().danger_accept_invalid_certs(true).build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let mut socket = cx.connect(&server_host, socket).await?;

    socket.write(write_version().unwrap().as_ref()).await?;
    socket.write(write_auth(user_name).unwrap().as_ref()).await?;


    //TODO: Add correct buffer handling (currently we don't have any option to handle overflowing
    // buffer data)
    let mut buffer = [0; 4096];
    loop {
        let n = socket.read(&mut buffer[..]).await.unwrap();
        process_message(&buffer[..n]);
    }
}

fn process_message(data: &[u8]) {
    let message_info = deserialize_message(data);
    if message_info.is_ok() {
        let message = message_info.unwrap();
        match num::FromPrimitive::from_u16(message.message_type) {
            Some(NetworkMessage::Version) => {
                match Version::parse_from_bytes(&data[6..6 + message.length]) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing: {:?}", e)
                }
            }
            _ => println!("Todo")
        }
    }
}

#[tokio::main]
async fn main() {
    // Handle command line arguments
    let server_host = "89.58.32.239".to_string();
    let server_port = 64738u16;
    let user_name = "Endor".to_string();
    let server_addr = (server_host.as_ref(), server_port).to_socket_addrs().expect("Failed to parse server address").next().expect("Failed to resolve server address");

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