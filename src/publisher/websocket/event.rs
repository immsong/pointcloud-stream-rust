#[derive(Debug)]
pub enum WebsocketEvent {
    Connected {
        addr: std::net::SocketAddr,
    },
    Disconnected {
        addr: std::net::SocketAddr,
    },
    Message {
        addr: std::net::SocketAddr,
        message: tokio_tungstenite::tungstenite::Message,
    },
}
