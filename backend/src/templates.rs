use tera::Tera;

use crate::conversation::{self, Conversation};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummaryMsg {
    pub speaker: String,
    pub msg: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummaryDetails {
    pub messages: Vec<SummaryMsg>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptDetails {
    pub conversation: String,
}

pub fn get_tera() -> anyhow::Result<Tera> {
    Ok(Tera::new("../templates/*.template")?)
}
pub fn summarize(details: &SummaryDetails) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render(
        "conversation_snippet.txt",
        &tera::Context::from_serialize(details)?,
    )?;
    Ok(result)
}
pub fn prompt(details : &PromptDetails) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render(
        "prompt.template",
        &tera::Context::from_serialize(details)?,
    )?;
    Ok(result)

}
