mod conversation;
mod editor;
mod openai;
mod remote_server;
mod templates;
use clap::Parser;
use std::{sync::Arc, time::SystemTime};

use conversation::*;
use openai::*;
use remote_server::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Subcommands>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Subcommands {
    Derived {
        #[arg(short, long)]
        derived_flag: bool,
    },
}

#[tokio::main]
async fn main() {
    let subcommands = Cli::parse();
    println!("{:?}", subcommands);

    let mut conversation = Conversation::default();
    let db = jammdb::DB::open("real.db").unwrap();
    let mut conversations = Conversations::new(Arc::new(db), None).unwrap();

    let mut serv = InferenceServer::new("http://localhost:9090");
    let start = serv.start(StartReq {}).await;
    println!("{:?}", start);

    for k in conversations.clone().into_iter() {
        println!("iter: {:?}", k);
    }
    loop {
        break;
        let my_msg = editor::edit_content("").unwrap();
        if my_msg.to_lowercase() == "exit" {
            break;
        }
        conversation.messages.push(Message::UserMessage {
            user: User::Zack,
            msg: my_msg.clone(),
            time: SystemTime::now(),
        });

        conversations.insert(&mut conversation).unwrap();
        let prompt = templates::prompt(&templates::PromptDetails {
            conversation: messages_tostr(conversation.messages.as_slice())
                .unwrap()
                .into(),
        })
        .unwrap();
        println!("{}", prompt);
        let infer = serv
            .infer(InferReq {
                prompt: my_msg,
                config: GenerationConfig::default(),
            })
            .await
            .unwrap();
        println!("{:?}", &infer);
        let jake_msg = editor::edit_content(infer.completion).unwrap();
        conversation.messages.push(Message::UserMessage {
            user: User::Jake,
            msg: jake_msg,
            time: SystemTime::now(),
        });
        conversations.insert(&mut conversation).unwrap();
    }
    for (_,c) in conversations.into_iter() {
    println!("{:+?}",c.to_training_data())
    }
}
