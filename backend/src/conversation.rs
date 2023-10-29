use std::collections::HashMap;

use clap::{Parser, Subcommand};
use std::time::SystemTime;

use anyhow::{anyhow, bail};

use anyhow::{Context, Result};
use jammdb::{Error as JammError, DB};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use strum_macros::Display;
use uuid::Uuid;

use crate::nexos::{extract_commands, Command, LogLine, NexosInstance};
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
    System,
    TaskReport { creator: Box<User> },
}
impl ToString for User {
    fn to_string(&self) -> String {
        match self {
            Self::Jake => String::from("Me"),
            Self::Zack => String::from("Zack"),
            Self::Docker => String::from("Docker"),
            Self::System => String::from("System"),
            Self::TaskReport { creator } => format!("{} (from subtask)", creator.to_string()),
        }
    }
}
#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub task_actions: Vec<TaskAction>,
    pub omit_history_until: Option<String>,
    pub exclude_from_training: bool,
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TaskAction {
    Create { id: String, name: String },
    Enter { id: String },
    Exit { id: String, summary: String },
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub time: SystemTime,
    pub meta: Metadata,
    pub user: User,
    pub msg: String,
    pub id: String,
}

impl Message {
    pub fn new(user: User) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            time: SystemTime::now(),
            user,
            meta: Metadata::default(),
            msg: String::new(),
        }
    }

    pub fn new_with_msg(user: User, msg: String) -> Self {
        let mut new = Self::new(user);
        new.msg = msg;
        new
    }
    pub fn to_prompt_template(&self) -> anyhow::Result<MessagePromptTemplateEntry> {
        let mut message = String::new();
        message += "\t";
        message += &self.msg.replace("\n", "\n\t");
        Ok(MessagePromptTemplateEntry {
            author: self.user.to_string(),
            value: message,
        })
    }
    pub fn to_meta_entries(
        &self,
        conversation: &Conversation,
    ) -> anyhow::Result<Vec<MetadataPromptTemplateEntry>> {
        let mut res = Vec::new();
        let datetime: chrono::DateTime<chrono::offset::Utc> = self.time.clone().into();
        res.push(MetadataPromptTemplateEntry {
            key: "Time".to_string(),
            value: datetime.format("%Y-%m-%d %T").to_string(),
        });

        let task_stack = conversation.get_task_stack(&self.id, false)?;
        if task_stack.len() > 0 {
            let mut tasks = String::new();
            for task in task_stack {
                tasks.push_str(&format!("\t- {}\n", task.name))
            }

            res.push(MetadataPromptTemplateEntry {
                key: "Tasks".to_string(),
                value: tasks.trim_end().to_string(),
            });
        }
        Ok(res)
    }
    pub fn eval(&mut self, conversation: &Conversation) -> anyhow::Result<Vec<Message>> {
        let commands = extract_commands(&self.msg);
        dbg!(&commands);
        let mut to_ret = Vec::new();
        // clear the metadata every time. It should be generated through the eval
        self.meta = Metadata::default();
        for command in commands {
            let mut out = NexosInstance {};
            match command {
                Command::Nexos(command) => {
                    let req = out.exec_simple(&command);

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    // Call the asynchronous connect method using the runtime.
                    let result = rt.block_on(req).context("failed to exec command")?;
                    let mut msg = String::new();
                    for line in &result.output {
                        match line {
                            LogLine::StdOut { message } => msg += message,
                            LogLine::StdErr { message } => msg += message,
                        }
                    }

                    to_ret.push(Message::new_with_msg(User::Docker, msg));
                }
                Command::System(command) => {
                    let mut args = shellwords::split(&command)?;
                    // add an extra empty string as the program name here
                    args.insert(0, String::new());

                    let subcommands = SystemCli::try_parse_from(args);
                    match subcommands {
                        Ok(subcommands) => match subcommands.command {
                            SystemSubcommand::Task { command } => match command {
                                TaskNexosCommand::Start { name } => {
                                    let uuid = uuid::Uuid::new_v4();
                                    self.meta.task_actions.push(TaskAction::Create {
                                        id: uuid.to_string(),
                                        name: name.clone(),
                                    });

                                    self.meta.task_actions.push(TaskAction::Enter {
                                        id: uuid.to_string(),
                                    });

                                    to_ret.push(Message::new_with_msg(
                                        User::System,
                                        format!("Task \"{}\" started", name.clone()),
                                    ));
                                }
                                TaskNexosCommand::Done { summary } => {
                                    let task_stack =
                                        conversation.get_task_stack(&self.id, false)?;

                                    let exited_task =
                                        task_stack.last().ok_or(anyhow!("no tasks to exit"))?;
                                    self.meta.task_actions.push(TaskAction::Exit {
                                        id: exited_task.id.clone(),
                                        summary: summary.clone(),
                                    });
                                    let mut new_msg = Message::new_with_msg(
                                        User::TaskReport {
                                            creator: Box::new(User::Jake),
                                        },
                                        format!(
                                            "Task \"{}\" finished with summary \"{}\"",
                                            exited_task.name.clone(),
                                            summary.clone()
                                        ),
                                    );
                                    new_msg.meta.omit_history_until =
                                        Some(exited_task.msg_start_id.clone());
                                    to_ret.push(new_msg);
                                }
                            },
                            SystemSubcommand::Nexos { command } => match command {
                                SystemNexosCommand::Rebuild {} => {
                                    let req = out.rebuild();

                                    let rt = tokio::runtime::Builder::new_current_thread()
                                        .enable_all()
                                        .build()
                                        .unwrap();

                                    // Call the asynchronous connect method using the runtime.
                                    let result =
                                        rt.block_on(req).context("failed to exec command")?;
                                    let mut msg = String::new();
                                    for line in &result.output {
                                        match line {
                                            LogLine::StdOut { message } => msg += message,
                                            LogLine::StdErr { message } => msg += message,
                                        }
                                    }

                                    to_ret.push(Message::new_with_msg(User::Docker, msg));
                                }
                            },
                        },
                        Err(e) => {
                            to_ret.push(Message::new_with_msg(User::Docker, e.to_string()));
                        }
                    }
                }
                _ => todo!(),
            }
        }
        Ok(to_ret)
    }
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    pub id: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default = "field_1_default")]
    pub time: SystemTime,
}
impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: Option::default(),
            messages: Vec::default(),
            time: SystemTime::now(),
        }
    }
}
fn field_1_default() -> SystemTime {
    SystemTime::now()
}
pub fn messages_prompt_data(
    prev_msgs: &[Message],
    curr_msg: &Message,
    conversation: &Conversation,
) -> anyhow::Result<PromptTemplateData> {
    let mut data = PromptTemplateData::default();

    data.meta = curr_msg
        .to_meta_entries(conversation)
        .context("get meta entries")?;
    data.response = curr_msg.msg.clone();
    let mut omit_until: Option<String> = None;
    for m in prev_msgs.iter().rev() {
        if let Some(until_id) = &omit_until {
            if m.id == **until_id {
                omit_until = None;
            } else {
                continue;
            }
        }

        if let Some(until_id) = &m.meta.omit_history_until {
            omit_until = Some(until_id.clone())
        }
        data.msgs
            .push(m.to_prompt_template().context("to prompt template")?)
    }
    data.msgs.reverse();
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
        let prompt_data = messages_prompt_data(&self.messages[0..i], &self.messages[i], self)
            .context("getting messages prompt data")?;
        templates::prompt(&prompt_data)
    }
    pub fn to_training_data(&self) -> anyhow::Result<Vec<String>> {
        let mut data = Vec::new();
        for i in 0..self.messages.len() {
            if self.messages[i].user == User::Jake {
                if self.messages[i].meta.exclude_from_training {
                    continue;
                }
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
    pub fn apply(&mut self, action: ConversationAction) -> anyhow::Result<()> {
        match action {
            ConversationAction::AddMessage { index, user } => {
                let msg = Message::new(user);
                match index {
                    Some(index) => self.messages.insert(index, msg),
                    None => self.messages.push(msg),
                }
            }
            ConversationAction::MutateMessage { new_message } => {
                let _ = std::mem::replace(
                    self.messages
                        .iter_mut()
                        .find(|m| m.id == new_message.id)
                        .ok_or(anyhow::anyhow!(
                            "failed to mutate message because we failed to match ids"
                        ))?,
                    new_message,
                );
            }
            ConversationAction::EvalMessage { id } => {
                let (index, msg) = self
                    .messages
                    .iter()
                    .enumerate()
                    .find(|(_, m)| m.id == id)
                    .ok_or(anyhow::anyhow!("failed to eval because did not find id"))?
                    .clone();
                let mut msg = msg.clone();
                let mut to_add = msg.eval(&self)?;
                self.messages[index] = msg;
                to_add.reverse();
                for newmsg in to_add {
                    self.messages.insert(index + 1, newmsg)
                }
            }
            ConversationAction::DeleteMessage { id } => {
                self.messages.retain(|i| i.id != id);
            }
        }
        Ok(())
    }
    pub fn get_msgs_till(&self, msgid: &str, inclusive: bool) -> Vec<Message> {
        let mut result = Vec::new();
        for msg in &self.messages {
            if msg.id == msgid {
                if inclusive {
                    result.push(msg.clone());
                }
                break;
            }
            result.push(msg.clone());
        }
        result
    }
    pub fn get_tasks_till(
        &self,
        msgid: &str,
        inclusive: bool,
    ) -> anyhow::Result<HashMap<String, TaskInfo>> {
        let mut tasks = HashMap::new();
        let visible_msgs = self.get_msgs_till(msgid, inclusive);
        for msg in &visible_msgs {
            for action in &msg.meta.task_actions {
                if let TaskAction::Create { ref id, ref name } = action {
                    tasks.insert(
                        id.clone(),
                        TaskInfo {
                            id: id.clone(),
                            name: name.clone(),
                            done: false,
                            summary: None,
                            msg_start_id: msg.id.clone(),
                            msg_end_id: None,
                        },
                    );
                }
            }
        }

        for msg in &visible_msgs {
            for action in &msg.meta.task_actions {
                if let TaskAction::Exit {
                    ref id,
                    ref summary,
                } = action
                {
                    let muttask = tasks
                        .get_mut(id)
                        .ok_or(anyhow!("Failed to find matching id for task action exit"))?;
                    muttask.done = true;
                    muttask.msg_end_id = Some(msg.id.clone());
                    muttask.summary = Some(summary.clone());
                }
            }
        }
        Ok(tasks)
    }
    pub fn get_task_stack(&self, msgid: &str, inclusive: bool) -> anyhow::Result<Vec<TaskInfo>> {
        let mut stack = Vec::new();

        let visible_msgs = self.get_msgs_till(msgid, inclusive);
        let tasks = self.get_tasks_till(msgid, inclusive)?;

        for msg in &visible_msgs {
            for action in &msg.meta.task_actions {
                if let TaskAction::Enter { ref id } = action {
                    let task = tasks
                        .get(id)
                        .ok_or(anyhow!("failed to find matching id for task action enter"))?;
                    if !task.done {
                        stack.push(task.clone());
                    }
                }
            }
        }

        Ok(stack)
    }
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub done: bool,
    pub msg_start_id: String,
    pub msg_end_id: Option<String>,
    pub summary: Option<String>,
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
                match rmp_serde::from_slice::<Conversation>(k.kv().value()) {
                    Ok(conv) => {
                        data.push((uuid_str.to_string(), conv.clone()));
                    }
                    Err(err) => {
                        eprintln!("{err:?}")
                    }
                }
            }
        }
        Box::new(data.into_iter())
    }
}

#[derive(Clone, Debug, Display, PartialEq)]
pub enum ConversationAction {
    DeleteMessage { id: String },
    AddMessage { index: Option<usize>, user: User },
    EvalMessage { id: String },
    MutateMessage { new_message: Message },
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct SystemCli {
    #[command(subcommand)]
    command: SystemSubcommand,
}

#[derive(Subcommand, Debug)]
enum SystemSubcommand {
    /// Work with Nexos
    Nexos {
        #[command(subcommand)]
        command: SystemNexosCommand,
    },
    /// Work with and create Tasks
    Task {
        #[command(subcommand)]
        command: TaskNexosCommand,
    },
}

#[derive(Subcommand, Debug)]
enum SystemNexosCommand {
    /// Rebuild Nexos from the dockerfile at ~/System/Dockerfile.txt
    Rebuild {},
}

#[derive(Subcommand, Debug)]
enum TaskNexosCommand {
    /// Create and start a new task
    Start {
        #[arg(short, long)]
        name: String,
    },
    /// Exit the current task with a summary
    Done {
        #[arg(short, long)]
        summary: String,
    },
}
