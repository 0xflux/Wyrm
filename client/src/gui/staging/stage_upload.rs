use std::{fs, path::PathBuf, sync::Arc};

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
    tasks::{AdminCommand, FileUploadStagingFromClient, WyrmResult},
};

use crate::{
    gui::{Message, Page, normal_page::model::NormalPage},
    net::{IsTaskingAgent, api_request},
    state::Credentials,
};

#[derive(Default, Debug)]
pub struct StageUploadPage {
    credentials: Arc<Credentials>,
    file_name: String,
    staged_uri: String,
    file_path: PathBuf,
    page_errors: Vec<String>,
}

impl StageUploadPage {
    pub fn new(credentials: Arc<Credentials>) -> Self {
        Self {
            credentials: credentials,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub enum StageUploadMessage {
    SubmitForm,
    FileNameInput(String),
    UriInput(String),
    FileInput,
    BackButton,
}

impl Page for StageUploadPage {
    fn update(
        &mut self,
        message: super::super::Message,
    ) -> (Option<Box<dyn Page>>, iced::Task<super::super::Message>) {
        if let Message::StageUpload(message) = message {
            match message {
                StageUploadMessage::SubmitForm => {
                    self.page_errors.clear();

                    if self.file_name.is_empty()
                        || self.file_path.as_os_str().is_empty()
                        || self.staged_uri.is_empty()
                    {
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

                    let data = fs::read(&self.file_path);
                    let file_data = match data {
                        Ok(d) => d,
                        Err(e) => {
                            self.page_errors.push(format!("Failed to read file. {e}"));
                            return (None, Task::none());
                        }
                    };

                    let download_name = std::mem::take(&mut self.file_name);
                    let api_endpoint = std::mem::take(&mut self.staged_uri);

                    let staging_info = FileUploadStagingFromClient {
                        download_name,
                        api_endpoint,
                        file_data: file_data,
                    };

                    //
                    // Upload the file to the C2
                    //

                    let response: WyrmResult<String> = match api_request(
                        AdminCommand::StageFileOnC2(staging_info),
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
                StageUploadMessage::FileInput => 'file_input: {
                    let file = FileDialog::new().pick_file();

                    let selected_file_path = match file {
                        Some(f) => f,
                        None => {
                            print_failed("Failed to select file");
                            self.page_errors.push("Failed to select file.".into());
                            break 'file_input;
                        }
                    };

                    self.file_path = selected_file_path;
                }
                StageUploadMessage::FileNameInput(s) => {
                    // prevent user adding a leading /
                    if !s.starts_with('/') {
                        self.file_name = s
                    }
                }
                StageUploadMessage::UriInput(s) => {
                    // prevent user adding a leading /
                    if !s.starts_with('/') {
                        self.staged_uri = s
                    }
                }
                StageUploadMessage::BackButton => {
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

        let selected_path = if !self.file_path.as_os_str().is_empty() {
            let mut path_col = column![];

            path_col = path_col.push(text!("File selected: {}", self.file_path.display()).size(16));

            path_col
        } else {
            column![]
        };

        container(
            Scrollable::new(column![
                    button("<- Back").on_press(Message::StageUpload(StageUploadMessage::BackButton)),
                    text("Wyrm File Upload").size(26),
                    Space::with_height(SPACING),
                    text("This will guide you through the process of uploading a file to the C2."),

                    Space::with_height(SPACING),
                    text("Please enter a download name for this payload, you MUST enter the file extension if you want one here. E.g. if you want the file to download\
                    natively as a PDF, you would write: invoice.pdf"),
                    text_input("Download name", self.file_name.as_str()).on_input(|s| Message::StageUpload(StageUploadMessage::FileNameInput(s))),

                    Space::with_height(SPACING),
                    text("Choose a URI endpoint that will serve the file download. This may include multi-path directories and URL params such \
                    as contracts/microsoft/2025/msft_contract_25&auth=4b5lk4jhb45khjb7hbv345hvb34hb1jbn"),
                    text_input("Staging URI", self.staged_uri.as_str()).on_input(|s| Message::StageUpload(StageUploadMessage::UriInput(s))),

                    Space::with_height(SPACING),
                    text("Select the file you wish to upload:"),
                    iced::widget::button("Select file")
                        .on_press(Message::StageUpload(StageUploadMessage::FileInput)),
                    selected_path,

                    Space::with_height(SPACING_SMALL),
                    Space::with_height(SPACING_SMALL),
                    page_errors,
                    iced::widget::button("Upload").on_press(Message::StageUpload(StageUploadMessage::SubmitForm)),
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
