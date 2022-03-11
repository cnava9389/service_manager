use std::sync::Arc;
use super::{json};
use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;
use std::io::{self, BufRead, prelude::*};
use futures::executor::block_on;
use std::net::TcpStream;
use futures::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{WebSocket, Message};
use warp::Filter;
use std::net::SocketAddr;
use uuid::Uuid;

mod helper;
use helper::{Store, EventQueue, Manager, Sender};



type Res<T> = Result<T, Box<dyn std::error::Error>>;


#[derive(Debug)]
/// The service manager recieves and logs any operation and forwards it to the appropriate manager to deal with
struct ServiceManager {
    store: RwLock<Store>,
    event_queue: RwLock<EventQueue>
}

impl ServiceManager {

    // ---------------------------------------------------------------- no self

    /// returns an arc service manager for multiple thread support
    fn new() -> Arc<ServiceManager> {
        Arc::new(ServiceManager{
            store: RwLock::new(Store::new()),
            event_queue: RwLock::new(EventQueue::new()),
        })
    }

    /// starts a webserver with a websocket to connect to clients and logs any operation and forwards it to the appropriate manager.
    async fn start_server() -> Res<()> {
        dotenv::dotenv().ok();

        let socket_addr: SocketAddr = "127.0.0.1:9000".parse().expect("valid socket address");

        // service manager for http and sockets
        let manager = ServiceManager::new();
        let clone = manager.clone();
        let manager_filter = warp::any().map(move || manager.clone());
        
        let chat = warp::path("ws")
            .and(warp::ws())
            // .and(users)
            .and(manager_filter)
            .map(|ws: warp::ws::Ws, ws_manager| ws.on_upgrade(move |socket| connect(socket, ws_manager)));
    
        // not found page
        let res_404 = warp::any().map(|| {
            warp::http::Response::builder()
                .status(warp::http::StatusCode::NOT_FOUND)
                .body("404 Not Found!")
        });
    
        // add all routes together. can modularize even further
        let routes = chat.or(res_404);//.or(hello)
        
        
        let test_env = dotenv::var("TEST").unwrap_or_else(|_|"false".to_string());
        
        tokio::task::spawn_blocking(move || {
            let _ = block_on(ServiceManager::start_tcp_server(clone,"7878"));
        });
        
        if test_env == "true" {
            let server = warp::serve(routes).try_bind(socket_addr);
            println!("Running web server on 127.0.0.1:9000!");
            server.await
        }else {
            println!("Running server on 433!");
            warp::serve(routes).tls().cert_path("./fullchain.pem").key_path("./privkey.pem").run(([127,0,0,1],433)).await;
        }
        Ok(())
    }

    
    /// starts the tcp server that communicates with the service managers on :7878
    async fn start_tcp_server(manager: Arc<dyn Manager>, port: &str) {
        let listener = TcpListener::bind(format!("127.0.0.1:{}",port)).await;
        match listener {
            Ok(listener) => {
                println!("starting tcp server on 127.0.0.1:{}!",port);
                loop {
                    let (mut socket, _addr) = listener.accept().await.unwrap();
                    let manager = manager.clone();
                    println!("connection made");
                    // A new task is spawned for each inbound socket. The socket is
                    // moved to the new task and processed there.
                    tokio::spawn(async move {
                        let _manager = manager;
                        let mut buf = [0; 1024];
                        
                        // In a loop, read data from the socket and write the data back.
                        loop {
                            let n = match socket.read(&mut buf).await {
                                // socket closed
                                Ok(n) if n == 0 => return,
                                Ok(n) => n,
                                Err(e) => {
                                    eprintln!("failed to read from socket; err = {:?}", e);
                                    return;
                                }
                            };
        
                            // ! socket sends info for tcp
                            // Write the data back
                            if let Err(e) = socket.write_all(&buf[0..n]).await {
                                eprintln!("failed to write to socket; err = {:?}", e);
                                return;
                            }
                        }
                    });
                }
            },
            // ! log
            Err(_) => println!("Error starting new Service Manager or one is already running on 127.0.0.1:7878"),
        }
        
    }

}

impl Manager for ServiceManager {
    fn test(&self) {
        println!("Service manager test");
    }

    fn process_message<'a>(&self, message: &'a str, sender: Sender) -> Res<&'a str> {
        Ok(message)
    }

    fn send(&self) -> Res<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------- private

/// the function that handles the connection to the websocket
async fn connect(ws: WebSocket, server: Arc<ServiceManager>) {

    // Establishing a connection
    let (user_tx, mut user_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel();
    
    let rx = UnboundedReceiverStream::new(rx);
    
    tokio::spawn(rx.forward(user_tx));

    println!("Connected");
    
    while let Some(result) = user_rx.next().await {
        if result.is_ok() {
            let result = result.unwrap();
            
            let result = result.to_str();
            if result.is_ok() {
                let result = result.unwrap();
                //  !  tx sends message back for ws
                //let message = server.process_message(result, tx);
                //server.send(message);
                tx.send(Ok(Message::text(format!("hello from service manager tcp server your message: {}", result)))).expect("Failed to send message");
            }else{
                break;
            }

        }else{
            tx.send(Ok(Message::text("Error reading what was sent to tcp server"))).expect("Failed to send message");
        }
        // broadcast_msg(result.expect("Failed to fetch message")).await;
    }
    // disconnected
    
}


/// function that takes the port number as a string and connects to that tcp server for testing a single message
fn test_server(port: &str) -> Res<()> {
    if port.chars().all(char::is_numeric){
        match TcpStream::connect(format!("127.0.0.1:{}",port)){
            Ok(mut stream) => {

                let mut buffer = [0; 1024];
                
                stream.write_all(b"hello")?;

                let _ = stream.read(&mut buffer[..])?;

                match String::from_utf8(buffer.into()) {
                    Ok(v) => println!("{}",v),
                    Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                };
                
            },
            Err(_) => println!("Error connecting to server for testing"),
        };
    }else{
        println!("Error not a port number");
    }
    Ok(())
}

//---------------------------------------------------------------- public

/// function that starts a service if you run program with appropriate manager argument
pub async fn select_start(arg: &str) {
    match arg {
        "service" => {
            let _ = ServiceManager::start_server().await;
        },
        x => println!("Error unkown input: {}", x)
    }
}

/// function executed to begin the cli
pub fn start() {
    println!("Which service would you like to start
    sm for service manager
    db for database manager
    ca for cache manager
    ss for session manager
    test to test a local netork tcp server
    q or quit to exit");

    let mut processes: Vec<JoinHandle<()>> = Vec::new();
    let mut online_sm = false;
    let stdin = io::stdin();
    let mut iterator = stdin.lock().lines();

    while let Some(line) = iterator.next() {

       let result = match line {
        Ok(x) => x,
        Err(_) => "error".to_owned(),
        };

        match result.to_lowercase().as_str() {
            "error" => println!("Error reading input"),
            "quit" | "q" => {
                for process in processes {
                    process.abort();
                }
                break
            },
            "sm" => {
                if !online_sm {
                    let x = tokio::task::spawn_blocking(|| {
                        let _ = block_on(ServiceManager::start_server());
                    });
                    processes.push(x);
                    online_sm = true;
                }
            },
            "test" => {
                println!("what port number?");
                if let Some(input) = iterator.next() { 
                    match input {
                        Ok(x) => test_server(&x).unwrap(),
                        Err(_) => println!("Error reading port for test"),
                    }
                 }
            }
            _ => println!("Error unkown input")
        }
    }
}
