use std::{
    fs::File,
    sync::Arc,
    time::{Duration, SystemTime},
};

use eframe::{egui, HardwareAcceleration};
use egui::{Ui, Widget, WidgetInfo, TextStyle, Style};

use crate::{
    conversation::{Conversation, ConversationAction, Conversations, Message, Metadata, User},
    model_server::{GenerationConfig, InferReq, InferenceServerArgs, ServerManager, ServerStatus},
    nexos::{extract_commands, LogLine, NexosInstance},
};
pub fn launch_gui(db: String) -> anyhow::Result<()> {
    let db = jammdb::DB::open(db).unwrap();
    let mut conversations = Conversations::new(Arc::new(db), None).unwrap();
    let options = eframe::NativeOptions {
        // initial_window_size: Some(egui::vec2(300.0, 240.0)),
        // hardware_acceleration: HardwareAcceleration::,
        ..Default::default()
    };
    eframe::run_native(
        "Jake",
        options,
        Box::new(|cc| {
            // This gives us image support:
            // egui_extras::install_image_loaders(&cc.egui_ctx);

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
            let mut style = Style::default();
            style.override_text_style = Some(TextStyle::Monospace);
            ui.set_style(style);

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
                        let mut conversations = self
                            .conversations
                            .clone()
                            .into_iter()
                            .collect::<Vec<(String, Conversation)>>();

                        conversations.sort_by(|(_, b), (_, a)| {
                            a.time
                                .partial_cmp(&b.time)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });

                        for (u, _) in conversations {
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
                    egui::ScrollArea::vertical()
                        .id_source("message_info")
                        .show(ui, |ui| {
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
                                if let Ok(Some(mut conversation)) = conversation {
                                    let mut action: Option<ConversationAction> = None;
                                    for (i, msg) in conversation.messages.iter().enumerate() {
                                        let mut msg = msg.clone();
                                        ui.group(|ui| {
                                            if ui.button("delete").clicked() {
                                                action = Some(ConversationAction::DeleteMessage {
                                                    id: msg.id.clone(),
                                                });
                                            }
                                            let datetime: chrono::DateTime<chrono::offset::Utc> =
                                                msg.time.clone().into();
                                            ui.label(datetime.format("%Y-%m-%d %T").to_string());
                                            ui.label(format!("{}:", msg.user.to_string()));
                                            let output = egui::TextEdit::multiline(&mut msg.msg)
                                                .hint_text("Type something!")
                                                .desired_width(1000.0)
                                                .show(ui);
                                            if ui.button("eval").clicked() {
                                                action = Some(ConversationAction::EvalMessage {
                                                    id: msg.id.clone(),
                                                });
                                            }
                                            if msg.user == User::Jake {
                                                ui.checkbox(
                                                    &mut msg.meta.exclude_from_training,
                                                    "exclude",
                                                );
                                                if let Some(ref is) =
                                                    self.server_manager.inference_server
                                                {
                                                    ui.label("Inference server");
                                                    let status =
                                                        is.lock().unwrap().status().cloned();
                                                    if status.is_err() {
                                                        ui.label(format!(
                                                            "Status error {:?}",
                                                            status
                                                        ));
                                                        return;
                                                    }
                                                    let status = status.unwrap();
                                                    if let ServerStatus::Ready { .. }
                                                    | ServerStatus::DoneGenerating { .. } =
                                                        status
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

                                                    if let ServerStatus::DoneGenerating {
                                                        text: val,
                                                    } = status
                                                    {
                                                        if ui.button("copy into").clicked() {
                                                            msg.msg = String::from(val);
                                                        }
                                                    }
                                                };
                                            };
                                        });
                                        if msg != conversation.messages[i] {
                                            action = Some(ConversationAction::MutateMessage {
                                                new_message: msg,
                                            })
                                        }
                                    }
                                    if ui.button("+ User").clicked() {
                                        action = Some(ConversationAction::AddMessage {
                                            index: None,
                                            user: User::Zack,
                                        });
                                    }
                                    if ui.button("+ Assistant").clicked() {
                                        action = Some(ConversationAction::AddMessage {
                                            index: None,
                                            user: User::Jake,
                                        });
                                    }
                                    if let Some(action) = action {
                                        conversation.apply(action).unwrap();
                                        let res = self.conversations.insert(&mut conversation);
                                        if let Err(res) = res {
                                            println!("err saving {:?}", res)
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
                                        egui::ScrollArea::vertical()
                                            .max_width(500.0)
                                            .id_source("generating")
                                            .show(ui, |ui| {
                                                ui.label(text);
                                            });

                                        if ui.button("stop").clicked() {
                                            is.lock().unwrap().stop().unwrap();
                                        }
                                    }
                                    ServerStatus::DoneGenerating { text } => {
                                        egui::ScrollArea::vertical()
                                            .max_width(500.0)
                                            .id_source("done_generating")
                                            .show(ui, |ui| {
                                                ui.label(text);
                                            });
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
