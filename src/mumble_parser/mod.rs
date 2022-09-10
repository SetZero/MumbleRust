mod tests;

use std::cmp;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use byteorder::{BigEndian, ByteOrder};
use protobuf::Message;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use crate::mumble::mumble::{ChannelState, CodecVersion, CryptSetup, PermissionQuery, ServerConfig, ServerSync, TextMessage, UserState, Version};
use crate::{NetworkMessage};
use crate::mumble::mumble;

const METADATA_SIZE: usize = 6;
const BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
struct MessageInfo {
    pub message_type: u16,
    pub length: usize,
}

pub struct MumbleParser<R>
    where R: AsyncRead + AsyncWrite + Unpin {
    input: R,
    pub user_name: String,
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

fn write_version() -> Result<impl AsRef<[u8]>, ()> {
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

fn write_auth(username: String) -> Result<impl AsRef<[u8]>, ()> {
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

fn write_ping() -> Result<impl AsRef<[u8]>, ()> {
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

impl<R> MumbleParser<R>
    where R: AsyncRead + AsyncWrite + Unpin {
    pub(crate) fn new(input: R, user_name: String) -> MumbleParser<R> {
        MumbleParser { input, user_name }
    }

    pub(crate) async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.input.write(write_version().unwrap().as_ref()).await?;
        self.input.write(write_auth(self.user_name.clone()).unwrap().as_ref()).await?;

        let mut tmp_buffer = [0; BUFFER_SIZE];

        let mut buffer_size = 0;
        let mut buffer_last_read = 0;
        loop {
            let mut n = 0;
            let mut metadata_message_buf = Vec::<u8>::new();
            while n < METADATA_SIZE {
                let old_buffer_last_read;
                if buffer_size <= buffer_last_read {
                    buffer_last_read = 0;
                    buffer_size = 0;
                    tmp_buffer = [0; BUFFER_SIZE];

                    n += self.input.read(&mut tmp_buffer[..]).await?;
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
                    let input_buffer_remaining_bytes = cmp::min(METADATA_SIZE, metadata.length - payload_buffer.len());
                    n = self.input.read(&mut tmp_buffer[..input_buffer_remaining_bytes]).await?;
                    payload_buffer.extend_from_slice(&tmp_buffer[..n]);
                } else {
                    let mut tmp_last_read = cmp::min(metadata.length, buffer_size - buffer_last_read);
                    payload_buffer.extend_from_slice(&tmp_buffer[buffer_last_read..tmp_last_read]);
                    buffer_last_read += tmp_last_read;
                }
            }
            self.process_message(metadata.message_type, payload_buffer);
            //TODO: Move to timer thread
            self.input.write(write_ping().unwrap().as_ref()).await?;
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

    // TODO: Remove this giant match and use some different pattern
    fn process_message(&self, message_type: u16, data: Vec<u8>) {
            match num::FromPrimitive::from_u16(message_type) {
                Some(NetworkMessage::Version) => {
                    match Version::parse_from_bytes(&*data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing: {:?}", e)
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
                        Ok(info) => println!("Data: {:?}", info),
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
                    match TextMessage::parse_from_bytes(&*data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing UserRemove: {:?}", e)
                    }
                }
                _ => println!("Todo: {}", message_type)
            }
    }
}