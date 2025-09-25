use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use shared::tasks::WyrmResult;
use shared_c2_client::StagedResourceData;

use crate::state::{Agent, Credentials};

pub type NPM = NormalPageMessage;

#[derive(Debug, Clone, Default)]
pub struct NormalPage {
    pub credentials: Arc<Credentials>,
    /// An in memory representation of agents connected to the C2. If no agents are connected,
    /// this value will be None.
    pub connected_agents: Arc<RwLock<Option<HashMap<String, Agent>>>>,
    /// Which tab is selected for the bottom pane
    pub selected_bottom_tab: SelectedBottomTab,
    /// Which agents are selected by the user to be a tab, by ID in the HashMap of agents
    pub agents_as_tabs: HashSet<String>,
    pub user_input: String,
    pub staged_resources: WyrmResult<Vec<StagedResourceData>>,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Clone)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Connecting
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SelectedBottomTab {
    StagedResources,
    Agent(String),
}

impl Default for SelectedBottomTab {
    fn default() -> Self {
        Self::StagedResources
    }
}

pub type FetchedAgentsWrapper = Arc<RwLock<Result<Option<HashMap<String, Agent>>, reqwest::Error>>>;

#[derive(Debug, Clone)]
pub enum NormalPageMessage {
    PollConnectedAgents,
    FetchedAgents(FetchedAgentsWrapper),
    AgentSelectFromTopPanel(String),
    BottomSectionButtonClick(SelectedBottomTab),
    CloseBottomTab(SelectedBottomTab),
    SendCommandFromInput,
    UserInputUpdated(String),
    StageAllFromProfile,
    StageFromDiskButton,
    RefreshResources,
    DoNothing,
    ReceiveStagedResources(WyrmResult<Vec<StagedResourceData>>),
    DeleteStagedResource(String),
}

impl NormalPage {
    pub fn new(creds: Arc<Credentials>) -> Self {
        Self {
            credentials: creds,
            ..Default::default()
        }
    }
}
