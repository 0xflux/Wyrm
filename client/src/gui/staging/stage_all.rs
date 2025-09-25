use std::{path::PathBuf, sync::Arc};

use iced::{
    Color, Length, Task,
    alignment::Horizontal,
    widget::{
        Scrollable, Space, button, column, container,
        scrollable::{Direction, Scrollbar},
        text, text_input,
    },
};
use rfd::FileDialog;
use shared::{
    pretty_print::print_failed,
    tasks::{AdminCommand, BuildAllBins, WyrmResult},
};

use crate::{
    gui::{Message, Page, normal_page::model::NormalPage},
    net::{IsTaskingAgent, api_request},
    state::Credentials,
};

#[derive(Default, Debug)]
pub struct StageAllPage {
    credentials: Arc<Credentials>,
    file_name: String,
    profile_name: String,
    save_path: PathBuf,
    page_errors: Vec<String>,
}

impl StageAllPage {
    pub fn new(credentials: Arc<Credentials>) -> Self {
        Self {
            credentials: credentials,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub enum StageAllMessage {
    SubmitForm,
    ProfileName(String),
    SavePath,
    BackButton,
}

impl Page for StageAllPage {
    fn update(
        &mut self,
        message: super::super::Message,
    ) -> (Option<Box<dyn Page>>, iced::Task<super::super::Message>) {
        if let Message::StageAll(message) = message {
            match message {
                StageAllMessage::SubmitForm => {
                    self.page_errors.clear();

                    if self.save_path.as_os_str().is_empty() || self.profile_name.is_empty() {
                        self.page_errors
                            .push("A field is empty, please enter data into each option.".into());
                    }

                    if !self.page_errors.is_empty() {
                        return (None, Task::none());
                    }

                    //
                    // Error checking was OK - we can now try upload the file
                    //

                    let local_copy_creds = (*self.credentials).clone();

                    if self.profile_name.is_empty() || self.save_path.as_os_str().is_empty() {
                        self.page_errors.push(format!("All fields required"));
                        return (None, Task::none());
                    }

                    let profile_name = std::mem::take(&mut self.profile_name);
                    let save_path = std::mem::take(&mut self.save_path);

                    let bab: BuildAllBins = (
                        profile_name,
                        // TODO: This is a lossy string conversion and may cause a bug..?
                        save_path.as_os_str().to_string_lossy().into(),
                        None,
                        None,
                    );

                    let response: WyrmResult<String> = match api_request(
                        AdminCommand::BuildAllBins(bab),
                        IsTaskingAgent::No,
                        &local_copy_creds,
                    ) {
                        Ok(r) => match serde_json::from_slice(&r) {
                            Ok(r) => r,
                            Err(e) => {
                                self.page_errors.push(format!(
                                    "An error was encountered deserialising the response, {e}"
                                ));
                                return (None, Task::none());
                            }
                        },
                        Err(e) => {
                            self.page_errors
                                .push(format!("An error was encountered uploading your file, {e}"));
                            return (None, Task::none());
                        }
                    };

                    match response {
                        WyrmResult::Ok(_) => {
                            return (
                                Some(Box::new(NormalPage::new(self.credentials.clone()))),
                                Task::none(),
                            );
                        }
                        WyrmResult::Err(e) => {
                            self.page_errors.push(e.to_string());
                            return (None, Task::none());
                        }
                    }
                }
                StageAllMessage::SavePath => 'file_input: {
                    let folder = FileDialog::new().pick_folder();

                    let selected_folder_path = match folder {
                        Some(f) => f,
                        None => {
                            print_failed("Failed to select folder");
                            self.page_errors.push("Failed to select folder.".into());
                            break 'file_input;
                        }
                    };

                    self.save_path = selected_folder_path;
                }
                StageAllMessage::ProfileName(s) => {
                    // prevent user adding a leading /
                    if !s.starts_with('/') {
                        self.profile_name = s
                    }
                }
                StageAllMessage::BackButton => {
                    return (
                        Some(Box::new(NormalPage::new(self.credentials.clone()))),
                        Task::none(),
                    );
                }
            }
        }

        (None, Task::none())
    }

    fn view(&self) -> iced::Element<'_, super::super::Message> {
        const SPACING: Length = Length::Fixed(20.);
        const SPACING_SMALL: Length = Length::Fixed(10.);

        let err_colour: Color = Color::from_rgb8(237, 135, 150);
        let page_errors = if !self.page_errors.is_empty() {
            let mut err_col = column![];

            err_col = err_col.push(
                text("The following errors were encountered:")
                    .size(16)
                    .color(err_colour),
            );

            for e in &self.page_errors {
                err_col = err_col.push(text!("{}", e).color(err_colour));
            }

            err_col
        } else {
            column![]
        };

        let selected_path = if !self.save_path.as_os_str().is_empty() {
            let mut path_col = column![];

            path_col = path_col.push(text!("File selected: {}", self.save_path.display()).size(16));

            path_col
        } else {
            column![]
        };

        container(
            Scrollable::new(column![
                    button("<- Back").on_press(Message::StageAll(StageAllMessage::BackButton)),
                    text("Stage all payloads").size(26),

                    Space::with_height(SPACING),
                    text("Type the name of the profile you wish to build from (do not include the .toml). For example, to build from the \
                        default profile, type: default"),
                    text_input("Profile name", self.profile_name.as_str()).on_input(|s| Message::StageAll(StageAllMessage::ProfileName(s))),

                    Space::with_height(SPACING),
                    text("Select the folder in which to save the files:"),
                    iced::widget::button("Select folder")
                        .on_press(Message::StageAll(StageAllMessage::SavePath)),
                    selected_path,

                    Space::with_height(SPACING_SMALL),
                    Space::with_height(SPACING_SMALL),
                    page_errors,
                    iced::widget::button("Build").on_press(Message::StageAll(StageAllMessage::SubmitForm)),
                    ].padding(20))
                    .width(650)
                    .height(Length::Fill)
                    .direction(Direction::Vertical(Scrollbar::new())),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .into()
    }
}
