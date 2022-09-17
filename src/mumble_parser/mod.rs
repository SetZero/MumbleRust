use std::borrow::BorrowMut;
use std::cmp;
use std::collections::HashMap;
use std::error::Error;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use byteorder::{BigEndian, ByteOrder};
use protobuf::Message;
use tokio::sync::{mpsc, Mutex};
use tokio::time;

use crate::{Network, TCPClient};
use crate::mumble::mumble;
use crate::mumble::mumble::{ChannelState, CodecVersion, CryptSetup, PermissionQuery, ServerConfig, ServerSync, TextMessage, UserRemove, UserState, Version};
use crate::mumble_parser::network::{TCPReceiver, TCPSender};
use crate::utils::networking::NetworkMessage;

pub mod network;

const METADATA_SIZE: usize = 6;

#[derive(Debug)]
struct MessageInfo {
    pub message_type: u16,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct MumbleChannel {
    pub name: String,
}

pub struct MumbleParser {
    channels: HashMap<u32, MumbleChannel>,
    /*network: Box<dyn TCPClient>,*/
}

impl MumbleParser {
    pub fn new() -> Self {
        MumbleParser { channels: HashMap::new() }
    }

    pub async fn connect(self: Arc<Self>, server_host: String, server_port: u16, user_name: String) -> Result<(), Box<dyn Error>> {
        let network = Arc::new(Mutex::new(Box::new(Network::new())));
        {
            let mut network_lock = network.deref().lock().await;
            network_lock.connect(server_host, server_port).await.expect("TODO: panic message");
            network_lock.send_message(write_version().unwrap()).await?;
            network_lock.send_message(write_auth(user_name).unwrap()).await?;
        }

        let (parse_tx, mut parse_rx) = mpsc::channel::<(u16, Vec<u8>)>(4096);

        tokio::spawn({
            let mut this = Arc::clone(&self);
            let net = network.clone();
            async move {
                let result = this.parse_message(net.deref(), parse_tx).await;
                if result.is_err() {
                    println!("Error: {:?}", result);
                }
            }
        });

        tokio::spawn({
            let mut this = Arc::clone(&self);
            async move {
                loop {
                    tokio::select! {
                        _ = this.task_inner(network.clone()) => {},
                        Some((msg_type, payload)) = parse_rx.recv() => {
                            this.process_message(msg_type, payload);
                        }
                    }
                };
            }
        });

        Ok(())
    }

    async fn task_inner(&self, network: Arc<Mutex<Box<Network>>>) {
        let mut interval = time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let mut network_lock = network.deref().lock().await;
            //network_lock.send_message(write_ping().unwrap()).await;
            println!("Ping");
        }
    }

    pub fn get_channels(&self) -> Vec<MumbleChannel> {
        self.channels.values().cloned().collect::<Vec<MumbleChannel>>()
    }

    async fn parse_message(&self, network: &Mutex<Box<Network>>, parse_channel: Sender<(u16, Vec<u8>)>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tmp_buffer = Vec::<u8>::new();

        let mut buffer_size = 0;
        let mut buffer_last_read = 0;

        loop {
            let mut n = 0;
            let mut metadata_message_buf = Vec::<u8>::new();
            while n < METADATA_SIZE {
                let old_buffer_last_read;
                if buffer_size <= buffer_last_read {
                    buffer_last_read = 0;
                    //buffer_size = 0;
                    {
                        //TODO: This lock causes semi-deadlock, because it is waiting for the channel
                        //      to get a message, but that requires an incoming message :-/
                        let mut network_lock = network.deref().lock().await;
                        tmp_buffer = network_lock.get_message().await.unwrap_or_default();
                    }
                    n += tmp_buffer.len();

                    old_buffer_last_read = buffer_last_read;
                    buffer_last_read += cmp::min(METADATA_SIZE - metadata_message_buf.len(), n);
                    buffer_size = n - metadata_message_buf.len();
                } else {
                    old_buffer_last_read = buffer_last_read;
                    let read_bytes = cmp::min(METADATA_SIZE, buffer_size - buffer_last_read);
                    buffer_last_read += read_bytes;
                    n += read_bytes;
                }
                metadata_message_buf.extend_from_slice(&tmp_buffer[old_buffer_last_read..buffer_last_read]);
            }
            let metadata = self.message_metadata(&metadata_message_buf)?;

            let mut payload_buffer = Vec::<u8>::new();
            let tmp_last_read = cmp::min(metadata.length, buffer_size - buffer_last_read);
            payload_buffer.extend_from_slice(&tmp_buffer[buffer_last_read..buffer_last_read + tmp_last_read]);
            buffer_last_read += tmp_last_read;

            while payload_buffer.len() < metadata.length {
                if buffer_size <= buffer_last_read {
                    // let input_buffer_remaining_bytes = cmp::min(METADATA_SIZE, metadata.length - payload_buffer.len());
                    {
                        let mut network_lock = network.deref().lock().await;
                        tmp_buffer = network_lock.get_message().await.unwrap_or_default();
                    }
                    n = tmp_buffer.len();
                    // n = read_lock.read(&mut tmp_buffer[..input_buffer_remaining_bytes]).await?;
                    payload_buffer.extend_from_slice(&tmp_buffer[..n]);
                } else {
                    let tmp_last_read = cmp::min(metadata.length, buffer_size - buffer_last_read);
                    payload_buffer.extend_from_slice(&tmp_buffer[buffer_last_read..tmp_last_read]);
                    buffer_last_read += tmp_last_read;
                }
            }
            parse_channel.send((metadata.message_type, payload_buffer)).await?;
        }
    }

    fn message_metadata(&self, buffer: &Vec<u8>) -> Result<MessageInfo, String> {
        if buffer.len() >= METADATA_SIZE {
            let message = BigEndian::read_u16(&buffer);
            let length = BigEndian::read_u32(&buffer[2..]) as usize;
            Ok(MessageInfo { message_type: message, length })
        } else {
            Err(format!("Message to short to be processed: {:?}", buffer))
        }
    }

    fn process_message(&self, message_type: u16, data: Vec<u8>) {
        match num::FromPrimitive::from_u16(message_type) {
            Some(NetworkMessage::UDPTunnel) => {
                println!("Missing UDP Tunnel Implementation: {:?}", data);
            }
            Some(NetworkMessage::Version) => {
                match Version::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing Version: {:?}", e)
                }
            }
            Some(NetworkMessage::CryptSetup) => {
                match CryptSetup::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing CryptSetup: {:?}", e)
                }
            }
            Some(NetworkMessage::CodecVersion) => {
                match CodecVersion::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing CodecVersion: {:?}", e)
                }
            }
            Some(NetworkMessage::ChannelState) => {
                match ChannelState::parse_from_bytes(&*data) {
                    Ok(info) => {
                        self.update_channel(info);
                    }
                    Err(e) => println!("Error while parsing ChannelState: {:?}", e)
                }
            }
            Some(NetworkMessage::PermissionQuery) => {
                match PermissionQuery::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing PermissionQuery: {:?}", e)
                }
            }
            Some(NetworkMessage::UserState) => {
                match UserState::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing UserState: {:?}", e)
                }
            }
            Some(NetworkMessage::ServerSync) => {
                match ServerSync::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing ServerSync: {:?}", e)
                }
            }
            Some(NetworkMessage::ServerConfig) => {
                match ServerConfig::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing ServerConfig: {:?}", e)
                }
            }
            Some(NetworkMessage::TextMessage) => {
                match TextMessage::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing TextMessage: {:?}", e)
                }
            }
            Some(NetworkMessage::UserRemove) => {
                match UserRemove::parse_from_bytes(&*data) {
                    Ok(info) => println!("Data: {:?}", info),
                    Err(e) => println!("Error while parsing UserRemove: {:?}", e)
                }
            }
            _ => println!("Todo: {}", message_type)
        }
    }
    fn update_channel(&self, channel_info: ChannelState) -> Option<()> {
        let channel_id = channel_info.channel_id?;
        if self.channels.contains_key(&channel_id) {
            println!("TODO: update channel: {} ({:?})", channel_id, self.channels.get_key_value(&channel_id));
        } else {
            let channel_name = channel_info.name?;

            //self.channels.insert(channel_id, MumbleChannel { name: channel_name });
        }
        Some(())
    }
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

fn write_version() -> Result<Vec<u8>, ()> {
    let version = mumble::Version {
        version: Some((1 << 16) | (6 << 8)),
        release: Some(String::from("Mumble Rust without scroll bug")),
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

fn write_auth(username: String) -> Result<Vec<u8>, ()> {
    let auth = mumble::Authenticate {
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

fn write_ping() -> Result<Vec<u8>, ()> {
    //println!("PING!");
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let ping = mumble::Ping {
        timestamp: Option::from(since_the_epoch.as_secs()),
        good: None,
        late: None,
        lost: None,
        resync: None,
        udp_packets: None,
        tcp_packets: None,
        udp_ping_avg: None,
        udp_ping_var: None,
        tcp_ping_avg: None,
        tcp_ping_var: None,
        special_fields: Default::default(),
    };

    match &ping.write_to_bytes() {
        Ok(data) => {
            Ok(serialize_message(NetworkMessage::Authenticate, data))
        }
        Err(_) => Err(())
    }
}