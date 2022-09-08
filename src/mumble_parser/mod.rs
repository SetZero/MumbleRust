mod tests;

use std::error::Error;
use byteorder::{BigEndian, ByteOrder};
use protobuf::Message;
use tokio::io::{AsyncRead, AsyncReadExt};
use crate::mumble::mumble::{ChannelState, CodecVersion, CryptSetup, PermissionQuery, ServerConfig, ServerSync, TextMessage, UserState, Version};
use crate::NetworkMessage;

const METADATA_SIZE: usize = 6;
const BUFFER_SIZE: usize = 4096;

struct MessageInfo {
    pub message_type: u16,
    pub length: usize,
}

pub struct MumbleParser<R>
    where R: AsyncRead + Unpin {
    input: R,
}

impl<R> MumbleParser<R>
    where R: AsyncRead + Unpin {
    pub(crate) fn new(input: R) -> MumbleParser<R> {
        MumbleParser { input }
    }

    pub(crate) async fn process(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {

            let mut buffer = [0; BUFFER_SIZE];
            let mut buffer_pointer = 0;

            //FIXME: this will fail if metadata length > buffer length
            //FIXME: this will fail if metadata is cut
            let n = self.input.read(&mut buffer[..]).await?;
            while buffer_pointer < n {
                let metadata = self.message_metadata(&buffer[buffer_pointer..n])?;
                let data = &buffer[buffer_pointer + 6..buffer_pointer + 6 + metadata.length];
                self.process_message(metadata.message_type, data);
                buffer_pointer += 6 + metadata.length
            }
        }
    }

    fn message_metadata(&self, buffer: &[u8]) -> Result<MessageInfo, &'static str> {
        if buffer.len() >= 6 {
            let message = BigEndian::read_u16(&buffer);
            let length = BigEndian::read_u32(&buffer[2..]) as usize;
            Ok(MessageInfo { message_type: message, length })
        } else {
            Err("Message to short to be processed")
        }
    }

    // TODO: Remove this giant match and use some different pattern
    fn process_message(&self, message_type: u16, data: &[u8]) {
            match num::FromPrimitive::from_u16(message_type) {
                Some(NetworkMessage::Version) => {
                    match Version::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing: {:?}", e)
                    }
                }
                Some(NetworkMessage::CryptSetup) => {
                    match CryptSetup::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing CryptSetup: {:?}", e)
                    }
                }
                Some(NetworkMessage::CodecVersion) => {
                    match CodecVersion::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing CodecVersion: {:?}", e)
                    }
                }
                Some(NetworkMessage::ChannelState) => {
                    match ChannelState::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing ChannelState: {:?}", e)
                    }
                }
                Some(NetworkMessage::PermissionQuery) => {
                    match PermissionQuery::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing PermissionQuery: {:?}", e)
                    }
                }
                Some(NetworkMessage::UserState) => {
                    match UserState::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing UserState: {:?}", e)
                    }
                }
                Some(NetworkMessage::ServerSync) => {
                    match ServerSync::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing ServerSync: {:?}", e)
                    }
                }
                Some(NetworkMessage::ServerConfig) => {
                    match ServerConfig::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing ServerConfig: {:?}", e)
                    }
                }
                Some(NetworkMessage::TextMessage) => {
                    match TextMessage::parse_from_bytes(data) {
                        Ok(info) => println!("Data: {:?}", info),
                        Err(e) => println!("Error while parsing TextMessage: {:?}", e)
                    }
                }
                _ => println!("Todo: {}", message_type)
            }
    }
}