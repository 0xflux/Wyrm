use std::sync::Arc;

use dotenvy::dotenv;
use iced::{Color, Size, Subscription, Task, daemon::Appearance, window::Settings};
use shared::pretty_print::print_failed;

use crate::{
    gui::{
        login::{LPM, LoginPage},
        normal_page::model::{NPM, NormalPage},
        staging::{stage_all::StageAllMessage, stage_upload::StageUploadMessage},
    },
    state::Credentials,
};

mod login;
mod new_tasks;
pub mod normal_page;
mod staging;

pub fn start_gui() {
    if dotenv().is_err() {
        print_failed(
            "You must ensure you are using the .env file, as your ADMIN_TOKEN is required. You must set this up, and use the **same token** in the server .env",
        );
        std::process::exit(0);
    }

    let _ = iced::application("Wyrm Client", MyApp::update, MyApp::view)
        .window(Settings {
            size: Size::new(1350.0, 800.0),          // initial window dimensions
            min_size: Some(Size::new(800.0, 600.0)), // same here sets the floor
            resizable: true,
            ..Settings::default()
        })
        .style(|_state, _theem| Appearance {
            background_color: Color::from_rgb8(30, 32, 48),
            text_color: Color::from_rgb8(202, 211, 245),
        })
        .subscription(|app: &MyApp| app.subscription())
        .run_with(MyApp::new);
}

#[derive(Debug, Clone)]
pub enum Message {
    Login(LPM),
    NormalPage(NPM),
    StageUpload(StageUploadMessage),
    StageAll(StageAllMessage),
}

struct MyApp {
    page: Box<dyn Page>,
}

trait Page {
    fn update(&mut self, message: Message) -> (Option<Box<dyn Page>>, Task<Message>);
    fn view(&self) -> iced::Element<'_, Message>;

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}

impl MyApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                // page: Box::new(NormalPage::new(Arc::new(Credentials {
                //     username: "flux".into(),
                //     password: "password".into(),
                //     admin_env_token: "fdgiyh%^l!udjfh78364LU7&%df!!".into(),
                //     c2_url: "http://127.0.0.1:8080".into(),
                // }))),
                page: Box::new(LoginPage::new()),
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        self.page.subscription()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let (page, task) = self.page.update(message);

        if let Some(p) = page {
            self.page = p;
        }

        task
    }

    fn view(&self) -> iced::Element<'_, Message> {
        self.page.view()
    }
}
