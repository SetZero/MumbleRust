use std::error::Error;
use std::net::ToSocketAddrs;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio_native_tls::native_tls::TlsConnector;

const BUFFER_SIZE: usize = 4096;

#[async_trait]
pub trait TCPSender {
    async fn send_message(&mut self, message: Vec<u8>) -> Result<(), Box<SendError<Vec<u8>>>>;
}

#[async_trait]
pub trait TCPReceiver
{
    async fn get_message(&mut self) -> Option<Vec<u8>>;
}

#[async_trait]
pub trait TCPClient: TCPSender + TCPReceiver {
    async fn connect(&mut self, server_host: String, server_port: u16) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub struct Network {
    writer_tx: Option<Sender<Vec<u8>>>,
    reader_rx: Option<Receiver<Vec<u8>>>,
}

#[async_trait]
impl TCPClient for Network
    where Network: TCPSender + TCPReceiver {
    async fn connect(&mut self, server_host: String, server_port: u16) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(4096);
        let (reader_tx, reader_rx) = mpsc::channel::<Vec<u8>>(4096);
        self.writer_tx = Some(writer_tx);
        self.reader_rx = Some(reader_rx);

        let server_addr = (server_host.as_ref(), server_port)
            .to_socket_addrs()
            .expect("Failed to parse server address")
            .next()
            .expect("Failed to resolve server address");

        let socket = TcpStream::connect(&server_addr).await?;
        let cx = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let cx = tokio_native_tls::TlsConnector::from(cx);

        let socket = cx.connect(&server_host, socket).await?;
        let (mut reader, mut writer) = tokio::io::split(socket);

        let mut tmp_buffer = [0u8; BUFFER_SIZE];
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    write = writer_rx.recv() => {
                        if write.is_some() {
                            let _result = writer.write(&write.unwrap()).await;
                        }
                    }
                    bytes = reader.read(&mut tmp_buffer[..]) => {
                        if bytes.is_ok() {
                            let _result = reader_tx.send(tmp_buffer[..bytes.unwrap()].to_vec()).await;
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

impl Network {
    pub fn new() -> Self {
        Network { writer_tx: None, reader_rx: None }
    }
}

#[async_trait]
impl TCPSender for Network {
    async fn send_message(&mut self, message: Vec<u8>) -> Result<(), Box<SendError<Vec<u8>>>> {
        match &self.writer_tx {
            None => {
                println!("Unable to send a message to an uninitialized queue");
            }
            Some(queue) => {
                queue.send(message).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TCPReceiver for Network {
    async fn get_message(&mut self) -> Option<Vec<u8>> {
        self.reader_rx.as_mut()?.recv().await
    }
}