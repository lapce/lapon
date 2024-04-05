use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use lapon_common::{
    action::{ActionData, ActionMessage},
    node::NodeMessage,
};
use lapon_node::action::data::{self, all_actions};
use lapon_tui::{
    event::AppEvent,
    run::{ActionSection, HostSection, RunPanel},
};
use rcl::runtime::Value;
use uuid::Uuid;

use crate::{
    config::{Config, HostConfig},
    local::start_local,
    remote::{start_remote, SshHost, SshRemote},
};

pub struct Run {
    id: Uuid,
    tx: Sender<AppEvent>,
    hosts: Vec<HostConfig>,
    remote_user: Option<String>,
    actions: Vec<ActionData>,
}

impl Run {
    pub fn from_value(
        cwd: &Path,
        config: &Config,
        value: &Value,
        tx: &Sender<AppEvent>,
    ) -> Result<Self> {
        let Value::Dict(value) = value else {
            return Err(anyhow!("run should be a dict"));
        };

        let mut run = Run {
            id: Uuid::new_v4(),
            tx: tx.clone(),
            hosts: Vec::new(),
            remote_user: None,
            actions: Vec::new(),
        };

        if let Some(value) = value.get(&Value::String("hosts".into())) {
            if let Value::String(v) = value {
                run.hosts.append(&mut config.hosts_from_name(v)?);
            } else if let Value::List(v) = value {
                for host in v.iter() {
                    let Value::String(v) = host else {
                        return Err(anyhow!("hosts should be list of strings"));
                    };
                    run.hosts.append(&mut config.hosts_from_name(v)?);
                }
            } else {
                return Err(anyhow!("hosts should be either string or list"));
            };
        } else {
            run.hosts.push(HostConfig {
                id: Uuid::new_v4(),
                host: "localhost".to_string(),
                vars: HashMap::new(),
            });
        }

        if let Some(value) = value.get(&Value::String("remote_user".into())) {
            let Value::String(remote_user) = value else {
                return Err(anyhow!("remote_user should be string"));
            };
            run.remote_user = Some(remote_user.to_string());
        }

        if let Some(value) = value.get(&Value::String("actions".into())) {
            let actions = data::parse_value(cwd, value)?;
            run.actions = actions;
        }

        Ok(run)
    }

    pub fn execute(&self) -> Result<()> {
        let mut senders = Vec::new();

        for host in &self.hosts {
            let (tx, rx) = if host.host == "localhost" || host.host == "127.0.0.1" {
                start_local()
            } else {
                start_remote(SshRemote {
                    ssh: SshHost {
                        host: host.host.clone(),
                        port: None,
                        user: self.remote_user.clone(),
                    },
                })?
            };
            let (exit_tx, exit_rx) = crossbeam_channel::bounded::<()>(1);

            {
                let tx = self.tx.clone();
                let run_id = self.id;
                let host_id = host.id;
                std::thread::spawn(move || {
                    while let Ok(msg) = rx.recv() {
                        if let ActionMessage::NodeShutdown = &msg {
                            let _ = tx.send(AppEvent::Action {
                                run: run_id,
                                host: host_id,
                                msg,
                            });
                            let _ = exit_tx.send(());
                            return;
                        }
                        let _ = tx.send(AppEvent::Action {
                            run: run_id,
                            host: host_id,
                            msg,
                        });
                    }
                    let _ = exit_tx.send(());
                });
            }

            senders.push((tx, exit_rx))
        }

        let all_actions = all_actions();
        for action_data in &self.actions {
            let Some(_) = all_actions.get(&action_data.action) else {
                return Err(anyhow!("action {} can't be found", action_data.action));
            };
            for (tx, _) in &senders {
                tx.send(NodeMessage::Action(action_data.clone()))?;
            }
        }

        for (tx, _) in &senders {
            tx.send(NodeMessage::Shutdown)?;
        }

        // for (_, rx) in &senders {
        //     let _ = rx.recv();
        // }

        Ok(())
    }

    pub fn to_panel(&self) -> RunPanel {
        let hosts = self
            .hosts
            .iter()
            .map(|host| {
                HostSection::new(
                    host.id,
                    self.actions
                        .iter()
                        .map(|action| ActionSection::new(action.id, action.name.clone()))
                        .collect(),
                )
            })
            .collect();
        RunPanel::new(self.id, hosts)
    }
}
