mod remote_server;
use remote_server::*;
#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let mut serv = InferenceServer::new("http://localhost:9090");
    let start = serv.start(StartReq {}).await;
    println!("{:?}", start);

    let infer = serv
        .infer(InferReq {
            prompt: "Hi!".into(),
        })
        .await;
    println!("{:?}", infer);
}
