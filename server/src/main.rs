use futures::sink::SinkExt;
use futures::stream::StreamExt;
use http_body_util::Full;
use hyper::body::{Bytes as HyperBytes, Incoming};
use hyper::service::Service;
use hyper::{body::Incoming as IncomingBody, Request, Response};
use hyper_tungstenite::{tungstenite, HyperWebsocket};
use hyper_util::rt::TokioIo;
use tungstenite::Message;
// type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

type SendSyncError = Box<dyn std::error::Error + Send + Sync + 'static>;

use bytes::Bytes;
use rand::distributions::{Alphanumeric, DistString};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::future::Future;
use std::pin::Pin;

// type GameServiceState = Arc<Mutex<HashMap<String, String>>>;
type GameServiceState = Arc<Mutex<u128>>;

#[derive(Clone)]
struct GameService {
    state: GameServiceState,
    name: String,
}
// impl GameService {
//     async fn serve_websocket(&self, websocket: HyperWebsocket) -> Result<(), SendSyncError> {
//         let mut websocket = websocket.await?;
//         while let Some(message) = websocket.next().await {
//             match message? {
//                 Message::Text(msg) => {
//                     println!("Received text message: {msg}");
//                     websocket
//                         .send(Message::text("Thank you, come again."))
//                         .await?;
//                 }
//                 Message::Binary(msg) => {
//                     println!("Received binary message: {msg:02X?}");
//                     websocket
//                         .send(Message::binary(b"Thank you, come again.".to_vec()))
//                         .await?;
//                 }
//                 Message::Ping(msg) => {
//                     // No need to send a reply: tungstenite takes care of this for you.
//                     println!("Received ping message: {msg:02X?}");
//                 }
//                 Message::Pong(msg) => {
//                     println!("Received pong message: {msg:02X?}");
//                 }
//                 Message::Close(msg) => {
//                     // No need to send a reply: tungstenite takes care of this for you.
//                     if let Some(msg) = &msg {
//                         println!(
//                             "Received close message with code {} and message: {}",
//                             msg.code, msg.reason
//                         );
//                     } else {
//                         println!("Received close message");
//                     }
//                 }
//                 Message::Frame(_msg) => {
//                     unreachable!();
//                 }
//             }
//         }

//         Ok(())
//     }
// }
impl Service<Request<IncomingBody>> for GameService {
    type Response = Response<Full<HyperBytes>>;
    type Error = hyper::Error;
    // type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn call(&self, mut request: Request<IncomingBody>) -> Self::Future {
        // Check if the request is a websocket upgrade request.
        if hyper_tungstenite::is_upgrade_request(&request) {
            let Ok((response, websocket)) = hyper_tungstenite::upgrade(&mut request, None) else {
                eprintln!("Error upgrading connection!");
                return Box::pin(async {
                    Ok(
                        Response::builder()
                            .status(400)
                            .body(Full::<HyperBytes>::from("Couldn't upgrade connection to websocket"))
                            .unwrap()
                    )
                });
            };

            // let mut state = self.state.lock().expect("lock poisoned");
            // (*state).insert(String::from("asda"), String::from("asda"));

            // Spawn a task to handle the websocket connection.
            let state = Arc::clone(&self.state);
            let name = self.name.clone();
            tokio::spawn(async move {
                if let Err(e) = serve_websocket(state, name, websocket).await {
                    eprintln!("Error in websocket connection: {e}");
                }
            });

            // Return the response so the spawned future can continue.
            return Box::pin(async { Ok(response) });
        }

        // Handle regular HTTP requests here.
        Box::pin(async { Ok(Response::new(Full::<HyperBytes>::from("Hello HTTP!"))) })
    }
}

/// Handle a websocket connection.
async fn serve_websocket(
    state: GameServiceState,
    name: String,
    websocket: HyperWebsocket,
) -> Result<(), SendSyncError> {
    let mut websocket = websocket.await?;
    while let Some(message) = websocket.next().await {
        match message? {
            Message::Text(msg) => {
                println!("Received text message: {msg}");

                // use scope so that lock is not held before await
                {
                    let mut state = state.lock().expect("lock poisoned");
                    *state += 1;
                    println!("COUNTER IS AT {:?}. Name: {:?}", *state, name);
                }

                websocket
                    .send(Message::text("Thank you, come again."))
                    .await?;
            }
            Message::Binary(msg) => {
                println!("Received binary message: {msg:02X?}");
                websocket
                    .send(Message::binary(b"Thank you, come again.".to_vec()))
                    .await?;
            }
            Message::Ping(msg) => {
                // No need to send a reply: tungstenite takes care of this for you.
                println!("Received ping message: {msg:02X?}");
            }
            Message::Pong(msg) => {
                println!("Received pong message: {msg:02X?}");
            }
            Message::Close(msg) => {
                // No need to send a reply: tungstenite takes care of this for you.
                if let Some(msg) = &msg {
                    println!(
                        "Received close message with code {} and message: {}",
                        msg.code, msg.reason
                    );
                } else {
                    println!("Received close message");
                }
            }
            Message::Frame(_msg) => {
                unreachable!();
            }
        }
    }

    Ok(())
}

// /// Handle a HTTP or WebSocket request.
// async fn handle_request(
//     mut request: Request<Incoming>,
// ) -> Result<Response<Full<HyperBytes>>, Error> {
//     // Check if the request is a websocket upgrade request.
//     if hyper_tungstenite::is_upgrade_request(&request) {
//         let (response, websocket) = hyper_tungstenite::upgrade(&mut request, None)?;

//         // Spawn a task to handle the websocket connection.
//         tokio::spawn(async move {
//             if let Err(e) = serve_websocket(websocket).await {
//                 eprintln!("Error in websocket connection: {e}");
//             }
//         });

//         // Return the response so the spawned future can continue.
//         return Ok(response);
//     }

//     // Handle regular HTTP requests here.
//     Ok(Response::new(Full::<HyperBytes>::from("Hello HTTP!")))
// }

#[tokio::main]
async fn main() -> Result<(), SendSyncError> {
    let addr: std::net::SocketAddr = "[::1]:3000".parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Listening on http://{addr}");

    let mut http = hyper::server::conn::http1::Builder::new();
    http.keep_alive(true);

    let state = Arc::new(Mutex::new(0));
    loop {
        let (stream, _) = listener.accept().await?;
        let connection = http
            .serve_connection(
                TokioIo::new(stream),
                GameService {
                    state: state.clone(),
                    name: Alphanumeric.sample_string(&mut rand::thread_rng(), 16),
                },
            )
            .with_upgrades();
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                println!("Error serving HTTP connection: {err:?}");
            }
        });
    }
}
