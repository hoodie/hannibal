use hannibal::{Broker, prelude::*};

use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, Response},
    routing::get,
};
use futures::{SinkExt as _, StreamExt as _, stream::SplitSink};
use std::{
    net::SocketAddr,
    sync::{LazyLock, atomic::AtomicU32},
};

use crate::connector::{Update, WebsocketConnector};

mod connector {
    use super::*;
    type WsSender = SplitSink<WebSocket, Message>;
    type WsStreamMessage = Result<Message, axum::Error>;

    pub struct WebsocketConnector {
        pub sender: WsSender,
    }

    impl Actor for WebsocketConnector {
        async fn started(&mut self, ctx: &mut Context<Self>) -> DynResult<()> {
            ctx.publish(Update::default()).await?;
            ctx.subscribe::<Update>().await?;
            Ok(())
        }
    }

    impl WebsocketConnector {
        pub async fn send(&mut self, msg: &str) {
            if let Err(error) = self.sender.send(msg.into()).await {
                log::warn!("error sending message: {error}");
            }
        }
    }

    impl Handler<Update> for WebsocketConnector {
        async fn handle(&mut self, _: &mut Context<Self>, msg: Update) {
            match msg {
                Update::PeerJoined(msg) => {
                    eprintln!("Peer joined: {}", msg);
                    self.send(&format!("A peer joined, {}", msg)).await;
                }
                Update::PageLoaded => self.send(&format!("somebody loaded the page")).await,
            }
        }
    }

    impl StreamHandler<WsStreamMessage> for WebsocketConnector {
        async fn handle(&mut self, ctx: &mut hannibal::Context<Self>, msg: WsStreamMessage) {
            match msg {
                Ok(Message::Text(text)) => {
                    if text == "end" {
                        self.send("over and out").await;
                        ctx.stop().unwrap();
                    }

                    self.send(&format!("Thanks for your friendly {:?}", text.to_string()))
                        .await
                }

                Ok(Message::Close(_close_frame)) => {
                    log::info!("websocket terminated by other side");
                    if let Err(error) = ctx.stop() {
                        log::error!("error stopping actor: {error}");
                    }
                }
                _ => {}
            };
        }

        async fn finished(&mut self, _ctx: &mut hannibal::Context<Self>) {
            log::info!("websocket stream ended")
        }
    }

    #[derive(Clone, Message)]
    pub enum Update {
        PeerJoined(u32),
        PageLoaded,
    }

    impl Default for Update {
        fn default() -> Self {
            static ID: LazyLock<AtomicU32> = LazyLock::new(|| AtomicU32::new(0));
            Self::PeerJoined(ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }
}

pub async fn peer_connected(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|socket| async move {
        let (sender, messages) = socket.split();
        hannibal::build(WebsocketConnector { sender })
            .on_stream(messages)
            .spawn()
            .await
            .unwrap();
        log::info!("actor ended")
    })
}

#[hannibal::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const ADDR: &str = "127.0.0.1:3000";
    env_logger::init();

    let app = Router::new()
        .route("/ws", axum::routing::get(peer_connected))
        .route(
            "/",
            get(async || {
                Broker::publish(Update::PageLoaded).await.unwrap();
                Html(include_str!("webclient.html"))
            }),
        );

    let listener = tokio::net::TcpListener::bind(ADDR).await?;
    log::debug!("listening on {}", listener.local_addr()?);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
