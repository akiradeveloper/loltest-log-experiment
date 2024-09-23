use std::time::Duration;
use tracing::{info, span, subscriber::set_default, Level};
use anyhow::Result;
use tracing_futures::Instrument;
use tracing_subscriber::layer::SubscriberExt;

mod proto {
    tonic::include_proto!("my_service");
}
#[derive(Debug)]
struct App;
#[tonic::async_trait]
impl proto::my_service_server::MyService for App {
    async fn ping(&self, _: tonic::Request<()>) -> Result<tonic::Response<()>, tonic::Status> {
        info!("ping");
        Ok(tonic::Response::new(()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {

    let mut free_ports = vec![];
    for _ in 0..20 {
        let p = port_check::free_local_ipv4_port().unwrap();
        free_ports.push(p);
    }
    dbg!(&free_ports);

    for port in free_ports.clone() {
        let format = tracing_subscriber::fmt::format().with_thread_ids(true).with_target(false).compact();
        let sub = tracing_subscriber::fmt().event_format(format).finish();

        let svc_task = async move {
            let _g = set_default(sub);

            let svc = proto::my_service_server::MyServiceServer::new(App);
            let socket = format!("0.0.0.0:{port}").parse().unwrap();
            tonic::transport::Server::builder()
                .add_service(svc)
                .serve(socket).await.unwrap();
        };
        tokio::spawn(svc_task);
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    use rand::seq::SliceRandom;
    use rand::thread_rng;
    let mut rng = thread_rng();
    for _ in 0..20 {
        let tgt_port = free_ports.choose(&mut rng).unwrap();
        dbg!(tgt_port);
        let mut cli = proto::my_service_client::MyServiceClient::connect(format!("http://localhost:{tgt_port}")).await?;
        cli.ping(()).await?;
    }

    Ok(())
}
