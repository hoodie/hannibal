//! # Process Actor Example
//!
//! This example demonstrates a hierarchical actor system where an `IntakeActor` manages
//! multiple `ProcessActor` children. The `IntakeActor` is configured with a shell command
//! and receives `File` messages containing file paths. For each file that exists, it spawns
//! a `ProcessActor` child that:
//!
//! 1. Executes the configured command on the file (e.g., "cat", "wc", "grep pattern")
//! 2. Processes the command's output line-by-line as a stream
//! 3. Stores all received lines internally
//! 4. Provides status updates via `Handler<()>`
//!
//! The example showcases:
//! - Parent-child actor relationships using `ctx.add_child()`
//! - Stream processing with `StreamHandler<String>`
//! - Message handling between actors
//! - Process spawning and stdout streaming
//! - Lifecycle management (children stop when parent stops)

use async_process::{Command, Stdio};
use futures_lite::{io::BufReader, prelude::*};
use std::path::Path;

use hannibal::{OwningAddr, prelude::*};

// Message for sending file paths to IntakeActor
#[derive(Debug, Clone, Message)]
struct File(String);

// ProcessActor that stores lines internally
#[derive(Debug)]
struct ProcessActor {
    lines: Vec<String>,
    file_path: String,
}

impl ProcessActor {
    fn new(file_path: String) -> Self {
        Self {
            lines: Vec::new(),
            file_path,
        }
    }
}

impl Actor for ProcessActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("ProcessActor started for file: {}", self.file_path);
        Ok(())
    }

    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!(
            "ProcessActor stopped for file: {} with {} lines stored",
            self.file_path,
            self.lines.len()
        );
    }
}

impl StreamHandler<String> for ProcessActor {
    async fn handle(&mut self, _ctx: &mut Context<Self>, line: String) {
        println!("ProcessActor[{}] received: {}", self.file_path, line);
        self.lines.push(line);
    }
}

impl Handler<()> for ProcessActor {
    async fn handle(&mut self, _: &mut Context<Self>, _: ()) {
        println!(
            "ProcessActor[{}] status: {} lines collected",
            self.file_path,
            self.lines.len()
        );
    }
}

// IntakeActor that manages ProcessActor children
#[derive(Debug)]
struct IntakeActor {
    command: String,
}

impl IntakeActor {
    fn new(command: String) -> Self {
        Self { command }
    }

    fn launch_process(&mut self, file_path: String) -> Option<OwningAddr<ProcessActor>> {
        let mut child = match Command::new(&self.command)
            .arg(&file_path)
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                println!("Failed to spawn command for file {}: {}", file_path, e);
                return None;
            }
        };
        let lines = BufReader::new(child.stdout.take().unwrap())
            .lines()
            .filter_map(Result::ok);
        let process_actor = ProcessActor::new(file_path.clone());
        let child_addr = hannibal::build(process_actor)
            .on_stream(lines)
            .spawn_owning();
        Some(child_addr)
    }
}

impl Actor for IntakeActor {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> DynResult<()> {
        println!("IntakeActor started with command: {}", self.command);
        Ok(())
    }

    async fn stopped(&mut self, _: &mut Context<Self>) {
        println!("IntakeActor stopped");
    }
}

#[derive(Debug, Message)]
struct ProcessResult(Vec<String>);
impl Handler<ProcessResult> for IntakeActor {
    async fn handle(&mut self, _: &mut Context<Self>, msg: ProcessResult) {
        println!("Received ProcessResult: {:?}", msg.0);
    }
}

impl Handler<File> for IntakeActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, File(file_path): File) {
        println!("IntakeActor checking file: {}", file_path);

        // Check if file exists
        if !Path::new(&file_path).exists() {
            println!("File does not exist: {}", file_path);
            return;
        }

        println!("File exists, spawning ProcessActor for: {}", file_path);

        let mut child_addr = match self.launch_process(file_path) {
            Some(value) => value,
            None => return,
        };

        // Add the child to this actor's context
        ctx.add_child(child_addr.as_ref());

        let child_joined = child_addr.join();
        let self_sender = ctx.weak_sender();
        ctx.spawn_task(async move {
            if let Some((process_result, sender)) = child_joined.await.zip(self_sender.upgrade()) {
                sender
                    .send(ProcessResult(process_result.lines))
                    .await
                    .unwrap()
            }
        });
    }
}

#[hannibal::main]
async fn main() -> DynResult<()> {
    env_logger::init();

    // Create IntakeActor with "cat" command
    let intake = IntakeActor::new("bat".to_string());
    let addr = intake.spawn();

    // Send some file messages
    addr.send(File("Cargo.toml".to_string())).await?;
    addr.send(File("src/lib.rs".to_string())).await?;
    addr.send(File("examples/children.rs".to_string())).await?;
    addr.send(File("nonexistent.txt".to_string())).await?;

    // Wait a bit for processing
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Halt the intake actor (this will also stop all children)
    addr.halt().await?;

    Ok(())
}
