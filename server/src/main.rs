use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{client, Message, Result as TungResult};
// type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
use uuid::Uuid;

type SendSyncError = Box<dyn std::error::Error + Send + Sync + 'static>;

use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use serde_json::Result as SerdeResult;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

// The shared library between server and client
use shared::{GameClient, PositionEvent};

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

    // Closure that removes the client
    let close_client = || -> Result<GameClient, &str> {
        let mut state = state.lock().expect("Couldn't acquire state lock!");
        return state.clients.remove(&client_id).ok_or("Client not found");
    };

    // Parse received message
    let (mut write, mut read) = ws_stream.split();
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(msg) => {
                if let Ok(pos) = serde_json::from_str::<PositionEvent>(&msg) {
                    let mut state = state.lock().expect("Couldn't acquire state lock!");
                    let client = state
                        .clients
                        .get_mut(&client_id)
                        .expect("Couldn't find previously created client");

                    // Set position
                    client.position = [pos.x, pos.y];
                }

                // TODO: Move this someplace else
                // Now we're going to respond with serialized game state
                let msg = {
                    let state = state.lock().expect("Couldn't acquire state lock!");

                    // Only send other clients
                    let clients =
                        Vec::from_iter(state.clients.values().filter(|x| x.uuid != client_id));

                    // Return serialized string
                    serde_json::to_string(&clients).expect("asd")
                };

                write.send(Message::text(msg)).await?;
            }
            Message::Close(_) => {
                if close_client().is_err() {
                    panic!("Couldn't remove client!")
                }
            }
            _ => {}
        }
    }

    // Unsure if this will be reached, ignore failure
    let _ = close_client();

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
