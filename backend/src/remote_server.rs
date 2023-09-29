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
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferResp {
    pub completion: String,
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
