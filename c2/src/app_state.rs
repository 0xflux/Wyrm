use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use rand::{Rng, distr::Alphanumeric};
use tokio::{
    sync::{Mutex, RwLock},
    time::sleep,
};

use crate::{
    COOKIE_TTL,
    agents::AgentList,
    db::Db,
    profiles::{Profile, add_listeners_from_profiles, add_tokens_from_profiles},
};

pub struct AppState {
    /// The agents currently connected to the C2 which are able to be interacted with
    pub connected_agents: Arc<AgentList>,
    /// Database pool
    pub db_pool: Db,
    pub endpoints: RwLock<Endpoints>,
    /// Tokens added during the agent creation wizard in which validate agents who are authorised to talk to the C2
    pub agent_tokens: RwLock<HashSet<String>>,
    pub profile: RwLock<Vec<Profile>>,
    sessions: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(Debug)]
pub struct DownloadEndpointData {
    pub file_name: String,
    pub internal_name: String,
    pub xor_key: Option<u8>,
}

impl DownloadEndpointData {
    pub fn new(file_name: &str, internal_name: &str, xor_key: Option<u8>) -> Self {
        Self {
            file_name: file_name.into(),
            internal_name: internal_name.into(),
            xor_key,
        }
    }
}

#[derive(Debug)]
pub struct Endpoints {
    /// API endpoints which can be polled by the agent to check in / get tasks / POST data
    pub c2_endpoints: HashSet<String>,
    /// `HashMap<endpoint, DownloadEndpointData>` - A collection of URI endpoints,
    /// not including a /, which can serve agents over HTTP(s).
    pub download_endpoints: HashMap<String, DownloadEndpointData>,
}

impl AppState {
    pub async fn from(db_pool: Db, profile: Vec<Profile>) -> Self {
        // Fetch the endpoints from the database that we are going to use. If none are setup, it will
        // default to `::new()` for each type.
        let (mut c2_endpoints, download_endpoints, mut agent_tokens) =
            db_pool.get_agent_related_db_cfg().await.unwrap();

        // Add any listener URIs specified in the profile(s)
        add_listeners_from_profiles(&mut c2_endpoints, &profile);
        add_tokens_from_profiles(&mut agent_tokens, &profile);

        let endpoints = Endpoints {
            c2_endpoints,
            download_endpoints,
        };

        let profile = RwLock::new(profile);

        let sessions = Arc::new(Mutex::new(HashMap::new()));

        Self {
            db_pool,
            connected_agents: Arc::new(AgentList::default()),
            endpoints: RwLock::new(endpoints),
            agent_tokens: RwLock::new(agent_tokens),
            profile,
            sessions,
        }
    }

    pub fn track_sessions(&self) {
        let sessions: Arc<Mutex<HashMap<String, Instant>>> = self.sessions.clone();
        tokio::spawn(async move {
            loop {
                let now = Instant::now();
                {
                    let mut lock = sessions.lock().await;
                    lock.retain(|_, value| now.duration_since(*value) < COOKIE_TTL);
                }

                sleep(Duration::from_secs(60)).await;
            }
        });
    }

    pub async fn create_session_key(&self) -> String {
        let mut lock = self.sessions.lock().await;

        // Loop until we generate a unique key (1024 alphanumeric character space) which is not already in the store
        let sid = loop {
            let rng = rand::rng();
            let key: String = rng
                .sample_iter(&Alphanumeric)
                .take(1024)
                .map(char::from)
                .collect();

            if lock.try_insert(key.clone(), Instant::now()).is_ok() {
                break key;
            }
        };

        sid
    }

    /// Determines whether the presented `key` is valid in the current sessions on
    /// the server.
    pub async fn has_session(&self, key: &str) -> bool {
        let lock = self.sessions.lock().await;

        let key = key
            .strip_prefix("session=")
            .expect("could not find prefix session=");

        lock.contains_key(key)
    }

    pub async fn remove_session(&self, key: &str) {
        let mut lock = self.sessions.lock().await;

        let key = key
            .strip_prefix("session=")
            .expect("could not find prefix session=");

        let _ = lock.remove(key);
    }
}

/// Continually monitors for when an agent hasn't checked in after an appropriate period and will automatically remove
/// it from the list of live agents.
pub async fn detect_stale_agents(state: Arc<AppState>) {
    // The duration to sleep the async task which will check whether we need to remove an agent from the
    // live list.
    const LOOP_SLEEP_SECONDS: u64 = 10;

    loop {
        {
            state.connected_agents.mark_agents_stale().await;
            tokio::time::sleep(Duration::from_secs(LOOP_SLEEP_SECONDS)).await;
        }
    }
}
