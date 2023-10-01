use tera::Tera;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummaryMsg {
    pub speaker: String,
    pub msg: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummaryDetails {
    pub messages: Vec<SummaryMsg>,
}

pub fn get_tera() -> anyhow::Result<Tera> {
    Ok(Tera::new("../templates/*.txt")?)
}
pub fn summarize(details: &SummaryDetails) -> anyhow::Result<String> {
    let tera = get_tera()?;
    let result = tera.render(
        "conversation_snippet.txt",
        &tera::Context::from_serialize(details)?,
    )?;
    Ok(result)
}
