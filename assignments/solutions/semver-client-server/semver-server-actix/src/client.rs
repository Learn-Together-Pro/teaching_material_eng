//! Simple websocket client.
use std::time::Duration;
use std::{io, thread};

use actix::io::SinkWrite;
use actix::*;
use actix_codec::Framed;
use awc::{
    error::WsProtocolError,
    ws::{Codec, Frame, Message},
    BoxedSocket, Client,
};
use bytes::Bytes;
use futures::stream::{SplitSink, StreamExt};
use log::warn;
use semver::{Crate, Program, SemVer};
use semver_api::{Command, Update};
use serde_json::json;

fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    pretty_env_logger::init();

    let sys = System::new("websocket-client");

    Arbiter::spawn(async {
        let (response, framed) = Client::new()
            .ws("http://127.0.0.1:8080/ws/")
            .connect()
            .await
            .map_err(|e| {
                println!("Error: {}", e);
            })
            .unwrap();

        println!("{:?}", response);
        let (sink, stream) = framed.split();
        let addr = ChatClient::create(|ctx| {
            ChatClient::add_stream(stream, ctx);
            ChatClient(SinkWrite::new(sink, ctx))
        });

        // TODO send a number commands via websocket to web server:
        // for example as here: https://github.com/ferrous-systems/teaching-material/blob/master/assignments/solutions/semver-client-server/semver-client/src/main.rs#L14
        //
        // Useful documentation https://docs.rs/actix/0.12.0/actix/prelude/struct.Recipient.html#method.do_send on how to send data
        // see `semver_api::Command` on the different Repository commands

    });
    sys.run().unwrap();
}

struct ChatClient(SinkWrite<Message, SplitSink<Framed<BoxedSocket, Codec>, Message>>);

#[derive(Message)]
#[rtype(result = "()")]
struct ClientCommand(semver_api::Command);

impl Actor for ChatClient {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        // start heartbeats otherwise server will disconnect after 10 seconds
        self.hb(ctx)
    }

    fn stopped(&mut self, _: &mut Context<Self>) {
        println!("Disconnected");

        // Stop application on disconnect
        System::current().stop();
    }
}

impl ChatClient {
    fn hb(&self, ctx: &mut Context<Self>) {
        ctx.run_later(Duration::new(1, 0), |act, ctx| {
            act.0.write(Message::Ping(Bytes::from_static(b"")));
            act.hb(ctx);

            // client should also check for a timeout here, similar to the
            // server code
        });
    }
}

/// Handle stdin commands
impl Handler<ClientCommand> for ChatClient {
    type Result = ();

    fn handle(&mut self, msg: ClientCommand, _ctx: &mut Context<Self>) {
        let text = serde_json::to_string(&msg.0).unwrap();
        self.0.write(Message::Text(text));
    }
}

/// Handle server websocket messages
impl StreamHandler<Result<Frame, WsProtocolError>> for ChatClient {
    fn handle(&mut self, msg: Result<Frame, WsProtocolError>, _: &mut Context<Self>) {
        if let Ok(Frame::Text(txt)) = msg {
            println!("Server: {:?}", txt)
        }
    }

    fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Connected");
    }

    fn finished(&mut self, ctx: &mut Context<Self>) {
        println!("Server disconnected");
        ctx.stop()
    }
}

impl actix::io::WriteHandler<WsProtocolError> for ChatClient {}
