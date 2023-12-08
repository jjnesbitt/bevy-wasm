use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{client, Result as TungResult};
// type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
use uuid::Uuid;

type SendSyncError = Box<dyn std::error::Error + Send + Sync + 'static>;

use rand::distributions::{Alphanumeric, DistString};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use shared::PositionEvent;

#[derive(Clone, Debug)]
struct GameClient {
    uuid: Uuid,
    position: [f32; 2],
}

#[derive(Clone, Debug)]
struct GameState {
    clients: HashMap<Uuid, GameClient>,
}
impl GameState {
    fn new() -> GameState {
        return GameState {
            clients: HashMap::new(),
        };
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

    // Use scope to avoid holding onto mutex lock
    let client_id = Uuid::new_v4();
    {
        let mut state = state.lock().expect("Couldn't acquire state lock!");
        state.clients.insert(
            client_id.clone(),
            GameClient {
                uuid: client_id.clone(),
                position: [0.0, 0.0],
            },
        );
    }

    // Parse received message
    let (_, mut read) = ws_stream.split();
    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let Ok(msg) = msg.to_text() {
            if let Ok(pos) = serde_json::from_str::<PositionEvent>(msg) {
                let mut state = state.lock().expect("Couldn't acquire state lock!");
                let client = state
                    .clients
                    .get_mut(&client_id)
                    .expect("Couldn't find previously created client");

                // Set position
                client.position = [pos.x, pos.y];
            }
        }
    }

    // This means the web socket is closed, and so we'll remove the client from the game state
    {
        let mut state = state.lock().expect("Couldn't acquire state lock!");
        state
            .clients
            .remove(&client_id)
            .expect("Couldn't remove client from game state!");
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
    let readonlyclone = state.clone();
    use std::{thread, time};
    tokio::spawn(async move {
        loop {
            thread::sleep(time::Duration::from_secs(1));
            println!("{:?}", readonlyclone.lock().unwrap().clients);
        }
    });

    // Accept connections
    loop {
        let (stream, _) = listener.accept().await?;
        println!("Accepted");

        let state = state.clone();
        tokio::spawn(handle_connection(stream, state));
    }
}
