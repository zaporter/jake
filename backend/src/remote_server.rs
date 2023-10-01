pub struct InferenceServer {
    url: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum ServerRequestBody {
    Start { req: String },
    Stop,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ServerInfo {
    hostname: String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StartReq {}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StartResp {}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferReq {
    pub prompt: String,
    pub config: GenerationConfig,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferResp {
    pub completion: String,
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

impl InferenceServer {
    pub fn new<S: AsRef<str>>(url: S) -> Self {
        Self {
            url: url.as_ref().into(),
        }
    }

    pub async fn start(&mut self, body: StartReq) -> anyhow::Result<StartResp> {
        self.run("start", body).await
    }

    pub async fn infer(&mut self, body: InferReq) -> anyhow::Result<InferResp> {
        self.run("infer", body).await
    }
    pub async fn run<S: AsRef<str>, B: serde::Serialize, T: for<'de> serde::Deserialize<'de>>(
        &mut self,
        route: S,
        req: B,
    ) -> anyhow::Result<T> {
        let client = reqwest::Client::new();
        let res = client
            .post(format!("{}/{}", self.url.clone(), route.as_ref()))
            .json(&req)
            .send()
            .await?;
        if res.status() != reqwest::StatusCode::OK {
            anyhow::bail!("request failed with {}", res.status())
        }
        let json = res.json().await?;
        Ok(json)
    }
}

// impl tower::Service<ServerRequest> for InferenceServer {
//     fn call(&mut self, req: ServerRequest) -> Self::Future {}
//     fn poll_ready(
//         &mut self,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), Self::Error>> {
//     }
//     type Error = anyhow::Error;
//     type Future  = dyn Future<Output = Result<HttpResponse, Error>>;
// }
