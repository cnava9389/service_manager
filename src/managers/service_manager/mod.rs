use std::fmt;
use std::str::Split;
use std::sync::Arc;
use super::{json, error, database_manager::DataBaseManager};
use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc};
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

pub mod helper;
use helper::{Store, EventQueue, Manager, Sender, TCPServers, login_func};

pub use helper as other;



type Res<T> = Result<T, error::ServerError>;


#[derive(Debug)]
/// The service manager recieves and logs any operation and forwards it to the appropriate manager to deal with
pub struct ServiceManager<'a> {
    store: Store,
    servers: TCPServers<'a>,
    event_queue: EventQueue
}

impl<'a> ServiceManager<'a> {
    // ---------------------------------------------------------------- self
    async fn send_to_server(&self, server_key:&str, message:&str) -> Res<String> {
        
        match self.servers.send_to_server(server_key,message).await {
            Ok(x)=> {
                Ok(x)
            },
            Err(e) => Err(e)
        }
    }

    async fn new_server(&self, k:&'a str, v: TcpStream) -> Res<()> {
        let servers = &self.servers; 
        if !k.is_empty() {
            if !servers.contains_key(k).await {
            match servers.insert(k,v).await {
                Ok(_) => Ok(()),
                Err(e) => Err(e)
            }
            }else {
                Err(error::ServerError::TAKEN)
            }
        }else{Err(error::ServerError::INVALID_ARG)}
        
    }

    // async fn remove_server(&self, k:&'a str) -> Res<()> {
    //     self.servers.remove(k).await
    // }

    async fn contains_s_p(&self,k:&str,v:&str) -> Res<()> {
        self.servers.contains(k, v).await
    }

    // ---------------------------------------------------------------- no self

    /// returns an arc service manager for multiple thread support
    fn new() -> Arc<ServiceManager<'a>> {
        Arc::new(ServiceManager{
            store:Store::new(),
            servers:TCPServers::new(),
            event_queue:EventQueue::new(),
        })
    }

    /// starts a webserver with a websocket to connect to clients and logs any operation and forwards it to the appropriate manager.
    async fn start_server(manager:Arc<ServiceManager<'static>>) -> Res<()> {
        dotenv::dotenv().ok();

        let socket_addr:SocketAddr;
        match std::env::args().nth(2) {
            Some(server)=> {
                socket_addr = server.parse().expect("valid socket address");
            },
            None => {
                socket_addr = "127.0.0.1:9000".parse().expect("valid socket address");
            }
        };

        // service manager for http and sockets
        let clone = manager.clone();
        let manager_filter = warp::any().map(move || clone.clone());

        let cors = warp::cors()
        .allow_origins(vec!["https://127.0.0.1:3000","http://127.0.0.1:3000","https://localhost:3000","http://localhost:3000"])
        .allow_methods(vec!["GET","POST","DELETE","PUT","OPTIONS","HEAD"])
        .allow_headers(vec!["User-Agent", "Sec-Fetch-Mode", "Referer", "Origin", "Access-Control-Request-Method", "Access-Control-Request-Headers","Content-Type",
        "Authorization","X-Request-With"]);
        
        let ws = warp::path("ws")
            .and(warp::path::end())
            .and(warp::ws())
            // .and(users)
            .and(manager_filter.clone())
            .map(|ws: warp::ws::Ws, ws_manager| ws.on_upgrade(move |socket| connect(socket, ws_manager)));
            // .with(cors);
    
        // not found page
        let res_404 = warp::any().map(|| {
            warp::http::Response::builder()
                .status(warp::http::StatusCode::NOT_FOUND)
                .body("404 Not Found!")
        });

        let login = warp::path("login")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(manager_filter.clone())
        .and_then(login_func);
        
        // .map(|| "from login");
    
        // add all routes together. can modularize even further
        let routes = ws.or(login).or(res_404).with(cors);
        
        
        let test_env = dotenv::var("TEST").unwrap_or_else(|_|"false".to_string());
        
        // let clone = manager.clone();
        // tokio::task::spawn_blocking(move || {
            //     let _ = block_on(ServiceManager::start_tcp_server(clone,"7878"));
            // });
            
            
            if test_env == "true" {
                let server = warp::serve(routes).try_bind(socket_addr);
                println!("Running web server on {}!",socket_addr);
                server.await
            }else {
                println!("Running server on https://127.0.0.1:5000!");
                let CERT= dotenv::var("CERT").unwrap_or_else(|_|"NA".to_string());
                let KEY = dotenv::var("KEY").unwrap_or_else(|_|"NA".to_string());
            warp::serve(routes).tls().cert_path(CERT).key_path(KEY).run(([127,0,0,1],5000)).await;
        }
        Ok(())
    }

    // ! need to be able to take any impl manager
    /// starts the tcp server that communicates with the service managers on :7878
    async fn start_tcp_server(manager: Arc<dyn Manager>, port: &str) {
        let addr;
        if port.is_empty() {
            match std::env::args().nth(2) {
                Some(address) => {
                    addr = address;
                },
                None =>  {
                    addr = String::from("127.0.0.1:5000");
                },
            }
        } else {
            addr = format!("127.0.0.1:{}",port);
        }
        let listener = TcpListener::bind(&addr).await;
        match listener {
            Ok(listener) => {
                println!("starting tcp server on {}!",&addr);
                loop {
                    // let (mut socket, _addr) = listener.accept().await.unwrap();
                    match listener.accept().await {
                        Ok((mut socket, _addr)) => {
                            let manager = manager.clone();
                            let id = Uuid::new_v4();
                            let id = Uuid::to_string(&id);
                        // A new task is spawned for each inbound socket. The socket is
                        // moved to the new task and processed there.
                        tokio::spawn(async move {
                            let manager = manager;
                            let mut buf = [0; 1024];
                            
                            // In a loop, read data from the socket and write the data back.
                            loop {
                                let n = match socket.read(&mut buf).await {
                                    // socket closed
                                    Ok(n) if n == 0 => return,
                                    Ok(n) => n,
                                    Err(_) => {
                                        println!("failed to read from connection or remote host disconnected");
                                        return
                                    }
                                };

                                
                                match std::str::from_utf8(&buf[..n]) {
                                    Ok(x) => {
                                        // process and get data
                                        match manager.process_message(x, &id) {
                                            Ok(x) => {
                                                match manager.send(&x, Sender::TCP(&mut socket)) {
                                                    Ok(_) => (),
                                                    Err(e) => {
                                                        let _ = manager.send(e.produce_error(),Sender::TCP(&mut socket));
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                let _ = manager.send(e.produce_error(),Sender::TCP(&mut socket));
                                            },
                                        }
                                    },
                                    Err(_) => {
                                        let _ = manager.send("server error",Sender::TCP(&mut socket));
                                    },
                                }

                            }
                    });
                        },
                        Err(_) => println!("could not make connection"),
                    }
                }
            },
            // ! log
            Err(_) => println!("Error starting new Service Manager or one is already running on 127.0.0.1:7878"),
        }
        
    }

    fn add_cmd(&self,cmd:&str, mut result:Split<&str>) -> Res<String> {
        match cmd {
            x => {
                     let result_3 = result.next().unwrap_or_else(|| "");
                     match block_on(self.contains_s_p(x, result_3)) {
                         Ok(_) => {
                             let addr;
                             let result_3 = result_3.to_owned();
                             if result_3.contains(":") {
                                 addr = result_3;
                             }else {
                                 addr = format!("127.0.0.1:{}",result_3);
                             }
                             match TcpStream::connect(&addr){
                                 Ok(stream) => {
                                     let static_name = |x:&str| {
                                         match x {
                                             "TCP" => "TCP",
                                             "DB" => "DB",
                                             _ => "",
                                         }
                                     };
                                     let name = static_name(x);
                                     let _ = block_on(self.servers.insert_port(name, addr));
                                     match block_on(self.new_server(name, stream)) {
                                         Ok(_) => {
                                             let user = dotenv::var("USER").unwrap_or_else(|_|"false".to_string());
                                             let password= dotenv::var("PASSWORD").unwrap_or_else(|_|"false".to_string());
                                             let cmd = format!("NEW {} {}",user, password);
                                             match block_on(self.send_to_server(name, &cmd)){
                                                 Ok(x) => {
                                                     if x.contains("Error") || x.contains("error") {
                                                         let _ = block_on(self.servers.remove_server(name));
                                                         Err(error::ServerError::INCOMPLETE_OPERATION)
                                                     }else { 
                                                         Ok(x)
                                                     }
                                                 },
                                                 Err(_) => {
                                                     let _ = block_on(self.servers.remove_server(name));
                                                     Err(error::ServerError::CONNECTION)
                                                 }
                                             }
                                         }
                                         Err(e) => {
                                             Err(e)
                                         }
                                     }
                                 },
                                 Err(_) => {
                                     Err(error::ServerError::CONNECTION)
                                 },
                             }
                         },
                         Err(e) => {
                             Err(e)
                         }
                     }
             }
             // _ => Ok(format!("Invalid argument: {}",error::ServerError::produce_error(&error::ServerError::INVALID_ARG)))
         }
    }

}

impl fmt::Display for ServiceManager<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result{
        write!(f,"Service Manager: store: ({}), queue: ({}), servers: ({})",self.store,self.event_queue,self.servers)
    }
}

impl Manager for ServiceManager<'_> {
    // this needs to send message to tcp servers via commands and then return the data
    fn process_message(&self, message: &str, _id: &str) -> Res<String> {

        let processed_message = json::to_value_from_str(message);
        let processed_message = match processed_message {
            Ok(x) => x,
            Err(_) => serde_json::Value::Null,
        };

        
        match processed_message {
            serde_json::Value::Null => Err(error::ServerError::INVALID_JSON),
            serde_json::Value::Bool(_) => Err(error::ServerError::INVALID_JSON),
            serde_json::Value::Number(_) => Err(error::ServerError::INVALID_JSON),
            serde_json::Value::String(_) => Err(error::ServerError::INVALID_JSON),
            serde_json::Value::Array(_) => Err(error::ServerError::INVALID_JSON),
            serde_json::Value::Object(x_command) => {
                // ! use command format
                let command = x_command.get("command");
                match command {
                    Some(x) => {
                        let x = x.as_str();
                        match x {
                            Some(x) => {
                                // ! maybe something faser than a vec?
                                let mut result = x.split(" ");
                                let result_1 = result.next().unwrap();
                                let result_2 = result.next().unwrap_or_else(|| "");
                                match result_1 {
                                    "ADD" => {
                                        self.add_cmd(result_2, result)
                                    },
                                    "MSG" => {
                                        let message = x_command.get("message");
                                        match message {
                                            Some(msg) => {
                                                let msg = msg.as_str().unwrap_or_else(|| "Error");
                                                match block_on(self.send_to_server(result_2, msg)) {
                                                    Ok(x) => Ok(x),
                                                    Err(e) => Err(e),
                                                } 
                                            },
                                            None => {
                                                Err(error::ServerError::INVALID_JSON)
                                            }
                                        }
                                    },
                                    "DEL" => {
                                        match result_2 {
                                            x => {
                                                let _ = block_on(self.servers.remove_server(x));
                                                Ok(String::from("Deleted server"))
                                            }
                                        }
                                    },
                                    "TEST" => {
                                        Ok(format!("{:?}",self))
                                    },
                                    _ => {
                                        let example = json::to_json!({"command":"command_for_service_manager server_name optional_argument","message":"server_command server_argument optional_argument"});
                                        let example = example.to_string();
                                        Ok(String::from("Use this format for message\n") + &example)
                                    }
                                }
                            },
                            None => Err(error::ServerError::INVALID_JSON),
                        }
                    },
                    None => Err(error::ServerError::INVALID_JSON),
                }
            },
        }
    }

    // either sends data to user or tcp servers
    fn send(&self, message: &str, sender: Sender) -> Res<()> {
        
        match sender {
            // this writes back data from tcp servers
            Sender::TCP(x) => {
                match block_on(x.write_all(message.as_bytes())) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(error::ServerError::NO_VALUE),
                }
            },
            // this writes back to user form webserver
            Sender::WS(x) => {
                x.send(Ok(Message::text(message))).expect("Failed to send message");
                Ok(())

            },
        }
        
    }
}

// ---------------------------------------------------------------- private

/// the function that handles the connection to the websocket
async fn connect(ws: WebSocket, server: Arc<ServiceManager<'_>>) {

    // Establishing a connection
    let (user_tx, mut user_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel();

    let id = Uuid::new_v4();
    let id = Uuid::to_string(&id);
    
    let rx = UnboundedReceiverStream::new(rx);
    
    tokio::spawn(rx.forward(user_tx));
    
    while let Some(result) = user_rx.next().await {
        match result {
            Ok(x) => {
                match x.to_str() {
                    Ok(result) => {
                        // processing message and return data
                        match server.process_message(result, &id) {
                            Ok(x) => {
                                // service manager sends to websocekt user
                                match server.send(&x, Sender::WS(&tx)) {
                                    Ok(_) => (),
                                    Err(err) => println!("error sending message: {}", err),
                                }
                            },
                            Err(e) => {
                                let e = e.produce_error();
                                let error = format!("error processing your message: {}",e);
                                let _ = server.send(&error, Sender::WS(&tx));
                            }
                        }
                    },
                    Err(_) => break,
                }
            },
            Err(_) => tx.send(Ok(Message::text("Error reading what was sent to tcp server"))).expect("Failed to send message"),
        }

        // }else{
        //     tx.send(Ok(Message::text("Error reading what was sent to tcp server"))).expect("Failed to send message");
        // }
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
                
                let _ = stream.write_all(b"hello");

                let n = stream.read(&mut buffer[..]).unwrap();

                let buffer = &buffer[..n];

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
            let manager = ServiceManager::new();
            let _ = ServiceManager::start_server(manager).await;
        },
        "database" => {
            let manager = DataBaseManager::new();
            let _ = ServiceManager::start_tcp_server(manager,"").await;
        }
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
    let mut online_db = false;
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
                    let manager = ServiceManager::new();
                    let x = tokio::task::spawn_blocking(|| {
                        let _ = block_on(ServiceManager::start_server(manager));
                    });

                    processes.push(x);
                    online_sm = true;
                }
            },
            "db" => {
                if !online_db {
                    let manager = DataBaseManager::new();
                    let y = tokio::task::spawn_blocking(move || {
                        let _ = block_on(ServiceManager::start_tcp_server(manager,"8000"));
                    });
                    processes.push(y);
                    online_db = true;
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
