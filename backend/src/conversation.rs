use std::time::SystemTime;

use anyhow::{bail, anyhow};

use anyhow::{Context, Result};
use jammdb::{Error as JammError, DB};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
pub enum Programs {}

pub trait Program {
    fn help() -> String;
    fn run() -> anyhow::Result<String>;
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum User {
    Jake,
    Zack,
}
impl ToString for User {
    fn to_string(&self) -> String {
        match self {
            Self::Jake => String::from("Jake"),
            Self::Zack => String::from("Zack"),
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Message {
    UserMessage {
        user: User,
        msg: Option<String>,
        time: SystemTime,
    },
}
impl Message {
    pub fn tostr(&self) -> anyhow::Result<String> {
        match &self {
            Message::UserMessage { user, msg, time } => {
                let datetime: chrono::DateTime<chrono::offset::Utc> = time.clone().into();
                match msg {
                    Some(msg) => Ok(format!(
                        "---\n{} {}:\n{}\n",
                        datetime.format("%Y-%m-%d %T"),
                        user.to_string(),
                        msg
                    )),
                    None => Ok(format!(
                        "---\n{} {}:",
                        datetime.format("%Y-%m-%d %T"),
                        user.to_string()
                    )),
                }
            }
            _ => {
                bail!("unimplemented message type to string")
            }
        }
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    pub id: Option<String>,
    pub messages: Vec<Message>,
}
impl Conversation {
    pub fn tostr(&self) -> anyhow::Result<String> {
        let db = &jammdb::DB::open("my-db.db")?;
        db.tx(true);
        let mut result = String::new();
        for m in &self.messages {
            result.push_str(&m.tostr()?)
        }
        Ok(result)
    }
}

#[derive(Clone)]
pub struct Conversations {
    pub db: Arc<DB>,
    pub bucket_name: String,
}

impl Conversations {
    pub fn new(db: Arc<DB>, bucket_name: Option<String>) -> anyhow::Result<Self> {
        let bucket_name = bucket_name.unwrap_or_else(|| "conversations".to_string());
        let tx = db.tx(true)?;
        let create_result = tx.create_bucket(bucket_name.to_string());
        match create_result {
            Ok(_) => {}
            Err(JammError::BucketExists) => {}
            Err(e) => anyhow::bail!("failed to create bucket {e}") 
        };

        tx.commit()?;

        Ok(Conversations { db, bucket_name })
    }

    pub fn get(&self, uuid: &str) -> Result<Option<Conversation>> {
        let tx = self.db.tx(false)?;
        let bucket = tx.get_bucket(self.bucket_name.clone())?;
        let data = bucket.get(uuid.as_bytes());
        let data = match data {
            Some(data) => data,
            None => return Ok(None),
        };

        let conv: Conversation = serde_json::from_slice(&data.kv().value())
            .context("Failed to deserialize conversation data")?;
        Ok(Some(conv))
    }

    pub fn insert(&mut self, mut to_insert: Conversation) -> Result<String> {
        let uuid = match to_insert.id {
            Some(ref id) => id.clone(),
            None => {
                let new_uuid = Uuid::new_v4().to_string();
                to_insert.id = Some(new_uuid.clone());
                new_uuid
            }
        };
        let uuid_clone = uuid.clone();
        let tx = self.db.tx(true)?;
        let bucket = tx.get_bucket(self.bucket_name.clone())?;
        let data =
            serde_json::to_vec(&to_insert).context("Failed to serialize conversation data")?;
        bucket.put(uuid_clone.as_bytes(), data)?;
        tx.commit()?;
        Ok(uuid)
    }
}

impl IntoIterator for Conversations {
    type Item = (String, Conversation);
    type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        let tx = match self.db.tx(false) {
            Ok(tx) => tx,
            Err(_) => return Box::new(std::iter::empty()),
        };
        let bucket = match tx.get_bucket(self.bucket_name) {
            Ok(b) => b,
            Err(_) => return Box::new(std::iter::empty()),
        };

        let mut data = Vec::new();
        for k in bucket.into_iter() {
            if let Ok(uuid_str) = std::str::from_utf8(k.kv().key().clone()) {
                if let Ok(conv) = serde_json::from_slice::<Conversation>(k.kv().value()) {
                    data.push((uuid_str.to_string(), conv.clone()));
                }
            }
        }
        Box::new(data.into_iter())
    }
}
