use std::sync::Arc;
use std::{collections::HashMap, time::Duration};
use std::fmt;
use futures::executor::block_on;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use warp::body::bytes;
use warp::reply::Json;
use std::net::TcpStream as tcp;
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;
use super::{json, error, super::session_manager};
use std::io::{ prelude::*};


type Res<T> = Result<T, error::ServerError>;
pub trait Manager: Sync + Send{
    
    /// function processes the messasge and returns data 
    fn process_message<'a>(&self, message: &str, id: &str) -> Res<String>;

    /// sends data back from requested user
    fn send(&self, message: &str, sender: Sender) -> Res<()>;
}

pub enum Sender<'a> {
    TCP(&'a mut TcpStream),
    WS(&'a UnboundedSender<Result<Message, warp::Error>>),
}

#[derive(Debug)]
pub struct TCPServers<'a> {
    ports: RwLock<HashMap<&'a str, String>>,
    store: RwLock<HashMap<&'a str, tcp>>
}

impl fmt::Display for TCPServers<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let store = block_on(self.store.read());
        let ports = block_on(self.ports.read());
        write!(f,"store: ({:?}), ports: ({:?})",store,ports)
    }
}

impl<'a> TCPServers<'a> {
    pub fn new() -> TCPServers<'a> {
        TCPServers { 
            ports: RwLock::new(HashMap::new()),
            store: RwLock::new(HashMap::new())
         }
    }

    /// inserts value into store
    pub async fn insert (&self, k:&'a str, v: tcp) -> Res<()> {
        match self.store.write().await.insert(k,v) {
            Some(mut x) => {
                match x.write_all(b"NEW") {
                    Ok(_)=>{
                        println!("sent");
                        Ok(())
                    },
                    Err(_)=>{
                        println!("failed");
                        Err(error::ServerError::CONNECTION)
                    }
                }
            },
            None => Ok(()),
        }
    }
    
    pub async fn remove (&self, k:&'a str) -> Res<()> {
        let mut store = self.store.write().await;
        store.remove(k);
        Ok(())
    }

    pub async fn contains_key (&self, k: &str) -> bool {
        self.store.read().await.contains_key(k)
    }

    pub async fn contains(&self,k:&str,v:&str) -> Res<()> {
        let port_k_v = self.ports.read().await;
        let mut taken = false;
        let ports = port_k_v.iter();
        for (p_k, p_v) in ports {
            if p_k == &k {
                taken = true;
            } else if p_v == &v {
                taken = true;
            } else { 
                ()
            }
        };
        if taken {
            Err(error::ServerError::TAKEN)
        }else { Ok(()) }
    }

    pub async fn insert_port(&self, k:&'a str,v:String) -> Result<(),()> {
        let mut ports = self.ports.write().await;
        match ports.insert(k, v){
            Some(_) => Ok(()),
            None => Err(())
        }
    }

    pub async fn remove_server(&self, k:&str) -> Res<()> {
        let mut ports = self.ports.write().await;
        let mut store = self.store.write().await;

        match ports.remove(k){
            Some(_) => (),
            None => ()
        };

        match store.remove(k){
            Some(_) => (),
            None => ()
        };
        Ok(())
    }

    pub async fn send_to_server(&self, k:&str, message: &str) -> Res<String> {
        let server = self.store.read().await;
        let server = server.get(k);
        match server {
            Some(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::new(7,50)));
                match s.write_all(str::as_bytes(message)) {
                    Ok(_) => {
                        let mut buffer = [0; 1024];
                        let n = match s.read(&mut buffer[..]){
                            Ok(x) => {
                                x
                            },
                            Err(_) => {
                                0
                            },
                        };
                        if n == 0 {
                            return Err(error::ServerError::INVALID_DATA)
                        }
                        match std::str::from_utf8(&buffer[..n]) {
                            Ok(v) => {
                                Ok(v.to_owned())
                            },
                            Err(_) => Err(error::ServerError::INVALID_DATA),
                        }
                        
                    },
                    Err(_) => Err(error::ServerError::CONNECTION),
                }
            },
            None => Err(error::ServerError::CONNECTION)
        }
    }
}

#[derive(Debug)]
pub struct Store {
    store: RwLock<HashMap<String, json::JSON>>
}

impl Store{
    pub fn new() -> Store {
        Store { store: RwLock::new(HashMap::new()) }
    }
}
impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{:?}",block_on(self.store.read()))
    }
}

#[derive(Debug)]
pub struct EventQueue {
    queue: RwLock<Vec<String>>
}

impl fmt::Display for EventQueue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{:?}",block_on(self.queue.read()))
    }
}

impl EventQueue {
    pub fn new() -> EventQueue {
        EventQueue { queue: RwLock::new(Vec::new()) }
    }
}

pub async fn login_func(data: json::JSON, _manager: Arc<super::ServiceManager<'_>>) -> Result<Json, warp::Rejection> {
    let credentials = data;
    
    match credentials {
    serde_json::Value::Object(ref map) => {

        let value = map.get("email").unwrap_or_else(|| &json::JSON::Null);

        match value {

            serde_json::Value::String(email) => {

                let bytes_credentials = credentials.to_string();
                let bytes_credentials = bytes_credentials.as_bytes();
            
                let hash = session_manager::hash_bytes(bytes_credentials);
            
                match hash {
                    Ok(hash) => {
                        let credentials = warp::reply::json(&hash);
            
                        Ok(credentials)
                    }
                    Err(_) => {
                        Err(warp::reject::reject())
                    }
                }
            },
            _ => {
                Err(warp::reject::reject())
            }
        }
    },   
        _ => { 
            Err(warp::reject::reject())
        }
   // Err(warp::reject::reject())
    }
}
