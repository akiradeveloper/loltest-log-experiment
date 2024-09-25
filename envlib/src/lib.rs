use anyhow::Result;
use std::collections::HashMap;
use std::sync::Once;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::info;

mod proto {
    tonic::include_proto!("my_service");
}
use proto::my_service_client::MyServiceClient;

#[derive(Debug)]
struct App;
#[tonic::async_trait]
impl proto::my_service_server::MyService for App {
    async fn ping(&self, _: tonic::Request<()>) -> Result<tonic::Response<()>, tonic::Status> {
        info!("ping");
        Ok(tonic::Response::new(()))
    }
}

static INIT: Once = Once::new();

struct Node {
    id: u8,
    port: u16,
    abort_tx0: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Node {
    pub fn new(id: u8, port: u16) -> Result<Self> {
        let nd_tag = format!("ND{port}>");
        let (tx, rx) = tokio::sync::oneshot::channel();

        let svc_task = async move {
            info!("add (id={id})");

            let svc = proto::my_service_server::MyServiceServer::new(App);
            let socket = format!("127.0.0.1:{port}").parse().unwrap();
            tonic::transport::Server::builder()
                .add_service(svc)
                .serve_with_shutdown(socket, async {
                    info!("remove (id={id})");
                    rx.await.ok();
                })
                .await
                .unwrap();
        };

        std::thread::Builder::new()
            .name(nd_tag.clone())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .thread_name(nd_tag)
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(svc_task);
            })
            .unwrap();

        Ok(Self {
            id,
            port,
            abort_tx0: Some(tx),
        })
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        let tx = self.abort_tx0.take().unwrap();
        tx.send(()).ok();
    }
}
pub struct Env {
    nodes: HashMap<u8, Node>,
}
impl Env {
    pub fn new() -> Self {
        INIT.call_once(|| {
            let format = tracing_subscriber::fmt::format()
                .with_target(false)
                .with_thread_names(true)
                .compact();
            tracing_subscriber::fmt().event_format(format).init();
        });
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: u8) {
        let free_port = port_check::free_local_ipv4_port().unwrap();
        let node = Node::new(id, free_port).unwrap();
        self.nodes.insert(id, node);
    }

    pub fn remove_node(&mut self, id: u8) {
        if let Some(node) = self.nodes.remove(&id) {}
    }

    pub async fn connect_ping_client(&self, id: u8) -> Result<MyServiceClient<Channel>> {
        let port = self.nodes.get(&id).unwrap().port;
        let address = format!("http://127.0.0.1:{port}");
        let cli = MyServiceClient::connect(address).await?;
        Ok(cli)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_1() {
        let mut env = Env::new();
        env.add_node(1);
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut cli = env.connect_ping_client(1).await.unwrap();
        cli.ping(()).await.unwrap();

        env.remove_node(1);
        env.add_node(1);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    async fn test_env_2() {
        let mut env = Env::new();
        for id in 0..=255 {
            env.add_node(id);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
