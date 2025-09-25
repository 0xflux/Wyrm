use std::{env, sync::Arc};

use iced::{
    Color, Length, Task, alignment,
    widget::{button, column, container, row, text, text_input},
};
use shared::pretty_print::print_failed;

use crate::{
    gui::{Message, Page, normal_page::model::NormalPage},
    state::{Credentials, do_login},
};

pub type LPM = LoginPageMessage;

#[derive(Debug, Clone)]
pub enum LoginPageMessage {
    LoginButtonPress,
    UsernameInput(String),
    PasswordInput(String),
    UrlInput(String),
    LoginAttempt(Option<Credentials>),
}

#[derive(Debug, Default)]
pub struct LoginPage {
    username: String,
    password: String,
    c2_url: String,
    login_in_progress: bool,
    login_error_message: String,
}

impl LoginPage {
    pub fn new() -> Self {
        Self {
            login_in_progress: false,
            ..Self::default()
        }
    }

    fn try_login(&self) -> Task<Message> {
        let admin_env_token =
            env::var("ADMIN_TOKEN").expect("could not find environment variable ADMIN_TOKEN");
        if admin_env_token.is_empty() {
            print_failed(
                "You must set up your .env to contain an ADMIN_TOKEN, make it random, make it secure. Then copy that token into the C2's .env",
            );
            std::process::exit(0);
        }

        let creds = Credentials {
            username: self.username.clone(),
            password: self.password.clone(),
            admin_env_token,
            c2_url: self.c2_url.clone(),
        };

        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || do_login(creds))
                    .await
                    .unwrap()
            },
            move |login_result| Message::Login(LPM::LoginAttempt(login_result)),
        )
    }
}

impl Page for LoginPage {
    fn update(&mut self, message: Message) -> (Option<Box<dyn Page>>, Task<Message>) {
        if let Message::Login(msg) = message {
            match msg {
                LPM::LoginButtonPress => {
                    self.login_in_progress = true;
                    self.login_error_message = "".into();
                    let task = self.try_login();
                    return (None, task);
                }
                LPM::UsernameInput(s) => self.username = s,
                LPM::PasswordInput(s) => self.password = s,
                LPM::UrlInput(s) => self.c2_url = s,
                LPM::LoginAttempt(login_result) => {
                    self.login_in_progress = false;

                    if let Some(creds) = login_result {
                        return (
                            Some(Box::new(NormalPage::new(Arc::new(creds)))),
                            Task::none(),
                        );
                    }

                    self.login_error_message = "Login failed.".into();
                }
            }
        }

        (None, Task::none())
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let login_button = match !self.login_in_progress {
            true => button("Log in").on_press(Message::Login(LPM::LoginButtonPress)),
            false => button("Log in"),
        };

        let form = column![
            row![text("Login to Wyrm C2.").size(30),].spacing(15),
            row![text(&self.login_error_message).color(Color::from_rgb8(231, 130, 132)),]
                .spacing(5),
            text_input("C2 URL", self.c2_url.as_str())
                .on_input(|s| Message::Login(LPM::UrlInput(s)))
                .width(400),
            text_input("Username", self.username.as_str())
                .on_input(|s| Message::Login(LPM::UsernameInput(s)))
                .width(400),
            text_input("Password", &self.password)
                .secure(true)
                .on_input(|s| Message::Login(LPM::PasswordInput(s)))
                .width(400),
            login_button,
        ]
        .spacing(15)
        .align_x(alignment::Horizontal::Center);

        // content
        column![
            container(form)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
        ]
        .into()
    }
}
