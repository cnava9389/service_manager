use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;
use super::json;


type Res<T> = Result<T, Box<dyn std::error::Error>>;
pub trait Manager: Sync + Send{
    fn test(&self);
    
    fn process_message<'a>(&self, message: &'a str, sender: Sender) -> Res<&'a str>;

    fn send(&self) -> Res<()>;
}

pub enum Sender {
    TCP(TcpStream),
    WS(UnboundedSender<Res<Message>>),
}

impl Sender {
    fn send(&self, message: &str) -> Res<()> {        
        
        match *self {

            Sender::TCP(ref x) => {
                let mut y = Box::new(x);
                y.write_all(message.as_bytes())?;
            },
            Sender::WS(ref x) => {
                x.send(Ok(Message::text(format!("hello from service manager tcp server your message: {}", message)))).expect("Failed to send message");
            },
        }
        Ok(())
    }
}

#[derive(Debug,Clone)]
pub struct Store {
    store: HashMap<String, json::JSON>
}

impl Store{
    pub fn new() -> Store {
        Store { store: HashMap::new() }
    }
}

#[derive(Debug,Clone)]
pub struct EventQueue {
    queue: Vec<String>
}

impl EventQueue {
    pub fn new() -> EventQueue {
        EventQueue { queue: Vec::new() }
    }
}
