use leptos::{component, prelude::*};
use shared::StagedResourceDataNoSqlx;
use shared::tasks::{AdminCommand, WyrmResult};

use crate::{
    net::{C2Url, IsTaskingAgent, api_request},
    pages::logged_in_headers::LoggedInHeaders,
};

#[derive(Clone, Debug)]
pub struct StagedResourcesRowInner {
    download_name: String,
    uri: String,
    num_downloads: i64,
}

#[component]
pub fn StagedResourcesPage() -> impl IntoView {
    let staged_rows: RwSignal<Vec<StagedResourcesRowInner>> = RwSignal::new(vec![]);

    let fetch_resources = Action::new_local(|_: &()| async move {
        api_request(
            AdminCommand::ListStagedResources,
            &IsTaskingAgent::No,
            None,
            C2Url::Standard,
            None,
        )
        .await
    });
    let staged_resources_response = fetch_resources.value();

    Effect::new(move |_| {
        staged_resources_response.with(|inner| {
            if let Some(res) = inner {
                match res {
                    Ok(res) => {
                        let inner: WyrmResult<Vec<StagedResourceDataNoSqlx>> =
                            serde_json::from_slice(&res).unwrap();

                        let inner = inner.unwrap();

                        {
                            let mut guard = staged_rows.write();
                            for line in inner {
                                (*guard).push(StagedResourcesRowInner {
                                    download_name: line.pe_name,
                                    uri: line.staged_endpoint,
                                    num_downloads: line.num_downloads,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to get response for staged data. {e}")
                    }
                }
            }
        })
    });

    Effect::new(move |_| {
        fetch_resources.dispatch(());
    });

    view! {
        <LoggedInHeaders />

        <div class="container-fluid py-4 app-page">
            <div class="row mb-4">
                <div class="col-12 text-center">
                    <h2 class="mb-2 fw-bold">Staged resources</h2>
                    <p>Here you can view resources you have staged on the C2 and their URI.
                    </p>
                </div>
            </div>

            <div class="container">
                <div class="table-responsive">
                    <table id="staged-resources-tbl" class="table table-sm align-middle">
                        <thead class="table">
                            <tr>
                                <th class="col">Download name</th>
                                <th class="col">URI</th>
                                <th class="col"># downloads</th>
                            </tr>
                        </thead>

                        <tbody id="staged-resource-rows">
                            <For
                                each=move || staged_rows.get()
                                key=|row: &StagedResourcesRowInner| row.download_name.clone()
                                children=move |row: StagedResourcesRowInner| {
                                    view! {
                                        <tr>
                                            <td class="col">{ row.download_name }</td>
                                            <td class="col">{ row.uri }</td>
                                            <td class="col">{ row.num_downloads }</td>
                                        </tr>
                                    }
                                }
                            />
                            <Show when=move || staged_rows.get().is_empty()>
                                <tr>
                                    <td class="col">You currently have no staged resources.</td>
                                    <td class="col"></td>
                                    <td class="col"></td>
                                </tr>
                            </Show>
                        </tbody>


                    </table>
                </div>
            </div>

        </div>
    }
}
