use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use gloo_timers::future::sleep;
use leptos::{IntoView, component, html, prelude::*, reactive::spawn_local, view};
use shared::tasks::AdminCommand;

use crate::{
    controller::dashboard::update_connected_agents,
    models::dashboard::{
        ActiveTabs, Agent, AgentC2MemoryNotifications, AgentIdSplit, TabConsoleMessages,
        get_agent_tab_name, get_info_from_agent_id, resolve_tab_to_agent_id,
    },
    net::{C2Url, IsTaskingAgent, api_request},
    pages::logged_in_headers::LoggedInHeaders,
    tasks::task_dispatch::dispatch_task,
};

#[component]
pub fn Dashboard() -> impl IntoView {
    //
    // Set up signals across the dashboard
    //
    let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
        RwSignal::new(HashMap::<String, RwSignal<Agent>>::new());
    provide_context(connected_agents);

    let tabs = RwSignal::new(ActiveTabs::from_store());
    // Providing this as context so we can grab it in the task dispatcher routines dynamically as required
    provide_context(tabs);

    view! {
        // There's got to be a better way of doing this repeating it everywhere, but I cannot find it
        <LoggedInHeaders />

        <ConnectedAgents tabs />
        <MiddleTabBar />
        <MessagePanel />
    }
}

#[component]
fn ConnectedAgents(tabs: RwSignal<ActiveTabs>) -> impl IntoView {
    let connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>> =
        use_context().expect("could not get RwSig connected_agents");

    //
    // Deal with the API request for connected agents
    //
    Effect::new(move || {
        spawn_local(async move {
            loop {
                // If server-side health check shows we are logged out, stop polling.
                if !crate::controller::is_logged_in().await {
                    break;
                }

                let result = match api_request(
                    AdminCommand::ListAgents,
                    &IsTaskingAgent::No,
                    None,
                    C2Url::Standard,
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

                update_connected_agents(connected_agents, deser_agents);

                sleep(Duration::from_secs(1)).await;
            }
        });
    });

    let agent_map =
        use_context::<RwSignal<HashMap<String, RwSignal<Agent>>>>().expect("no agent map found");

    view! {
        <div id="connected-agent-container" class="container-fluid">

            <div id="agents-header" class="row">
                <div class="col-2">Hostname</div>
                <div class="col-2">Username</div>
                <div class="col-1">Integrity</div>
                <div class="col-1">Process ID</div>
                <div class="col-2">Last check-in</div>
                <div class="col-4">Process name</div>
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
                            <div class="col-2">{ move ||
                                {
                                    let id_raw: String = agent.get().agent_id;
                                    let part = get_info_from_agent_id(&id_raw, AgentIdSplit::Hostname).unwrap_or("Error");
                                    part.to_string()
                                }
                            }</div>
                            <div class="col-2">{ move ||
                                {
                                    let id_raw: String = agent.get().agent_id;
                                    let part = get_info_from_agent_id(&id_raw, AgentIdSplit::Username).unwrap_or("Error");
                                    part.to_string()
                                }
                            }</div>
                            <div class="col-1">{ move ||
                                {
                                    let id_raw: String = agent.get().agent_id;
                                    let part = get_info_from_agent_id(&id_raw, AgentIdSplit::Integrity).unwrap_or("Error");
                                    part.to_string()
                                }
                            }</div>
                            <div class="col-1">{ move || agent.get().pid }</div>
                            <div class="col-2">{ move || agent.get().last_check_in.to_string() }</div>
                            <div class="col-4">{ move || agent.get().process_name }</div>
                        </div>
                    </a>
                </For>
            </div>
        </div>
    }
}

#[component]
fn MiddleTabBar() -> impl IntoView {
    let tabs: RwSignal<ActiveTabs> =
        use_context().expect("could not get tabs context in CommandInput()");
    let agent_map: RwSignal<HashMap<String, RwSignal<Agent>>> =
        use_context().expect("no agent map found in MiddleTabBar");

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
                            guard.active_id = None
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
                                    class:active={{
                                        let value = tab.clone();
                                        move || {
                                            let resolved = resolve_tab_to_agent_id(&value, &agent_map.get())
                                                .unwrap_or_else(|| value.clone());
                                            match tabs.read().active_id.clone() {
                                                Some(active) => active == resolved || active == value,
                                                None => false,
                                            }
                                        }
                                    }}
                                    href="#"
                                    on:click={
                                        let value = tab.clone();
                                        move |_| {
                                            let mut guard = tabs.write();
                                            (*guard).active_id = Some(value.clone())
                                        }
                                    }
                                >
                                    {
                                        let label = get_agent_tab_name(&tab).unwrap_or_else(|| tab.clone());
                                        label.clone()
                                    }
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
    let agent_map =
        use_context::<RwSignal<HashMap<String, RwSignal<Agent>>>>().expect("no agent map found");

    let tabs: RwSignal<ActiveTabs> =
        use_context().expect("could not get tabs context in MessagePanel()");

    let messages = Memo::new(move |_| {
        let map = agent_map.get();
        let active_id = tabs.read().active_id.clone();

        let Some(agent_id) = active_id.and_then(|id| resolve_tab_to_agent_id(&id, &map)) else {
            return Vec::<(String, TabConsoleMessages)>::new();
        };

        let Some(agent_sig) = map.get(&agent_id) else {
            return Vec::<(String, TabConsoleMessages)>::new();
        };

        // Track the agent signal so UI updates when its messages change.
        let msgs = agent_sig.with(|agent| agent.output_messages.clone());

        msgs.into_iter()
            .enumerate()
            .map(|(idx, msg)| (format!("{agent_id}-{idx}"), msg))
            .collect::<Vec<(String, TabConsoleMessages)>>()
    });

    let message_panel_ref = NodeRef::<html::Div>::new();
    let should_auto_scroll = RwSignal::new(true);

    let on_scroll = {
        let message_panel_ref = message_panel_ref.clone();
        move |_| {
            if let Some(panel) = message_panel_ref.get() {
                let max_scroll_top = panel.scroll_height() - panel.client_height();
                let near_bottom_threshold = (max_scroll_top - 24).max(0);
                let is_near_bottom = panel.scroll_top() >= near_bottom_threshold;

                should_auto_scroll.set(is_near_bottom);
            }
        }
    };

    Effect::new({
        let message_panel_ref = message_panel_ref.clone();
        move |_| {
            let _ = messages.with(|msgs| msgs.len());

            if !should_auto_scroll.get() {
                return;
            }

            if let Some(panel) = message_panel_ref.get() {
                panel.set_scroll_top(panel.scroll_height());
            }
        }
    });

    view! {
        <div
            id="message-panel"
            class="container-fluid"
            node_ref=message_panel_ref
            on:scroll=on_scroll
        >
            <For
                each=move || messages.get()
                key=|entry: &(String, TabConsoleMessages)| entry.0.clone()
                children=move |entry: (String, TabConsoleMessages)| {
                    let (_key, line) = entry;
                    view! {
                        <div class="console-line">
                            <span class="time">"["{ line.time }"]"</span>
                            <span class="evt">"["{ line.event }"]"</span>

                            <For
                                each=move || line.messages.clone()
                                key=|msg_line: &String| msg_line.clone()
                                children=move |msg_line: String| {
                                    let split_lines: Vec<String> = msg_line
                                        .split('\n')
                                        .map(|s| s.to_string())
                                        .collect();

                                    view! {
                                        <div class="msg">
                                            <For
                                                each=move || split_lines.clone()
                                                key=|line: &String| line.clone()
                                                children=move |text: String| {
                                                    view! {
                                                        <p class="msg-line">{ text }</p>
                                                    }
                                                }
                                            />
                                        </div>
                                    }
                                }
                            />
                        </div>
                    }
                }
            />
        </div>

        <CommandInput />
    }
}

#[component]
fn CommandInput() -> impl IntoView {
    let input_data = RwSignal::new(String::new());
    let agent_map =
        use_context::<RwSignal<HashMap<String, RwSignal<Agent>>>>().expect("no agent map found");
    let tabs: RwSignal<ActiveTabs> =
        use_context().expect("could not get tabs context in CommandInput()");

    let submit_input = Action::new_local(move |input: &String| {
        let input = input.clone();
        let map = agent_map.get();
        let agent_id = tabs
            .read()
            .active_id
            .clone()
            .and_then(|id| resolve_tab_to_agent_id(&id, &map))
            .expect("could not resolve agent id from active tab");

        async move { dispatch_task(input, IsTaskingAgent::Yes(agent_id)).await }
    });

    view! {
        <div id="input-strip" class="d-flex align-items-center px-3">
            <span class="me-2">>></span>
            <form
                on:submit=move |ev| {
                    ev.prevent_default();

                    if input_data.get().is_empty() {
                        return;
                    }

                    //
                    // Push the input message by the user into the currently selected
                    // agent.
                    //
                    let map = agent_map.get();
                    let agent_id = tabs
                        .read()
                        .active_id
                        .clone()
                        .and_then(|id| resolve_tab_to_agent_id(&id, &map))
                        .expect("could not resolve agent id from active tab");
                    let agent_sig = map.get(&agent_id).unwrap();

                    // Get a snapshot of the input and work with that
                    let input_val = input_data.get();

                    let time = Utc::now().to_string();

                    let msg = TabConsoleMessages {
                        completed_id: 0,
                        event: "User Input".to_string(),
                        time,
                        messages: vec![input_val.clone()],
                    };

                    agent_sig.update(move |agent| agent.output_messages.push(msg.clone()));

                    submit_input.dispatch(input_val);

                    // Clear the input UI box
                    input_data.set(String::new());
                }
                autocomplete="off"
                class="d-flex flex-grow-1"
            >
                <Show
                    when=move || tabs.read().active_id.is_some()
                    fallback=|| view! {
                        "Please select an agent to use the input bar."
                    }
                >
                    <input
                        id="cmd_input"
                        name="cmd_input"
                        type="text"
                        class="flex-grow-1"
                        placeholder="Type a command..."
                        bind:value=input_data
                    />
                    <button class="btn btn-sm btn-secondary btn-block">"Send"</button>
                </Show>
            </form>
        </div>
    }
}
