use tokio::sync::{mpsc, oneshot};
use tokio::task;

#[derive(Debug)]
enum Message {
    Greet(&'static str),
    Add(i32, i32, oneshot::Sender<i32>),
    Stop,
}

struct MyActor {
    name: &'static str,
    receiver: mpsc::Receiver<Message>,
}

impl MyActor {
    async fn run(mut self) {
        println!("[Actor {}] started", self.name);

        while let Some(msg) = self.receiver.recv().await {
            match msg {
                Message::Greet(who) => {
                    println!(
                        "[Actor {me}] Hello {you}, my name is {me}",
                        me = self.name,
                        you = who,
                    );
                }
                Message::Add(a, b, responder) => {
                    let result = a + b;
                    let _ = responder.send(result);
                }
                Message::Stop => {
                    println!("[Actor {}] stopped", self.name);
                    break;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel(32);
    let actor = MyActor {
        name: "Caesar",
        receiver: rx,
    };

    let actor_handle = task::spawn(actor.run());

    // Send a Greet message
    tx.send(Message::Greet("Hannibal")).await.unwrap();

    // Send an Add message and wait for the response
    let (resp_tx, resp_rx) = oneshot::channel();
    tx.send(Message::Add(1, 2, resp_tx)).await.unwrap();
    let addition = resp_rx.await.unwrap();
    println!("The Actor Calculated: {:?}", addition);

    // Send a Stop message
    tx.send(Message::Stop).await.unwrap();

    // Wait for the actor to finish
    actor_handle.await.unwrap();
}
