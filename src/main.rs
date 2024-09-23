use std::time::Duration;
use tracing::instrument::WithSubscriber;
use tracing::{info, span, subscriber::set_default, Level};
use anyhow::Result;
use tracing_futures::{Instrument};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{layer, Layer};
use tracing::{Event, Subscriber};
use tracing_subscriber::{fmt, layer::Context, Registry};
use tracing_subscriber::fmt::format::{self, Format, Writer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::fmt::{FormatEvent, FormatFields};

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

struct CustomLayer;
impl<S> tracing_subscriber::Layer<S> for CustomLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        println!("ND>");
    }
}

// pub struct PrefixFormatter;

// impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for PrefixFormatter
// where
//     S: tracing::Subscriber + for<'a> LookupSpan<'a>,
//     N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
// {
//     fn format_event(
//         &self,
//         ctx: &fmt::FmtContext<'_, S, N>,
//         mut writer: Writer<'_>,
//         event: &tracing::Event<'_>,
//     ) -> std::fmt::Result {
//         // ログの前に「ND1>」を付加
//         write!(writer, "ND1> ")?;
//         // 標準のログフォーマットを適用
//         ctx.format_fields(writer.by_ref(), event)?;
//         writeln!(writer)
//     }
// }

pub struct PrefixedFormatter<F> {
    inner: F, // Delegate先のフォーマッタ
    prefix: &'static str,
}

impl<F> PrefixedFormatter<F> {
    pub fn new(inner: F, prefix: &'static str) -> Self {
        Self { inner, prefix }
    }
}

impl<S, N, F> FormatEvent<S, N> for PrefixedFormatter<F>
where
    S: tracing::Subscriber + for <'a> LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
    F: FormatEvent<S, N>, // Delegate先のフォーマッタが FormatEvent を実装している
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // ログの前に prefix を出力
        write!(writer, "{}> ", self.prefix)?;
        // Delegate先のフォーマッタに処理を委譲
        self.inner.format_event(ctx, writer, event)
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
        let format = tracing_subscriber::fmt::format().with_target(false).compact();
        let format = PrefixedFormatter::new(format, "ND1");
        // let sub0 = tracing_subscriber::fmt::layer().event_format();
        let sub = tracing_subscriber::fmt().event_format(format).finish();
        // let sub = tracing_subscriber::registry().with(CustomLayer).with_subscriber(sub).into_inner();
        // let sub = sub.with(CustomLayer);
        // let l = CustomLayer;
        // let l2 = l.and_then(sub);

        // let sub = tracing_subscriber::Registry::default().with(CustomLayer).with_subscriber(sub).into_inner();

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
