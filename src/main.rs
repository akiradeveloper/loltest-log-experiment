use anyhow::Result;
use tracing::dispatcher::set_global_default;
use tracing::instrument::WithSubscriber;
use std::time::Duration;
use tracing::{info, subscriber::set_default};
use tracing_subscriber::fmt::{self, layer};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::registry::LookupSpan;
use tracing::Dispatch;
use rand::seq::SliceRandom;
use rand::thread_rng;

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

pub struct PrefixedFormatter<F> {
    inner: F,
    prefix: String,
}

impl<F> PrefixedFormatter<F> {
    pub fn new(inner: F, prefix: String) -> Self {
        Self { inner, prefix }
    }
}

impl<S, N, F> FormatEvent<S, N> for PrefixedFormatter<F>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
    F: FormatEvent<S, N>, // Delegate先のフォーマッタが FormatEvent を実装している
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        write!(writer, "{}> ", self.prefix)?;
        self.inner.format_event(ctx, writer, event)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut free_ports = vec![];
    for _ in 0..10 {
        let p = port_check::free_local_ipv4_port().unwrap();
        free_ports.push(p);
    }
    dbg!(&free_ports);

    for port in free_ports.clone() {
        let format = tracing_subscriber::fmt::format()
            .with_target(false)
            .compact();
        let nd_number = format!("ND{port}");
    

        let svc_task = async move {
            // let _g = set_default(sub);

            let svc = proto::my_service_server::MyServiceServer::new(App);
            let socket = format!("0.0.0.0:{port}").parse().unwrap();
            tonic::transport::Server::builder()
                .add_service(svc)
                .serve(socket)
                .await
                .unwrap();
        };
        // tokio::spawn(svc_task);

        std::thread::spawn(move || {
            let format = PrefixedFormatter::new(format, nd_number);
            let sub = tracing_subscriber::fmt().event_format(format).finish();
            let _g = set_default(sub);
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(svc_task);
        });
    }

    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut rng = thread_rng();
    for _ in 0..10 {
        let tgt_port = free_ports.choose(&mut rng).unwrap();
        // dbg!(tgt_port);
        let mut cli = proto::my_service_client::MyServiceClient::connect(format!(
            "http://localhost:{tgt_port}"
        ))
        .await?;
        cli.ping(()).await?;
    }

    eprintln!("done");
    Ok(())
}