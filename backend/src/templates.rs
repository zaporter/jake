use tera::Tera;

use crate::{
    model_server,
};


#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct MetadataPromptTemplateEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct MessagePromptTemplateEntry {
    pub author: String,
    pub value: String,
}

#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct PromptTemplateData {
    pub meta: Vec<MetadataPromptTemplateEntry>,
    pub msgs: Vec<MessagePromptTemplateEntry>,
    pub response: String
}

#[derive(Clone, PartialEq, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct InjectedFileTemplateData {
    pub meta: Vec<MetadataPromptTemplateEntry>,
    pub filetext: String
}

pub fn get_tera() -> anyhow::Result<Tera> {
    Ok(Tera::new("./templates/*.template")?)
}
pub fn start_inference(config: &model_server::InferenceServerArgs) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render(
        "start_inference.template",
        &tera::Context::from_serialize(config)?,
    )?;
    Ok(result)
}
pub fn prompt(details: &PromptTemplateData) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render("prompt.template", &tera::Context::from_serialize(details)?)?;
    Ok(result)
}

pub fn injested_file(details: &InjectedFileTemplateData) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render("injested_file.template", &tera::Context::from_serialize(details)?)?;
    Ok(result)
}
