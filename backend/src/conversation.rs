use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::{anyhow, bail};

use anyhow::{Context, Result};
use jammdb::{Error as JammError, DB};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::templates::{
    self, MessagePromptTemplateEntry, MetadataPromptTemplateEntry, PromptTemplateData,
};
pub enum Programs {}

pub trait Program {
    fn help() -> String;
    fn run() -> anyhow::Result<String>;
}
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum User {
    Jake,
    Zack,
    Docker,
}
impl ToString for User {
    fn to_string(&self) -> String {
        match self {
            Self::Jake => String::from("Jake"),
            Self::Zack => String::from("Zack"),
            Self::Docker => String::from("Docker"),
        }
    }
}
#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Metadata {}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub time: SystemTime,
    pub meta: Metadata,
    pub user: User,
    pub msg: String,
}

impl Message {
    pub fn to_prompt_template(&self) -> anyhow::Result<MessagePromptTemplateEntry> {
        let mut message = String::new();
        message += "  ";
        message += &self.msg.replace("\n", "\n\t");
        Ok(MessagePromptTemplateEntry {
            author: self.user.to_string(),
            value: message,
        })
    }
    pub fn to_meta_entries(&self) -> anyhow::Result<Vec<MetadataPromptTemplateEntry>> {
        let mut res = Vec::new();
        let datetime: chrono::DateTime<chrono::offset::Utc> = self.time.clone().into();
        res.push(MetadataPromptTemplateEntry {
            key: "Time".to_string(),
            value: datetime.format("%Y-%m-%d %T").to_string(),
        });
        Ok(res)
    }
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    pub id: Option<String>,
    pub messages: Vec<Message>,
}
impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: Option::default(),
            messages: Vec::default(),
        }
    }
}
pub fn messages_prompt_data(
    prev_msgs: &[Message],
    curr_msg: &Message,
) -> anyhow::Result<PromptTemplateData> {
    let mut data = PromptTemplateData::default();

    data.meta = curr_msg.to_meta_entries().context("get meta entries")?;
    data.response = curr_msg.msg.clone();
    for m in prev_msgs {
        data.msgs
            .push(m.to_prompt_template().context("to prompt template")?)
    }
    Ok(data)
}

impl Conversation {
    pub fn msg_training_data(&self, i: usize) -> anyhow::Result<String> {
        let m = self.messages.get(i).ok_or(anyhow!(
            "i {} not in messages {}",
            i,
            self.messages.len()
        ))?;
        if m.user != User::Jake {
            bail!(
                "tried to get training data for non jake user {}, {}",
                m.user.to_string(),
                i
            )
        }
        let prompt_data = messages_prompt_data(&self.messages[0..i], &self.messages[i])
            .context("getting messages prompt data")?;
        templates::prompt(&prompt_data)
    }
    pub fn to_training_data(&self) -> anyhow::Result<Vec<String>> {
        let mut data = Vec::new();
        for i in 0..self.messages.len() {
            if self.messages[i].user == User::Jake {
                data.push(self.msg_training_data(i)?)
            }
        }
        return Ok(data);
    }
    pub fn write_jsonl<W: std::io::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        let training_data = self.to_training_data()?;
        #[derive(Serialize)]
        struct Data {
            text: String,
        }
        for msg in training_data {
            // We use to_string here instead of to_vec because it verifies that the JSON is valid UTF-8,
            // which is required by the JSON Lines specification (https://jsonlines.org).
            let json = serde_json::to_string(&Data { text: msg })?;

            writer.write_all(json.as_bytes())?;
            writer.write_all(b"\n")?;
        }

        Ok(())
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
            Err(e) => anyhow::bail!("failed to create bucket {e}"),
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

        let conv: Conversation = rmp_serde::from_slice(&data.kv().value())
            .context("Failed to deserialize conversation data")?;
        Ok(Some(conv))
    }

    pub fn insert(&mut self, to_insert: &mut Conversation) -> Result<String> {
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
            rmp_serde::to_vec(&to_insert).context("Failed to serialize conversation data")?;
        bucket.put(uuid_clone.as_bytes(), data)?;
        tx.commit()?;
        Ok(uuid)
    }

    pub fn delete(&mut self, uuid: &str) -> Result<()> {
        let tx = self.db.tx(true)?;
        let bucket = tx.get_bucket(self.bucket_name.clone())?;
        bucket.delete(uuid)?;
        tx.commit()?;
        Ok(())
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
                if let Ok(conv) = rmp_serde::from_slice::<Conversation>(k.kv().value()) {
                    data.push((uuid_str.to_string(), conv.clone()));
                }
            }
        }
        Box::new(data.into_iter())
    }
}
