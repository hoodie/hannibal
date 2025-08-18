use hannibal::prelude::*;

use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
};
use futures::{SinkExt as _, StreamExt as _, stream::SplitSink};
use std::net::SocketAddr;

type WsSender = SplitSink<WebSocket, Message>;
type WsStreamMessage = Result<Message, axum::Error>;

#[derive(Actor)]
pub struct WebsocketConnector {
    sender: WsSender,
}

impl WebsocketConnector {
    pub async fn send(&mut self, msg: &str) {
        if let Err(error) = self.sender.send(msg.into()).await {
            log::warn!("error sending message: {error}");
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

#[allow(clippy::missing_panics_doc)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const ADDR: &str = "127.0.0.1:3000";
    env_logger::init();

    let app = Router::new()
        .route("/ws", axum::routing::get(peer_connected))
        .route(
            "/",
            get(async || format!("please use dev-tools to connect to ws://{ADDR}/ws")),
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
