use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Result as TungResult;
// type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

type SendSyncError = Box<dyn std::error::Error + Send + Sync + 'static>;

use rand::distributions::{Alphanumeric, DistString};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct GameClient {
    uuid: String,
    position: [u32; 2],
}

#[derive(Clone, Debug)]
struct GameState {
    clients: Vec<GameClient>,
}
impl GameState {
    fn new() -> GameState {
        return GameState { clients: vec![] };
    }
}

type SharedGameState = Arc<Mutex<GameState>>;

async fn handle_connection(stream: TcpStream, state: SharedGameState) -> TungResult<()> {
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    println!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    // Acquire lock and push new client
    {
        let mut gamestate = state.lock().expect("something bad");
        gamestate.clients.push(GameClient {
            uuid: Alphanumeric.sample_string(&mut rand::thread_rng(), 8),
            position: [0, 0],
        });
    }

    let (mut write, mut read) = ws_stream.split();
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            // println!("{:?}", msg);
            write.send(msg).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), SendSyncError> {
    let addr: std::net::SocketAddr = "[::1]:3000".parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Listening on http://{addr}");

    let state = Arc::new(Mutex::new(GameState::new()));

    // Spawn printing service
    // let readonlyclone = state.clone();
    // use std::{thread, time};
    // tokio::spawn(async move {
    //     loop {
    //         thread::sleep(time::Duration::from_secs(1));
    //         println!("{:?}", readonlyclone.lock().unwrap().clients);
    //     }
    // });

    // Accept connections
    loop {
        let (stream, _) = listener.accept().await?;
        println!("Accepted");

        let state = state.clone();
        tokio::spawn(handle_connection(stream, state));
    }
}
