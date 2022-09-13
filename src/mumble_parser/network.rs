use std::error::Error;
use std::future::Future;
use std::net::ToSocketAddrs;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_native_tls::native_tls::TlsConnector;

const BUFFER_SIZE: usize = 4096;

trait TCPSender {
    fn send_message(&mut self);
}

trait TCPReceiver<T>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static
{
    fn listen_message(&mut self, future: T);
}

pub struct Network {}

impl Network {
    pub fn new() -> Self {
        Network {}
    }

    pub async fn connect(&mut self, server_host: String, server_port: u16, user_name: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(4096);
        let (reader_tx, mut reader_rx) = mpsc::channel::<Vec<u8>>(4096);

        tokio::spawn(async move {
            let data = reader_rx.recv().await;
            println!("Data: {:?}", data);
        });

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
        loop {
            tokio::select! {
                write = writer_rx.recv() => {
                    if write.is_some() {
                        writer.write(&write.unwrap());
                    }
                }
                bytes = reader.read(&mut tmp_buffer[..]) => {
                    if bytes.is_ok() {
                        reader_tx.send(tmp_buffer[..bytes.unwrap()].to_vec()).await;
                    }
                }
            }
        }
    }
}

impl TCPSender for Network {
    fn send_message(&mut self) {}
}

impl<T: std::future::Future> TCPReceiver<T> for Network
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static {
    fn listen_message(&mut self, future: T) {}
}