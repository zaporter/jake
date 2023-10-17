use std::{
    fs::File,
    sync::Arc,
    time::{Duration, SystemTime},
};

use eframe::egui;
use egui::{Ui, Widget, WidgetInfo};

use crate::{
    conversation::{Conversation, Conversations, Message, Metadata, User},
    model_server::{GenerationConfig, InferReq, InferenceServerArgs, ServerManager, ServerStatus},
    nexos::{extract_commands, LogLine, NexosInstance},
};
pub fn launch_gui(conversations: Conversations) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(300.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Jake",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::new(MyApp::new(conversations))
        }),
    );
    Ok(())
}
struct MyApp {
    conversations: Conversations,
    selected_convo: Option<String>,
    server_manager: ServerManager,
}

impl MyApp {
    fn new(conversations: Conversations) -> Self {
        Self {
            conversations,
            selected_convo: None,
            server_manager: ServerManager::default(),
        }
    }
}
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(50));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Jake");

            egui::SidePanel::left("left_panel")
                .resizable(true)
                .default_width(150.0)
                .width_range(80.0..=200.0)
                .show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Conversations");
                    });
                    if ui.button("new").clicked() {
                        match self.conversations.insert(&mut Conversation::default()) {
                            Ok(uuid) => self.selected_convo = Some(uuid),
                            Err(e) => {
                                println!("Error creating convo {e}")
                            }
                        }
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (u, c) in self.conversations.clone().into_iter() {
                            if ui.button(u.clone()).clicked() {
                                self.selected_convo = Some(u.clone());
                            }
                        }
                    });
                });

            ui.vertical_centered(|ui| {
                ui.heading(format!(
                    "Convo ({})",
                    self.selected_convo.clone().unwrap_or("none".into())
                ));
            });
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(ref convo_id) = self.selected_convo {
                            if ui.button("delete").clicked() {
                                let res = self.conversations.delete(convo_id);
                                if res.is_err() {
                                    println!("failed to delete a convo {:?}", res)
                                }
                                self.selected_convo = None;
                                return;
                            }
                            let conversation = self.conversations.get(convo_id);
                            if let Ok(Some(conversation)) = conversation {
                                let mut c_conversation = conversation.clone();
                                let mut to_delete = None;
                                let mut to_add: Vec<(usize, Message)> = Vec::new();
                                for i in 0..conversation.messages.len() {
                                    ui.group(|ui| {
                                        if ui.button("delete").clicked() {
                                            to_delete = Some(i);
                                        }
                                        let Message {
                                            ref user,
                                            ref mut msg,
                                            ref time,
                                            ..
                                        } = c_conversation.messages[i];
                                        let datetime: chrono::DateTime<chrono::offset::Utc> =
                                            time.clone().into();
                                        // ui.label(format!("{}", chrono::DateTime));
                                        ui.label(datetime.format("%Y-%m-%d %T").to_string());
                                        ui.label(format!("{}:", user.to_string()));
                                        let output = egui::TextEdit::multiline(msg)
                                            .hint_text("Type something!")
                                            .show(ui);
                                        if ui.button("eval").clicked() {
                                            let commands = extract_commands(&msg);
                                            dbg!(&commands);
                                            for command in commands {
                                                let mut out = NexosInstance {};
                                                let req = out.exec_simple(&command);

                                                let rt =
                                                    tokio::runtime::Builder::new_current_thread()
                                                        .enable_all()
                                                        .build()
                                                        .unwrap();

                                                // Call the asynchronous connect method using the runtime.
                                                let result = rt.block_on(req).unwrap();
                                                let mut msg = String::new();
                                                for line in &result.output {
                                                    match line {
                                                        LogLine::StdOut { message } => {
                                                            msg += message
                                                        }
                                                        LogLine::StdErr { message } => {
                                                            msg += message
                                                        }
                                                    }
                                                }

                                                to_add.push((
                                                    i+1,
                                                    Message {
                                                        time: SystemTime::now(),
                                                        user: User::Docker,
                                                        meta: Metadata::default(),
                                                        msg,
                                                    },
                                                ))
                                            }
                                        }
                                        if *user == User::Jake {
                                            if let Some(ref is) =
                                                self.server_manager.inference_server
                                            {
                                                ui.label("Inference server");
                                                let status = is.lock().unwrap().status().cloned();
                                                if status.is_err() {
                                                    ui.label(format!("Status error {:?}", status));
                                                    return;
                                                }
                                                let status = status.unwrap();
                                                if let ServerStatus::Ready { .. }
                                                | ServerStatus::DoneGenerating { .. } = status
                                                {
                                                    if ui.button("infer").clicked() {
                                                        println!(
                                                            "{:?}",
                                                            conversation.msg_training_data(i)
                                                        );
                                                        let prompt =
                                                            conversation.msg_training_data(i);
                                                        if let Ok(prompt) = prompt {
                                                            let resp = is
                                                                .lock()
                                                                .unwrap()
                                                                .infer(InferReq {
                                                                    prompt,
                                                                    config:
                                                                        GenerationConfig::default(),
                                                                })
                                                                .unwrap();
                                                            println!("{:?}", resp)
                                                        }
                                                    }
                                                };

                                                if let ServerStatus::DoneGenerating { text: val } =
                                                    status
                                                {
                                                    if ui.button("copy into").clicked() {
                                                        *msg = String::from(val);
                                                    }
                                                }
                                            };
                                        };
                                    });
                                }
                                if ui.button("+ User").clicked() {
                                    c_conversation.messages.push(Message {
                                        time: SystemTime::now(),
                                        user: User::Zack,
                                        meta: Metadata::default(),
                                        msg: String::new(),
                                    })
                                }
                                if ui.button("+ Assistant").clicked() {
                                    c_conversation.messages.push(Message {
                                        time: SystemTime::now(),
                                        user: User::Jake,
                                        meta: Metadata::default(),
                                        msg: String::new(),
                                    })
                                }
                                if let Some(i) = to_delete {
                                    c_conversation.messages.remove(i);
                                }
                                for (i, msg) in to_add.into_iter().rev() {
                                    c_conversation.messages.insert(i, msg)
                                }
                                if c_conversation != conversation {
                                    let res = self.conversations.insert(&mut c_conversation);
                                    if let Err(res) = res {
                                        println!("deleting {:?}", res)
                                    }

                                    return;
                                }
                            } else {
                                println!(
                                    "failed to read conversation {},{:?}",
                                    convo_id, conversation
                                )
                            }
                        } else {
                            ui.label("Select a conversation to begin");
                        }
                    });
                });

                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .max_width(500.0)
                        .id_source("data_info")
                        .show(ui, |ui| {
                            if let Some(ref convo_id) = self.selected_convo {
                                let conversation = self.conversations.get(convo_id);
                                if let Ok(Some(conversation)) = conversation {
                                    match conversation.to_training_data() {
                                        Ok(data) => {
                                            for datum in data {
                                                ui.group(|ui| {
                                                    ui.add(
                                                        egui::TextEdit::multiline(
                                                            &mut datum.clone(),
                                                        )
                                                        .text_style(egui::TextStyle::Body),
                                                    )
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            println!("failed to gen data {}", e)
                                        }
                                    }
                                }
                            }
                        });
                });

                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.heading("Model");
                        match self.server_manager.inference_server {
                            Some(ref is) => {
                                ui.label("Inference server");
                                let status = is.lock().unwrap().status().cloned();
                                if status.is_err() {
                                    ui.label(format!("Status error {:?}", status));
                                    return;
                                }
                                let status = status.unwrap();
                                ui.label(format!("Status: {}", status.to_string()));
                                match status {
                                    ServerStatus::Generating { text } => {
                                        ui.label(text);
                                        if ui.button("stop").clicked() {
                                            is.lock().unwrap().stop().unwrap();
                                        }
                                    }
                                    ServerStatus::DoneGenerating { text } => {
                                        ui.label(text);
                                        if ui.button("infer!").clicked() {
                                            println!("inferclicked");
                                            let resp = is
                                                .lock()
                                                .unwrap()
                                                .infer(InferReq {
                                                    prompt: "the average".to_owned(),
                                                    config: GenerationConfig::default(),
                                                })
                                                .unwrap();
                                            println!("{:?}", resp)
                                        }
                                    }
                                    ServerStatus::Ready {} => {
                                        if ui.button("infer!").clicked() {
                                            println!("inferclicked");
                                            let resp = is
                                                .lock()
                                                .unwrap()
                                                .infer(InferReq {
                                                    prompt: "the length of".to_owned(),
                                                    config: GenerationConfig::default(),
                                                })
                                                .unwrap();
                                            println!("{:?}", resp)
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            None => {
                                if ui.button("start server").clicked() {
                                    let res =
                                        self.server_manager.start_inference(&InferenceServerArgs {
                                            model_config: ".".into(),
                                            image_name: "fuck".into(),
                                            port: 9090,
                                        });
                                    dbg!(&res);
                                };
                            }
                        }
                    });
                });

                ui.vertical(|ui| {
                    ui.group(|ui| {
                        if ui.button("export data").clicked() {
                            let mut file = File::create("data.jsonl").unwrap();

                            for (_, c) in self.conversations.clone().into_iter() {
                                c.write_jsonl(&mut file).unwrap()
                            }
                        }
                    });
                });
            });
        });
    }
}
