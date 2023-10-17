#[macro_use]
extern crate mopa;
mod conversation;
mod editor;
mod frontend;
mod model_server;
mod openai;
mod templates;
mod nexos;
use anyhow::Context;
use clap::Parser;
use std::{sync::Arc, time::SystemTime};

use conversation::*;
use model_server::*;
use openai::*;

use crate::frontend::launch_gui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Subcommands,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Subcommands {
    Test {
        #[arg(short, long)]
        derived_flag: bool,
    },
    Server {},
    Frontend {},
}

fn main() {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let subcommands = Cli::parse();
    let db = jammdb::DB::open("real.db").unwrap();
    let mut conversations = Conversations::new(Arc::new(db), None).unwrap();
    println!("{:?}", subcommands);
    match subcommands.command {
        Subcommands::Frontend {} => launch_gui(conversations).unwrap(),
        // Subcommands::Test { .. } => test().await,
        // Subcommands::Server { .. } => server().await.unwrap(),
        _ => {}
    }
}
async fn server() -> anyhow::Result<()> {
    println!("fuck!");
    let mut srv = InferenceServer::start(&InferenceServerArgs {
        model_config: ".".into(),
        image_name: "fuck".into(),
        port: 9090,
    })
    .context("failed to start server")?;
    // srv.startreq().await.context("failed to wait for server to start")?;
    // let resp = srv
    //     .infer(InferReq {
    //         prompt: "fuck".into(),
    //         config: GenerationConfig::default(),
    //     })
    //     .await?;
    // println!("{:?}", resp);

    Ok(())
}
async fn test() {
    let resp = StatusResp {body: ServerStatus::Generating { text: "chicken".into() }};
    dbg!("{:?}",&resp);
    println!("{}",serde_json::to_string(&resp).unwrap());

}
// async fn test() {
//     let mut conversation = Conversation::default();
//     let db = jammdb::DB::open("real.db").unwrap();
//     let mut conversations = Conversations::new(Arc::new(db), None).unwrap();

//     let mut serv = InferenceServer::new("http://localhost:9090");

//     for k in conversations.clone().into_iter() {
//         println!("iter: {:?}", k);
//     }
//     loop {
//         let my_msg = editor::edit_content("").unwrap();
//         if my_msg.to_lowercase() == "exit" {
//             break;
//         }
//         conversation.messages.push(Message::UserMessage {
//             user: User::Zack,
//             msg: my_msg.clone(),
//             time: SystemTime::now(),
//         });

//         conversations.insert(&mut conversation).unwrap();
//         let prompt = templates::prompt(&templates::PromptDetails {
//             conversation: messages_tostr(conversation.messages.as_slice())
//                 .unwrap()
//                 .into(),
//         })
//         .unwrap();
//         println!("{}", prompt);
//         let infer = serv
//             .infer(InferReq {
//                 prompt: my_msg,
//                 config: GenerationConfig::default(),
//             })
//             .await
//             .unwrap();
//         println!("{:?}", &infer);
//         let jake_msg = editor::edit_content(infer.completion).unwrap();
//         conversation.messages.push(Message::UserMessage {
//             user: User::Jake,
//             msg: jake_msg,
//             time: SystemTime::now(),
//         });
//         conversations.insert(&mut conversation).unwrap();
//     }
//     for (_, c) in conversations.into_iter() {
//         println!("{:+?}", c.to_training_data())
//     }
// }
