use std::time::Duration;

use crate::gui::{
    Message, Page,
    normal_page::model::{NPM, NormalPage},
};
use iced::{Subscription, Task, time};

impl Page for NormalPage {
    fn update(&mut self, message: Message) -> (Option<Box<dyn Page>>, Task<Message>) {
        self.controller_impl(message)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut batch = vec![];

        batch.push(
            time::every(Duration::from_secs(1))
                .map(|_| Message::NormalPage(NPM::PollConnectedAgents)),
        );

        Subscription::batch(batch)
    }

    fn view(&self) -> iced::Element<'_, Message> {
        self.view_impl()
    }
}
