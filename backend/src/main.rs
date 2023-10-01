mod openai;
mod remote_server;
mod templates;
mod conversation;
mod editor;
use std::time::SystemTime;

use openai::*;
use remote_server::*;
use conversation::*;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let mut conversation =Conversation::default(); 

    let my_msg = editor::edit_content("").unwrap();
    conversation.messages.push(Message::UserMessage { user: User::Zack, msg: Some(my_msg.clone()), time: SystemTime::now() });
    conversation.messages.push(Message::UserMessage { user: User::Jake, msg: None, time: SystemTime::now() });
    println!("{}",conversation.tostr().unwrap());
    let mut serv = InferenceServer::new("http://localhost:9090");
    let start = serv.start(StartReq {}).await;
    println!("{:?}", start);

    let infer = serv
        .infer(InferReq {
            prompt: my_msg,
            config: GenerationConfig::default(),
        })
        .await;
    println!("{:?}", infer);
}
