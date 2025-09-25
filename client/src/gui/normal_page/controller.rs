use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use iced::Task;
use shared::tasks::{AdminCommand, WyrmResult};
use shared_c2_client::StagedResourceData;

use crate::{
    gui::{
        Message, Page,
        new_tasks::get_agents,
        normal_page::model::{
            ConnectionState, NPM, NormalPage, NormalPageMessage, SelectedBottomTab,
        },
        staging::{stage_all::StageAllPage, stage_upload::StageUploadPage},
    },
    net::{IsTaskingAgent, api_request},
    state::{Agent, Cli, TabConsoleMessages},
    tasks::task_dispatcher::dispatch_task,
};

impl NormalPage {
    pub fn controller_impl(&mut self, message: Message) -> (Option<Box<dyn Page>>, Task<Message>) {
        if let Message::NormalPage(msg) = message {
            match msg {
                NormalPageMessage::PollConnectedAgents => {
                    // Send the request off to the C2, the response will be done in
                    // the Message (event): `FetchedAgents`
                    let task = self.fetch_agents();
                    return (None, task);
                }
                NormalPageMessage::FetchedAgents(incoming_agents) => {
                    //
                    // We first check whether there are any connected agents on the C2, or whether we have no connection.
                    //

                    let incoming_agents_lock = incoming_agents.read().unwrap();
                    let incoming_agents_lock = match &*incoming_agents_lock {
                        Ok(lock) => match lock {
                            Some(lock) => {
                                if lock.is_empty() {
                                    let mut self_lock = self.connected_agents.write().unwrap();
                                    if let Some(inner) = self_lock.as_mut() {
                                        *inner = HashMap::new();
                                    }

                                    self.connection_state = ConnectionState::Connected;
                                    return (None, Task::none());
                                }

                                lock
                            }
                            None => {
                                let mut local_lock = self.connected_agents.write().unwrap();
                                *local_lock = None;
                                return (None, Task::none());
                            }
                        },
                        Err(_) => {
                            self.connection_state = ConnectionState::Disconnected;
                            return (None, Task::none());
                        }
                    };

                    self.connection_state = ConnectionState::Connected;

                    //
                    // If the agent exists in the current hashmap, we only want to update its last checkin
                    // time.
                    // If the agent does not exist, then we can insert it.
                    //

                    let mut self_lock = self.connected_agents.write().unwrap();
                    for incoming_agent in incoming_agents_lock {
                        // Do we have any agents in our internal list?
                        if let Some(self_lock) = self_lock.as_mut() {
                            // Yes - does the list contain the agent? If yes - just update the time,
                            // if no, insert the new agent
                            if let Some(agent_record) = self_lock.get_mut(incoming_agent.0) {
                                agent_record.last_check_in = incoming_agent.1.last_check_in;
                                agent_record.is_stale = incoming_agent.1.is_stale;
                            } else {
                                self_lock
                                    .insert(incoming_agent.0.clone(), incoming_agent.1.clone());
                            }
                        } else {
                            // No - we had no local agents, so insert a new one
                            let mut hm: HashMap<String, Agent> = HashMap::new();
                            hm.insert(incoming_agent.0.clone(), incoming_agent.1.clone());
                            *self_lock = Some(hm);
                        }
                    }

                    //
                    // Now we want to remove local copies of agents which do not exist on the server
                    //
                    let mut id_to_remove: Vec<String> = vec![];

                    for agent in self_lock.as_mut().unwrap() {
                        if !incoming_agents_lock.contains_key(agent.0) {
                            id_to_remove.push(agent.0.clone());
                        }
                    }

                    for key in id_to_remove {
                        self_lock.as_mut().unwrap().remove(&key);
                    }
                }
                NormalPageMessage::AgentSelectFromTopPanel(id) => {
                    self.agents_as_tabs.insert(id.clone());
                    self.selected_bottom_tab = SelectedBottomTab::Agent(id)
                }
                NormalPageMessage::BottomSectionButtonClick(selected_bottom_tab) => {
                    // if we opened the staged resources tab, start the http request to get the resources

                    self.selected_bottom_tab = selected_bottom_tab;
                    // clear the input box
                    let _ = std::mem::take(&mut self.user_input);

                    if self.selected_bottom_tab == SelectedBottomTab::StagedResources {
                        let task = self.get_staged_resources();
                        return (None, task);
                    }
                }
                NormalPageMessage::CloseBottomTab(selected_bottom_tab) => {
                    if let SelectedBottomTab::Agent(id) = selected_bottom_tab {
                        let _ = self.agents_as_tabs.remove(&id);
                    }

                    // Move back to the staged resources which will act as a default view
                    self.selected_bottom_tab = SelectedBottomTab::StagedResources;
                }
                NormalPageMessage::SendCommandFromInput => {
                    let user_input: String = std::mem::take(&mut self.user_input);

                    if user_input.is_empty() {
                        return (None, Task::none());
                    }

                    // Add the input into the console
                    if let SelectedBottomTab::Agent(agent_id) = &self.selected_bottom_tab {
                        let mut lock = self.connected_agents.write().unwrap();
                        let agent = lock.as_mut().unwrap().get_mut(agent_id);
                        if let Some(agent) = agent {
                            agent.output_messages.push(TabConsoleMessages {
                                event: "UserConsoleInput".into(),
                                time: "".into(),
                                messages: Some(vec![format!("{}", user_input)]),
                            });
                        }
                    }

                    // Create a new `Cli`, which comes from the legacy approach to this project. This is required
                    // to interact with the API. Rather than rewriting the ENTIRE thing, I have decided to just
                    // refactor the Cli into something that can work for this.
                    let uid = match &self.selected_bottom_tab {
                        SelectedBottomTab::StagedResources => "".into(),
                        SelectedBottomTab::Agent(uid) => uid.clone(),
                    };

                    let mut cli = Cli::from_page(
                        uid,
                        self.connected_agents.clone(),
                        self.credentials.clone(),
                    );

                    let task = iced::Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || dispatch_task(user_input, &mut cli))
                                .await
                                .unwrap()
                        },
                        move |_| Message::NormalPage(NPM::DoNothing),
                    );

                    return (None, task);
                }
                NormalPageMessage::UserInputUpdated(input) => self.user_input = input,
                NormalPageMessage::RefreshResources => {
                    let task: Task<Message> = self.get_staged_resources();

                    return (None, task);
                }
                NormalPageMessage::DoNothing => (),
                NormalPageMessage::ReceiveStagedResources(response) => {
                    self.staged_resources = response;
                }
                NormalPageMessage::DeleteStagedResource(download_endpoint) => {
                    // todo this will silently fail - but it should be okay (connection and C2 uptime permitted..) as
                    // teh agent ID should be guaranteed
                    let creds = (*self.credentials).clone();
                    let task = iced::Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let result = api_request(
                                    AdminCommand::DeleteStagedResource(download_endpoint),
                                    IsTaskingAgent::No,
                                    &creds,
                                );

                                match result {
                                    Ok(result) => {
                                        match serde_json::from_slice::<
                                            WyrmResult<Vec<StagedResourceData>>,
                                        >(&result)
                                        {
                                            Ok(r) => r,
                                            Err(e) => {
                                                WyrmResult::Err(format!("Error deserialising, {e}"))
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        WyrmResult::Err(format!("Error with connection, {e}"))
                                    }
                                }
                            })
                            .await
                            .unwrap()
                        },
                        // Automatically call the resource refresh on completion
                        move |_| Message::NormalPage(NPM::RefreshResources),
                    );

                    return (None, task);
                }
                NormalPageMessage::StageFromDiskButton => {
                    return (
                        Some(Box::new(StageUploadPage::new(self.credentials.clone()))),
                        Task::none(),
                    );
                }
                NormalPageMessage::StageAllFromProfile => {
                    return (
                        Some(Box::new(StageAllPage::new(self.credentials.clone()))),
                        Task::none(),
                    );
                }
            }
        }

        (None, Task::none())
    }

    /// Attempts to fetch the agents that are currently connected on the C2 by running an iced executor
    /// (but as a tokio blocking task within the async executor [for now]).
    fn fetch_agents(&mut self) -> Task<Message> {
        let credentials = self.credentials.clone();
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || get_agents(credentials))
                    .await
                    .unwrap()
            },
            move |agents| Message::NormalPage(NPM::FetchedAgents(Arc::new(RwLock::new(agents)))),
        )
    }

    /// Makes an asynchronous request to the C2 to get the staged resources.
    fn get_staged_resources(&self) -> Task<Message> {
        let creds = (*self.credentials).clone();
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let result = api_request(
                        AdminCommand::ListStagedResources,
                        IsTaskingAgent::No,
                        &creds,
                    );

                    match result {
                        Ok(result) => {
                            match serde_json::from_slice::<WyrmResult<Vec<StagedResourceData>>>(
                                &result,
                            ) {
                                Ok(r) => r,
                                Err(e) => WyrmResult::Err(format!("Error deserialising, {e}")),
                            }
                        }
                        Err(e) => WyrmResult::Err(format!("Error with connection, {e}")),
                    }
                })
                .await
                .unwrap()
            },
            move |res| Message::NormalPage(NPM::ReceiveStagedResources(res)),
        )
    }
}
