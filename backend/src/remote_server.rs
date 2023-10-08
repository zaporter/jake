use async_trait::async_trait;
use chrono::Duration;
use egui::Ui;
use std::{
    any::Any,
    collections::HashMap,
    fmt::{Debug, Display},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::SystemTime,
};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum ServerRequestBody {
    Start { req: String },
    Stop,
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ServerInfo {
    hostname: String,
}
#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StatusType(&'static str);
impl StatusType {
    const READY: Self = Self("ready");
    const TRAINING: Self = Self("training");
    const LOADING_MODEL: Self = Self("loading_model");
    const INFERING: Self = Self("infering");
    const ERROR: Self = Self("error");
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Status {
    op_id: Option<String>,
    #[serde(flatten)]
    status_body: StatusBody,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "status_code")]
#[serde(rename_all = "snake_case")]
pub enum StatusBody {
    Ready {},
    Training {
        epoch: usize,
        total_epochs: usize,
        loss: f64,
    },
    LoadingModel {},
    Infering {},
    Error {
        msg: String,
    },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StatusReq {}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StatusResp {
    operation_id: String,
    status: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AxolotlConfigOverrides {
    overrides: HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferReq {
    pub prompt: String,
    pub config: GenerationConfig,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InferResp {
    pub completion: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrainReq {}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrainResp {}

pub struct Operation {
    op_id: String,
    req: Box<dyn OperationRequest>,
    response: Option<anyhow::Result<Box<dyn OperationResult + Send>>>,
    status: Option<anyhow::Result<Box<dyn OperationStatus + Send>>>,
}
impl Operation {
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()> {
        ui.label("hi");
        Ok(())
    }
}
#[async_trait]
pub trait OperationRequest: mopa::Any + Debug + Send + Sync {
    async fn run(&self, srv: InferenceServer) -> anyhow::Result<Box<dyn OperationResult + Send>>;
    async fn status(&self, srv: InferenceServer) -> anyhow::Result<Box<dyn OperationStatus>>;
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()>;
}
mopafy!(OperationRequest);

pub trait OperationStatus: Debug + Send + Sync {
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()>;
}

pub trait OperationResult: Debug + Send + Sync {}

#[derive(Debug)]
pub struct LoadModelReq {
    data: String,
}
// impl OperationRequest for LoadModelReq {
//     fn run(&self) -> anyhow::Result<Box<dyn OperationResult>> {
//         anyhow::bail!("unimpl")
//     }
//     fn draw(&self, ui: &mut Ui) -> anyhow::Result<()> {
//         anyhow::bail!("unimpl")
//     }
//     fn status(&self) -> anyhow::Result<Box<dyn OperationStatus>> {
//         anyhow::bail!("unimpl")
//     }
// }
pub struct LoadModelStatus {}
pub struct LoadModelResponse {}
#[derive(Debug, serde::Serialize)]
pub struct InferenceReq {
    pub prompt: String,
}
#[async_trait]
impl OperationRequest for InferenceReq {
    async fn run(&self, srv: InferenceServer) -> anyhow::Result<Box<dyn OperationResult + Send>> {
        let result: InferenceResponse = srv.run("infer", self).await?;
        Ok(Box::new(result))
    }
    async fn status(&self, srv: InferenceServer) -> anyhow::Result<Box<dyn OperationStatus>> {
        let result: InferenceStatus = srv.run("stat_infer", self).await?;
        Ok(Box::new(result))
    }
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()> {
        ui.label("inference");
        Ok(())
    }
}
#[derive(Debug, serde::Deserialize)]
pub struct InferenceStatus {}
impl OperationStatus for InferenceStatus {
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()> {
        ui.label("status");
        Ok(())
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct InferenceResponse {}
impl OperationResult for InferenceResponse {}

pub struct ServerOperationManager {
    inner: Arc<RwLock<ServerOperationManagerInternal>>,
    srv: InferenceServer,
}
pub struct ServerOperationManagerInternal {
    operations: Vec<Arc<RwLock<Operation>>>,
    simple_status: String,
    status_refresh: SystemTime,
}
impl ServerOperationManager {
    fn start_main_thread(&self) -> anyhow::Result<()> {
        let inner = self.inner.clone();
        let srv = self.srv.clone();
        thread::spawn(move || {
            // Process each socket concurrently.
            loop {
                let _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000));
                let mut operation = None;
                {
                    let in_read = inner.read().unwrap();
                    if let Some(op) = in_read.operations.get(0) {
                        operation = Some(Arc::clone(op));
                    }
                }
                if let Some(op) = operation {
                    let res = {
                        let op_read = op.read().unwrap();
                        futures::executor::block_on(op_read.req.run(srv.clone()))
                    };
                    let mut op_write = op.write().unwrap();
                    op_write.response = Some(res);
                }
            }
        });
        Ok(())
    }
    fn draw(&self, ui: &mut Ui) -> anyhow::Result<()> {
        let inner = self.inner.read().unwrap();
        let mut result = Ok(());
        ui.label("ok");
        // ui.group(|ui| {
        //     for op in &inner.operations {
        //         if let Err(e) = op.draw(ui) {
        //             result = Err(e);
        //             return;
        //         }
        //         // match op.req.downcast_ref::<LoadModelReq>() {
        //         //     Some(b) => {
        //         //         println!("{}", b.data)
        //         //     }
        //         //     None => {
        //         //         panic!("fuck")
        //         //     }
        //         // }
        //     }
        // });
        result
    }
}
impl InferenceServer {
    pub fn new<S: AsRef<str>>(url: S) -> Self {
        Self {
            url: url.as_ref().into(),
        }
    }

    pub async fn infer(&mut self, body: InferReq) -> anyhow::Result<InferResp> {
        self.run("infer", body).await
    }

    pub async fn train(&mut self, body: TrainReq) -> anyhow::Result<TrainResp> {
        self.run("train", body).await
    }
    pub async fn run<S: AsRef<str>, B: serde::Serialize, T: for<'de> serde::Deserialize<'de>>(
        &self,
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
