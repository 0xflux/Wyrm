use std::{collections::HashMap, time::Duration};

use gloo_timers::future::sleep;
use leptos::{IntoView, component, logging::log, prelude::*, reactive::spawn_local, view};
use shared::tasks::AdminCommand;

use crate::{
    controller::{dashboard::update_connected_agents, get_item_from_browser_store},
    models::{
        C2_STORAGE_KEY,
        dashboard::{ActiveTabs, Agent, AgentC2MemoryNotifications},
    },
    net::{IsTaskingAgent, api_request},
    pages::logged_in_headers::LoggedInHeaders,
};

#[component]
pub fn Dashboard() -> impl IntoView {
    //
    // Set up signals across the dashboard
    //
    let (connected_agents, set_connected_agents) =
        signal(HashMap::<String, RwSignal<Agent>>::new());
    provide_context(connected_agents);

    let tabs = RwSignal::new(ActiveTabs::from_store());

    view! {
        // There's got to be a better way of doing this repeating it everywhere, but I cannot find it
        <LoggedInHeaders />

        <ConnectedAgents set_connected_agents tabs />
        <MiddleTabBar tabs />
        <MessagePanel />
    }
}

#[component]
fn ConnectedAgents(
    set_connected_agents: WriteSignal<HashMap<String, RwSignal<Agent>>>,
    tabs: RwSignal<ActiveTabs>,
) -> impl IntoView {
    //
    // Deal with the API request for connected agents
    //
    Effect::new(move || {
        spawn_local(async move {
            loop {
                if let Ok(c2_url) = get_item_from_browser_store::<String>(C2_STORAGE_KEY) {
                    let result = match api_request(
                        AdminCommand::ListAgents,
                        &IsTaskingAgent::No,
                        None,
                        &c2_url,
                        None,
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            leptos::logging::log!("Could not make request for ListAgents. {e}");
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    };

                    let deser_agents: Vec<AgentC2MemoryNotifications> =
                        match serde_json::from_slice(&result) {
                            Ok(r) => r,
                            Err(e) => {
                                leptos::logging::log!("Could not deserialise ListAgents. {e}");
                                sleep(Duration::from_secs(1)).await;
                                continue;
                            }
                        };

                    update_connected_agents(set_connected_agents, deser_agents);
                }

                sleep(Duration::from_secs(1)).await;
            }
        });
    });

    let agent_map =
        use_context::<ReadSignal<HashMap<String, RwSignal<Agent>>>>().expect("no agent map found");

    view! {
        <div id="connected-agent-container" class="container-fluid">

            <div id="agents-header" class="row">
                <div class="col-4">Agent ID</div>
                <div class="col-1">Process ID</div>
                <div class="col-2">Last check-in</div>
                <div class="col-5">Process name</div>
            </div>

            <div id="agent-rows">
                <For
                    each=move || {
                        let mut vals: Vec<RwSignal<Agent>> = agent_map.get().values().cloned().collect();
                        vals
                    }
                    key=|sig| sig.get().agent_id.clone()
                    let:(agent)
                >
                    <a href="#"
                        class=("agent-stale", move || agent.get().is_stale)
                        on:click=move |_| {
                            let mut guard = tabs.write();
                            guard.add_tab(&agent.get().agent_id);
                        }
                    >
                        <div class="row agent-row">
                            <div class="col-4">{ move || agent.get().agent_id }</div>
                            <div class="col-1">{ move || agent.get().pid }</div>
                            <div class="col-2">{ move || agent.get().last_check_in.to_string() }</div>
                            <div class="col-5">{ move || agent.get().process_name }</div>
                        </div>
                    </a>
                </For>
            </div>
        </div>
    }
}

#[component]
fn MiddleTabBar(tabs: RwSignal<ActiveTabs>) -> impl IntoView {
    view! {
        <div class="tabbar">
            <ul id="tab-bar-ul" class="nav nav-tabs flex-nowrap text-nowrap m-0 px-20">
                <li class="nav-item d-flex align-items-center">
                    <a
                        class="nav-link"
                        class:active=move || tabs.read().active_id.is_none()
                        href="#"
                        on:click=move |_| {
                            let mut guard = tabs.write();
                            (*guard).active_id = None
                        }
                    >
                        "Server"
                    </a>
                </li>
                <For
                    each=move || {
                        let s: Vec<String> = tabs.read().tabs.iter().cloned().collect();
                        s
                    }
                    key=|tab| tab.clone()
                    children=move |tab: String| {
                        view! {
                            <li class="nav-item d-flex align-items-center">
                                <a
                                    class="nav-link"
                                    class:active={
                                        let value = tab.clone();
                                        move || match tabs.read().active_id.clone()  {
                                            Some(tab_id) => {
                                                tab_id.eq(&value)
                                            },
                                            None => false,
                                        }
                                    }
                                    href="#"
                                    on:click={
                                        let value = tab.clone();
                                        move |_| {
                                            let mut guard = tabs.write();
                                            (*guard).active_id = Some(value.clone())
                                        }
                                    }
                                >
                                    {tab.clone()}
                                </a>

                                <button
                                    on:click=move |_| {
                                        let mut guard = tabs.write();
                                        (*guard).remove_tab(&tab.clone());
                                    }
                                    class="btn btn-sm btn-close ms-2"
                                    aria-label="Close"
                                    name="index"
                                    style="font-size:0.6rem;"></button>
                            </li>
                        }
                    }
                />
            </ul>
        </div>
    }
}

#[component]
fn MessagePanel() -> impl IntoView {
    view! {
        <div id="message-panel" class="container-fluid"
            hx-get="/api/dashboard/show_messages"
            hx-trigger="load, every 300ms"
            hx-target="#message-panel"
            hx-swap="innerHTML"></div>


        <div id="input-strip" class="d-flex align-items-center px-3">
            <span class="me-2">>></span>
            <form hx-post="/api/dashboard/send_command"
                hx-swap="none"
                hx-on::after-request="this.querySelector('#cmd_input').value=''"
                autocomplete="off"
                class="d-flex flex-grow-1">
                    <input id="cmd_input" name="cmd_input" type="text" class="flex-grow-1" placeholder="Type a command..." />
                    <button class="btn btn-sm btn-secondary btn-block">Send</button>
            </form>
        </div>
    }
}
