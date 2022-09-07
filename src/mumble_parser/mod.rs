mod tests;

use std::error::Error;
use byteorder::{BigEndian, ByteOrder};
use tokio::io::{AsyncRead, AsyncReadExt};
use crate::mumble::mumble::Version;

const METADATA_SIZE: usize = 6;

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

            let mut buffer = [0; 4096];

            let n = self.input.read(&mut buffer[..]).await?;

            let metadata = self.message_metadata(&buffer[..n]);
            /*match metadata {
                Some(metadata) => &buffer[6..6 + metadata.length],
                None => { /* metadata has been cut, wait for next message */ }
            }*/
        }
    }

    fn message_metadata(&self, buffer: &[u8]) -> Option<MessageInfo> {
        if buffer.len() >= 6 {
            let message = BigEndian::read_u16(&buffer);
            let length = BigEndian::read_u32(&buffer[2..]) as usize;
            Some(MessageInfo { message_type: message, length })
        } else {
            None
        }
    }

    /*fn process_message(&self, data: &[u8]) {
        let message_info = self.deserialize_message(data);
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
    }*/
}