use std::fmt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::Context;

use crate::templates;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferReq {
    pub prompt: String,
    pub config: GenerationConfig,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GenerationConfig {
    /// Penalty applied to repeating tokens.
    pub repetition_penalty: f64,

    /// Maximum number of new tokens to generate.
    pub max_new_tokens: usize,

    /// A value between 0 and 1 that controls randomness in output.
    /// Lower values make the output more deterministic.
    pub temperature: f64,

    /// A value between 0 and 1 that controls the fraction of the
    /// probability mass to consider in sampling. 1.0 means consider all.
    pub top_p: f64,

    /// The number of highest probability tokens to keep for sampling.
    pub top_k: usize,

    /// Whether to sample from the model's output distribution or just
    /// take the maximum probability token.
    pub do_sample: bool,

    /// Whether to use the model's cache in generation.
    pub use_cache: bool,

    /// Whether to return a dictionary structure in generation.
    pub return_dict_in_generate: bool,

    /// Whether to output attention values in generation.
    pub output_attentions: bool,

    /// Whether to output hidden state values in generation.
    pub output_hidden_states: bool,

    /// Whether to output scores in generation.
    pub output_scores: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        GenerationConfig {
            repetition_penalty: 1.1,
            max_new_tokens: 2000,
            temperature: 0.05,
            top_p: 0.95,
            top_k: 40,
            do_sample: true,
            use_cache: true,
            return_dict_in_generate: true,
            output_attentions: false,
            output_hidden_states: false,
            output_scores: false,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StatusReq {}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StatusResp {
    #[serde(flatten)]
    pub body: ServerStatus,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StopReq {}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StopResp {}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferResp {}
#[derive(Default)]
pub struct ServerManager {
    pub inference_server: Option<Arc<Mutex<InferenceServer>>>,
}
impl ServerManager {
    pub fn start_inference(&mut self, args: &InferenceServerArgs) -> anyhow::Result<()> {
        match self.inference_server {
            Some(_) => {}
            None => {
                let srv =
                    InferenceServer::start(args).context("failed to start inference server")?;
                self.inference_server = Some(Arc::new(Mutex::new(srv)))
            }
        }
        Ok(())
    }
}

pub struct InferenceServer {
    process_handle: std::process::Child,
    config: InferenceServerArgs,
    status: ServerStatus,
    status_refresh_time: SystemTime,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InferenceServerArgs {
    // will be linked
    pub model_config: PathBuf,
    pub image_name: String,
    pub port: usize,
}
// The empty brackets are important so that serde includes them as an empty map
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, strum_macros::Display)]
#[serde(tag = "status", content = "body")]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Starting {},
    Loading {},
    Generating { text: String },
    DoneGenerating { text: String },
    Ready {},
    Busy {},
    Dead {},
}

impl InferenceServer {
    pub fn start(args: &InferenceServerArgs) -> anyhow::Result<Self> {
        let command = templates::start_inference(args).context("start inference")?;
        println!("running {}", &command);
        let handle = Command::new("bash")
            .arg("-c")
            .arg(command)
            // .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .spawn()
            .context("command spawn")?;
        Ok(Self {
            process_handle: handle,
            config: args.clone(),
            status: ServerStatus::Starting {},
            status_refresh_time: SystemTime::now(),
        })
    }
    pub fn status(&mut self) -> anyhow::Result<&ServerStatus> {
        if SystemTime::now()
            .duration_since(self.status_refresh_time)
            .unwrap_or_else(|_| Duration::from_secs(0))
            > Duration::from_secs(1)
        {
            self.status_refresh_time = SystemTime::now();
            self.status = self.internal_get_status()?;
        }
        Ok(&self.status)
    }
    pub fn infer(&mut self, body: InferReq) -> anyhow::Result<InferResp> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        // Call the asynchronous connect method using the runtime.
        return rt.block_on(self.inferreq(body));
    }

    pub fn stop(&mut self) -> anyhow::Result<StopResp> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        // Call the asynchronous connect method using the runtime.
        return rt.block_on(self.stop_req(StopReq {}));
    }
    fn internal_get_status(&mut self) -> anyhow::Result<ServerStatus> {
        println!("checking status");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        // Call the asynchronous connect method using the runtime.
        let resp = rt.block_on(self.status_req());
        if let Ok(resp) = resp {
            return Ok(resp.body);
        } else {
            dbg!(&resp);
        }

        if self
            .process_handle
            .try_wait()
            .context("try wait")?
            .is_some()
        {
            Ok(ServerStatus::Dead {})
        } else {
            Ok(ServerStatus::Starting {})
        }
    }
    pub async fn status_req(&mut self) -> anyhow::Result<StatusResp> {
        runreq(self.get_url(), "status", StatusReq {}).await
    }
    pub async fn inferreq(&mut self, body: InferReq) -> anyhow::Result<InferResp> {
        runreq(self.get_url(), "infer", body).await
    }

    pub async fn stop_req(&mut self, body: StopReq) -> anyhow::Result<StopResp> {
        runreq(self.get_url(), "stop", body).await
    }
    fn get_url(&self) -> String {
        format!("http://localhost:{}", self.config.port)
    }
    pub fn shutdown(mut self) -> anyhow::Result<()> {
        Ok(self.process_handle.kill().context("process kill")?)
    }
}

pub async fn runreq<S: AsRef<str>, B: serde::Serialize, T: for<'de> serde::Deserialize<'de>>(
    url: String,
    route: S,
    req: B,
) -> anyhow::Result<T> {
    println!("Running {}", route.as_ref());
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/{}", url.clone(), route.as_ref()))
        .json(&req)
        .timeout(Duration::from_secs(1))
        .send()
        .await
        .context("failed request")?;
    if res.status() != reqwest::StatusCode::OK {
        println!("Getting data failed");
        anyhow::bail!("request failed with {}", res.status())
    }

    println!("Getting data");
    let full = res.bytes().await?;

    println!("unmarshalling data");
    let json = serde_json::from_slice(&full).with_context(|| {
        format!(
            "failed to unmarshall {:?}",
            String::from_utf8(full.to_vec()).unwrap()
        )
    })?;
    println!("done data");
    Ok(json)
}
