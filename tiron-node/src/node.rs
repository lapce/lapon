use std::{
    collections::HashMap,
    io::{stdin, stdout, BufReader},
};

use anyhow::{anyhow, Result};
use clap::Parser;
use crossbeam_channel::{Receiver, Sender};
use tiron_common::{
    action::{ActionData, ActionMessage, ActionOutputLevel},
    node::NodeMessage,
};

use crate::{
    action::{data::all_actions, Action},
    stdio::stdio_transport,
};

#[derive(Parser)]
#[clap(name = "tiron-node")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {}

pub fn start() -> Result<()> {
    let _ = Cli::parse();
    let (writer_tx, writer_rx) = crossbeam_channel::unbounded::<ActionMessage>();
    let (reader_tx, reader_rx) = crossbeam_channel::unbounded::<NodeMessage>();
    stdio_transport(stdout(), writer_rx, BufReader::new(stdin()), reader_tx);
    mainloop(reader_rx, writer_tx)?;
    Ok(())
}

pub fn mainloop(rx: Receiver<NodeMessage>, tx: Sender<ActionMessage>) -> Result<()> {
    let all_actions = all_actions();
    let mut had_error = false;
    while let Ok(msg) = rx.recv() {
        if had_error {
            continue;
        }
        match msg {
            NodeMessage::Action(action) => match node_run_action(&all_actions, &action, &tx) {
                Ok(result) => {
                    tx.send(ActionMessage::ActionOutputLine {
                        id: action.id,
                        content: format!("successfully {result}"),
                        level: ActionOutputLevel::Success,
                    })?;
                    tx.send(ActionMessage::ActionResult {
                        id: action.id,
                        success: true,
                    })?;
                }
                Err(e) => {
                    tx.send(ActionMessage::ActionOutputLine {
                        id: action.id,
                        content: format!("error: {e:#}"),
                        level: ActionOutputLevel::Error,
                    })?;
                    had_error = true;
                    tx.send(ActionMessage::ActionResult {
                        id: action.id,
                        success: false,
                    })?;
                    tx.send(ActionMessage::NodeShutdown { success: false })?;
                }
            },
            NodeMessage::Shutdown => {
                tx.send(ActionMessage::NodeShutdown { success: true })?;
            }
        }
    }
    Ok(())
}

fn node_run_action(
    all_actions: &HashMap<String, Box<dyn Action>>,
    data: &ActionData,
    tx: &Sender<ActionMessage>,
) -> Result<String> {
    let result = if let Some(action) = all_actions.get(&data.action) {
        let _ = tx.send(ActionMessage::ActionStarted { id: data.id });
        action.execute(data.id, &data.input, tx)?
    } else {
        return Err(anyhow!("can't find action name {}", data.action));
    };
    Ok(result)
}
