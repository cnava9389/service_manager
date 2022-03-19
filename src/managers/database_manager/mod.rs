use std::{collections::HashMap, sync::Arc, path::Path, fs::{File, OpenOptions, self}, io::{BufReader, Seek}, fmt, str::Split, borrow::BorrowMut, iter::Peekable};
use serde::{Serialize, Deserialize};
use serde_json::{map::Entry, Map};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use futures::executor::block_on;
use tokio::{sync::RwLock};

use super::{error,json, service_manager};

type Res<T> = Result<T, error::ServerError>;

#[derive(Debug)]
enum FileManager {
    Closed,
    Open(File)
}

#[derive(Debug)]
pub struct DataBaseManager {
    file: RwLock<FileManager>,
    store: RwLock<HashMap<String,json::JSON>>,
}

// ! does not work
impl fmt::Display for DataBaseManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"store: ({:?}), file: ({:?})",block_on(self.store.read()),block_on(self.file.read()))
    }
}

impl DataBaseManager {
    pub fn new() -> Arc<DataBaseManager> {
        Arc::new(DataBaseManager{
            file: RwLock::new(FileManager::Closed),
            store: RwLock::new(HashMap::new()),
        })
    }
    
    async fn store_size(&self) -> usize {
        let store = self.store.read().await;
        store.len()
    }

    /// if a files is open the current store is saved to the file otherwise en error is returned
    fn save_store(&self) -> Res<String> {
        let tuple = async {
            let file = self.file.write();
            let store = self.store.read();
            futures::join!(file,store)
        };
        let (mut file, store) = block_on(tuple);
        match *file {
            FileManager::Closed => Ok(String::from("No file open to save to")),
            FileManager::Open(ref mut x) => {
                let _ = x.set_len(0);
                let _ = x.rewind();
                match serde_json::to_writer(x, &*store) {
                    Ok(_) => Ok(String::from("Saved to file")),
                    Err(e) => {
                        println!("{}",e);
                        Err(error::ServerError::INVALID_JSON)
                    }
                }
            },
        }
    }

    /// opens a file at path or creates a new one if not present. returns error if fails to create new file.
    async fn open(&self, path: &str) -> Res<()> {
        let mut dir_part = String::new();
        if !path.contains(".json") {
            return Err(error::ServerError::INVALID_FILE);
        }
        if path.contains("/") {
            let mut split = path.split("/");
            loop {
                match split.next() {
                    Some(x) => {
                        if !x.contains(".json") {
                            dir_part += x;
                            dir_part += "/";
                        }
                    },
                    None => break
                };
            };
        };

        let path = Path::new(path);
        let file = OpenOptions::new().write(true).read(true).open(path);//File::open(path);
        match file {
            Ok(x) => {
                let mut this_file = self.file.write().await;
                *this_file = FileManager::Open(x);
                Ok(())
            },
            Err(_) => {
                let mut this_file = self.file.write().await;
                if !dir_part.is_empty() && dir_part != "./" {
                    let _ = fs::create_dir_all(dir_part);
                }
                let new_file = OpenOptions::new().create_new(true).append(true).open(path);
                match new_file {
                    Ok(x) => {
                        *this_file = FileManager::Open(x);
                        Ok(())
                    },
                    Err(_) => {
                        Err(error::ServerError::INVALID_FILE)
                    }
                }
            }
        }
    }

    /// loads json object from file into store if file is opened.
    async fn load_from_file(&self) -> Res<()> {
        let file = self.file.read().await;
        match *file {
            FileManager::Closed => {
                Err(error::ServerError::INVALID_FILE)
            },
            FileManager::Open(ref x) => {
                let reader = BufReader::new(x);
                let map= serde_json::from_reader::<BufReader<&File>, json::JSON>(reader);
                match map {
                    Ok(x) => {
                        match x {
                            serde_json::Value::Object(mut x) => {
                                let keys = x.keys().cloned().collect::<Vec<_>>();
                                for k in keys {
                                    match x.remove(&k) {
                                        Some(v) => {
                                            let _ = self.insert_to_store(k, v).await ;
                                        },
                                        None => (),
                                    };
                                };
                                if x.len() != 0 {
                                    return Err(error::ServerError::INCOMPLETE_OPERATION);
                                }
                                Ok(())
                            },
                            _ => {
                                Err(error::ServerError::INVALID_JSON)
                            }
                        }
                    },
                    Err(_) => Err(error::ServerError::INVALID_JSON),
                }
            },
        }
    }

    /// inserts item into store. works with nested objects
    async fn insert_to_store(&self, k:String, v:json::JSON) -> Res<()> {
        let key;
        let mut split;
        let mut store = self.store.write().await;
        let contains = k.contains(".");
        if contains {
            split = k.split(".").peekable();
            key = split.next().unwrap().to_owned();
        }else {
            split = "err.err".split(".").peekable();
            key = k;
        }
        if contains {
            match store.get_mut(&key) {
                Some(x) => {
                    if x.is_object() {
                        rec_ins(split,x,v)
                    }else {
                        // fail
                        Err(error::ServerError::INVALID_JSON)
                    }
                },
                None => {
                    Err(error::ServerError::INVALID_DATA)
                }
            }
        }else {
             match store.get(&key) {
            Some(x) => {
                if (x.is_array() && v.is_array()) || (x.is_string() && v.is_string()) || (x.is_object() && v.is_object()) ||
                (x.is_boolean() && v.is_boolean()) || (x.is_number() && v.is_number()) || (x.is_f64() && v.is_f64()) || 
                (x.is_i64() && v.is_i64()) || (x.is_u64() && v.is_u64()){
                    match store.insert(key, v) {
                        Some(_) => Ok(()),
                        None => Ok(()),
                    }
                }else {
                    Err(error::ServerError::INCOMPATIBLE_DATA_TYPES)
                }
            },
            None => {
                match store.insert(key, v) {
                    Some(_) => Ok(()),
                    None => Ok(()),
                }
            },
        }
        }
    }
}

impl service_manager::other::Manager for DataBaseManager {
    fn process_message(&self, message: &str, _id: &str) -> Res<String>{
        let mut split = message.split(" ");
        let str = split.next().unwrap();
        match str {
            "NEW" =>{
                Ok("New connection to this db".to_owned())
            },
            "OPEN" => {
                let file = split.next().unwrap_or_else(|| "Error");
                match block_on(self.open(file)) {
                    Ok(_) => Ok(format!("opened file {}",file)),
                    Err(e) => Err(e)
                }
            },
            "LOAD" => {
                let file_enum = block_on(self.file.read());
                match *file_enum {
                    FileManager::Open(_) => {
                        match block_on(self.load_from_file()){
                            Ok(_) => Ok("loaded file successfully".to_owned()),
                            Err(e) => Err(e),
                        }
                    },
                    FileManager::Closed => {
                        Ok("There is no file to load from, please open one with OPEN".to_owned())
                    }
                }
            },
            "FND" => {
                let key = split.next().unwrap_or_else(|| "Error");
                let store = block_on(self.store.read());
                if key.contains(".") {
                    let mut key_split = key.split(".").peekable();
                    let first_k = key_split.next().unwrap_or_else(||"Error");
                    let first_v = store.get(first_k);
                    match first_v {
                        Some(val) => {
                            rec_get(key_split,val)
                        },
                        None => {
                            Err(error::ServerError::MISSING_DATA)
                        },
                    }
                }else {
                    let value = store.get(key);
                    match value {
                        Some(x) => {
                            Ok(x.to_string())
                        },
                        None => Ok(json::JSON::Null.to_string())
                    }
                }
            },
            "INS" => {
                let key = split.next().unwrap_or_else(|| "Error");
                let value_str = split.next().unwrap_or_else(|| "Error");
                let value = serde_json::from_str::<json::JSON>(value_str);
                match value {
                    Ok(x) => {
                        let x_string = value_str;
                        match block_on(self.insert_to_store(key.to_owned(), x)) {
                            Ok(_) => Ok(x_string.to_owned()),
                            Err(e) => Err(e),
                        }
                    },
                    Err(_) => Err(error::ServerError::FAILED_READ),
                }
            },
            "DEL" => {
                let key = split.next().unwrap_or_else(|| "Error");
                let mut store = block_on(self.store.write());
                if key.contains(".") {
                    let mut key_split = key.split(".").peekable();
                    let first_k = key_split.next().unwrap_or_else(||"Error");
                    let first_v = store.get_mut(first_k);
                    match first_v {
                        Some(val) => {
                            rec_del(key_split,val)
                        },
                        None => {
                            Err(error::ServerError::MISSING_DATA)
                        },
                    }
                }else {
                    match store.remove(key) {
                        Some(x) =>  Ok(x.to_string()),
                        None => Err(error::ServerError::INVALID_DATA),
                    }
                }
            },
            "SAVE" => {
                self.save_store()
            }
            _ => {
                Err(error::ServerError::INVALID_ARG)
            }
        }
     
    }
    fn send(&self, message: &str, sender: service_manager::other::Sender) -> Res<()> {
        match sender {
            // this writes back data from tcp servers
            service_manager::other::Sender::TCP(x) => {
                match block_on(x.write_all(message.as_bytes())) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(error::ServerError::NO_VALUE),
                }
            },
            _ => Err(error::ServerError::INVALID_ARG)
        }
    }
}
/// deletes item from database if presents. works with nested objects
fn rec_del(mut key_split:Peekable<Split<&str>>, map: &mut json::JSON) -> Res<String> {
    let key = key_split.next();
    if key_split.peek().is_some() {
        match key {
            Some(key) => {
                match map.get_mut(key) {
                    Some(map) => {
                        rec_del(key_split, map)
                    },
                    None => {
                        Err(error::ServerError::INVALID_JSON)
                    },
                }

            },
            None => {
                Err(error::ServerError::INVALID_ARG)
            },
        }
    } else{
        match key {
            Some(key) => {
                match map.as_object_mut() {
                    Some(map) => {
                        let value = map.remove(key);
                        match value {
                            Some(val) => Ok(val.to_string()),
                            None => Ok(String::from("NOT_FOUND")),
                        }
                    },
                    None => todo!(),
                }
            },
            None => Err(error::ServerError::INVALID_ARG),
        }
    }
}

// recursively gets the specified key
fn rec_get(mut key_split:Peekable<Split<&str>>, map: &json::JSON) -> Res<String> {
    let key = key_split.next();
    if key_split.peek().is_some() {
        match key {
            Some(key) => {
                match map.get(key) {
                    Some(map) => {
                        rec_get(key_split, map)
                    },
                    None => {
                        Err(error::ServerError::INVALID_JSON)
                    },
                }

            },
            None => {
                Err(error::ServerError::INVALID_ARG)
            },
        }
    } else{
        match key {
            Some(key) => {
                let value = map.get(key);
                match value {
                    Some(val) => Ok(val.to_string()),
                    None => Ok(json::JSON::Null.to_string()),
                }
            },
            None => Err(error::ServerError::INVALID_ARG),
        }
    }
}

/// recursively inserts item at given path
fn rec_ins(mut str_iter:Peekable<Split<&str>>, map: &mut json::JSON, val:json::JSON) -> Res<()> {
    let key = str_iter.next();
    match key {
        Some(str) => {
            println!("{:?}",map);
            match map {
                serde_json::Value::Object(map_obj) => {
                    println!("map passed as object, key:{}",str);
                    let mo_v_o = map_obj.get_mut(str);
                    match str_iter.peek().is_some() {
                        true => {
                            match mo_v_o{
                                    Some(mo_v) => {
                                        println!("value passed to func {}",mo_v);
                                        rec_ins(str_iter,mo_v,val)
                                    },
                                    None => Err(error::ServerError::INVALID_ARG),
                                    
                                }
                        },
                        false => {
                            match mo_v_o{
                                Some(mo_value) => {
                                    if (mo_value.is_array() && val.is_array()) || (mo_value.is_string() && val.is_string()) || (mo_value.is_object() && val.is_object()) ||
                                    (mo_value.is_boolean() && val.is_boolean()) || (mo_value.is_number() && val.is_number()) || (mo_value.is_f64() && val.is_f64()) || 
                                    (mo_value.is_i64() && val.is_i64()) || (mo_value.is_u64() && val.is_u64()){
                                        let _ = map_obj.insert(str.to_string(), val);
                                        Ok(())
                                    }else {
                                        Err(error::ServerError::INCOMPATIBLE_DATA_TYPES)
                                    }
                                },
                                None => {
                                    let _ = map_obj.insert(str.to_string(), val);
                                    Ok(())
                                },
                            }
                            
                        },
                    }
                    
                },
                _ => Err(error::ServerError::INCOMPLETE_OPERATION)
            }
        },
        None => {
            Err(error::ServerError::FAILED_READ)
        }  
    }
}