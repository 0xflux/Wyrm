use iced::{
    Alignment, Background, Border, Color, Element, Length, Shadow, Theme,
    border::Radius,
    widget::{
        Column, Row, Scrollable, Space, Text,
        button::{Status, Style},
        column, container, horizontal_rule, row,
        scrollable::{Direction, Scrollbar},
        text, text_input,
    },
};
use shared::tasks::WyrmResult;

use crate::gui::{
    Message,
    normal_page::model::{NPM, NormalPage, SelectedBottomTab},
};

impl NormalPage {
    pub fn view_impl(&self) -> iced::Element<'_, Message> {
        container(column![
            Scrollable::new(self.display_connected_agents())
                .width(Length::FillPortion(2))
                .height(200),
            horizontal_rule(0),
            container(column![
                Scrollable::new(self.bottom_selection_tabs())
                    .width(Length::Fill)
                    .height(35)
                    .direction(Direction::Horizontal(Scrollbar::new())),
                row![
                    Scrollable::new(self.bottom_pane_display())
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .direction(Direction::Vertical(Scrollbar::new()))
                        .anchor_bottom()
                ]
                .width(Length::Fill)
                .height(Length::Fill),
            ]),
            self.bottom_navigation(),
            Space::with_height(6),
        ])
        .into()
    }

    /// Display the bottom navigation of the main panel. If the active tab is the agent, then we can display the
    /// text input box. Otherwise, we want to display buttons / controls that are context aware.
    fn bottom_navigation(&self) -> iced::Element<'_, Message> {
        const HORIZ_BTN_SPACING: u16 = 10;

        let bottom_nav = match self.selected_bottom_tab {
            SelectedBottomTab::StagedResources => {
                row![
                    text(">>>   "),
                    Space::with_width(HORIZ_BTN_SPACING),
                    iced::widget::button("Stage all from profile")
                        .on_press(Message::NormalPage(NPM::StageAllFromProfile)),
                    Space::with_width(HORIZ_BTN_SPACING),
                    iced::widget::button("Stage stager (locked)"),
                    Space::with_width(HORIZ_BTN_SPACING),
                    iced::widget::button("Stage from disk")
                        .on_press(Message::NormalPage(NPM::StageFromDiskButton)),
                    Space::with_width(HORIZ_BTN_SPACING),
                    iced::widget::button("Refresh staged resources")
                        .on_press(Message::NormalPage(NPM::RefreshResources)),
                ]
            }
            SelectedBottomTab::Agent(_) => row![
                text(">>>   "),
                text_input("", self.user_input.as_str())
                    .width(Length::Fill)
                    .on_input(|s| Message::NormalPage(NPM::UserInputUpdated(s)))
                    .on_submit(Message::NormalPage(NPM::SendCommandFromInput))
            ],
        };

        bottom_nav.height(30).into()
    }

    /// Draws the main bottom display which has content depending upon what the selected button is
    fn bottom_pane_display(&self) -> iced::Element<'_, Message> {
        match &self.selected_bottom_tab {
            SelectedBottomTab::StagedResources => self.display_staged_resources(),
            SelectedBottomTab::Agent(agent_id) => {
                if let Some(agent) = &*self.connected_agents.read().unwrap() {
                    let agent = match agent.get(agent_id) {
                        Some(a) => a,
                        None => return text("Agent removed, you may now close this tab.").into(),
                    };

                    let mut buf = column![];

                    for event in &agent.output_messages {
                        // This is to handle things the user typed into the bar at the bottom
                        if event.event == "UserConsoleInput" {
                            if let Some(message) = &event.messages {
                                buf = buf.push(
                                    text!("[+] {}", message.first().unwrap())
                                        .color(Color::from_rgb8(138, 173, 244)),
                                )
                            }

                            continue;
                        }

                        // This handles any messages back from the C2
                        buf = buf.push(
                            text(format!(
                                "* [{}], dispatched: [{}]",
                                event.time.clone(),
                                event.event.clone()
                            ))
                            .color(Color::from_rgb8(238, 212, 159)),
                        );
                        if let Some(messages) = &event.messages {
                            for message in messages {
                                buf = buf.push(text(message.clone()));
                            }
                        }
                    }

                    buf.into()
                } else {
                    text!("There are no notifications / events yet for this implant this session.")
                        .into()
                }
            }
        }
    }

    fn display_staged_resources(&self) -> iced::Element<'_, Message> {
        if let WyrmResult::Ok(collection) = &self.staged_resources {
            if collection.is_empty() {
                return Column::new()
                .push(Text::new(
                    "No staged resources detected, please use the button below to stage a new agent on your C2.",
                ))
                .into();
            }

            const NAME_SZ: u16 = 160;
            const PE_SZ: u16 = 160;
            const SLEEP_SZ: u16 = 120;
            const ENDPOINT_SZ: u16 = 280;

            let mut container = Column::new().spacing(10).push(
                Row::new()
                    .spacing(20)
                    .push(Text::new("Name").width(NAME_SZ))
                    .push(Text::new("PE download name").width(PE_SZ))
                    .push(Text::new("Download URI").width(ENDPOINT_SZ))
                    .push(Text::new("C2 URI").width(ENDPOINT_SZ))
                    .push(Text::new("Sleep time").width(SLEEP_SZ))
                    .push(Text::new("Port").width(SLEEP_SZ)),
            );

            for staged in collection {
                container = container.push(
                    Row::new()
                        .spacing(20)
                        .push(
                            Text::new(&staged.agent_name)
                                .width(NAME_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(
                            Text::new(&staged.pe_name)
                                .width(PE_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(
                            text!("/{}", staged.staged_endpoint)
                                .width(ENDPOINT_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(
                            Text::new(&staged.c2_endpoint)
                                .width(ENDPOINT_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(
                            Text::new(staged.sleep_time)
                                .width(SLEEP_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(
                            Text::new(staged.port)
                                .width(SLEEP_SZ)
                                .color(Color::from_rgb8(240, 198, 198)),
                        )
                        .push(iced::widget::button("Delete").on_press(Message::NormalPage(
                            NPM::DeleteStagedResource(staged.staged_endpoint.clone()),
                        ))),
                );
            }

            container = container.push(Space::with_height(20));

            Scrollable::new(container)
                .direction(Direction::Both {
                    vertical: Scrollbar::new(),
                    horizontal: Scrollbar::new(),
                })
                .into()
        } else {
            Column::new()
                .push(Text::new(
                    "Either there are no current staged resources, \
                     you need to refresh this panel, or your connection \
                     to the C2 is bad. Please press the refresh button \
                     below if you are expecting to see your staged \
                     resources; or click below to stage an agent.",
                ))
                .into()
        }
    }

    /// The selection tabs which run along the top of the main bottom panel, allowing the user to select
    /// which thing they want to view, or interact with.
    fn bottom_selection_tabs(&self) -> iced::Element<'_, Message> {
        // These tabs are persistent and cannot be closed
        let mut tabs = vec![(
            SelectedBottomTab::StagedResources,
            "Staged resources".into(),
        )];

        // Look for any agents the user has selected to show in the space at the bottom
        if let Some(agents) = &*self.connected_agents.read().unwrap() {
            for id in &self.agents_as_tabs {
                let label = agents
                    .get(id)
                    .map(|a| a.agent_id.clone())
                    .unwrap_or_else(|| "Removed".into());
                tabs.push((SelectedBottomTab::Agent(id.into()), label));
            }
        }
        let active_idx = tabs
            .iter()
            .position(|(t, _)| *t == self.selected_bottom_tab)
            .unwrap_or(0);

        // For a row of outer buttons (so we can nest the close button inside)
        let mut tabs_row = row![].spacing(0);

        for (idx, (tab, label)) in tabs.into_iter().enumerate() {
            let is_active = idx == active_idx;

            // Create the button for the close (X)
            let close_btn = iced::widget::button(Text::new("×").size(14))
                .padding(0)
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(28.0))
                .on_press(Message::NormalPage(NPM::CloseBottomTab(tab.clone())))
                .style(move |theme, _| {
                    // exactly same fill as outer, but no border
                    let mut s = bottom_tab_button_style(theme, Status::Active, is_active);
                    s.border.width = 0.0;
                    s
                });

            // Create the outer button which houses the text for the widget, as well as the X button
            let outer = iced::widget::button(
                row![
                    Text::new(label).size(14),
                    Space::with_width(Length::Fixed(10.0)),
                    close_btn,
                ]
                .spacing(0)
                .align_y(Alignment::Center),
            )
            .height(Length::Fixed(28.0))
            .on_press(Message::NormalPage(NPM::BottomSectionButtonClick(
                tab.clone(),
            )))
            .style(move |theme, status| bottom_tab_button_style(theme, status, is_active));

            tabs_row = tabs_row.push(outer);
        }

        tabs_row.into()
    }

    fn display_connected_agents(&self) -> Element<'_, Message> {
        // Define some constants and closures to help standardise the table for this section,
        // otherwise we will duplicate a lot of code

        const ID_SZ: u16 = 300;
        const PID_SZ: u16 = 70;
        const CHECKIN_SZ: u16 = 220;
        const FONT_SZ: u16 = 15;

        let make_header = |text: String, width: u16| {
            iced::widget::text(text)
                .size(FONT_SZ)
                .width(width)
                .color(Color::from_rgb8(202, 211, 245))
        };

        let make_cell =
            |text: String, width: u16| iced::widget::text(text).size(FONT_SZ).width(width);

        let make_full_width_cell =
            |text: String| iced::widget::text(text).size(FONT_SZ).width(Length::Fill);

        let lock = self.connected_agents.read().unwrap();

        match self.connection_state {
            crate::gui::normal_page::model::ConnectionState::Connecting => {
                return row![
                    text("Connecting, please wait...").color(Color::from_rgb8(245, 169, 127)),
                ]
                .into();
            }
            crate::gui::normal_page::model::ConnectionState::Connected => {
                if lock.is_none() {
                    return row![
                        text("C2 online, however - no agents connected.")
                            .color(Color::from_rgb8(166, 218, 149)),
                    ]
                    .into();
                }
            }
            crate::gui::normal_page::model::ConnectionState::Disconnected => {
                return row![
                    text("Connection error. Please check the C2 is online and you have internet.")
                        .color(Color::from_rgb8(231, 130, 132)),
                ]
                .into();
            }
        }

        // Clone the lock's inner data so we can avoid issues with the borrow checker for a below closure
        let data = lock.clone();
        drop(lock);

        //
        // At this point we have agents connected to display
        //

        let mut rows = column![row![
            make_header("Agent ID".into(), ID_SZ),
            make_header("PID".into(), PID_SZ),
            make_header("Last checkin".into(), CHECKIN_SZ),
            make_full_width_cell("Process Name".into()),
            Space::with_height(10),
        ]];

        if let Some(has_agents) = data {
            for agent in has_agents {
                let agent_id = {
                    let splitted: Vec<&str> = agent.1.agent_id.split('_').collect();
                    format!("{}@{}/{}", splitted[2], splitted[0], splitted[3])
                };
                let btn_content = row![
                    column![make_cell(agent_id, ID_SZ)],
                    column![make_cell(format!("{}", agent.1.pid), PID_SZ)],
                    column![make_cell(format!("{}", agent.1.last_check_in), CHECKIN_SZ)],
                    column![make_full_width_cell(agent.1.process_name)],
                ];

                let btn = iced::widget::button(btn_content)
                    .padding(0)
                    .style(move |_, status: Status| {
                        let base = if !agent.1.is_stale {
                            Style {
                                background: None,
                                text_color: Color::from_rgb8(240, 198, 198),
                                border: Border::default(),
                                shadow: Shadow::default(),
                            }
                        } else {
                            Style {
                                background: None,
                                text_color: Color::from_rgb8(147, 154, 183),
                                border: Border::default(),
                                shadow: Shadow::default(),
                            }
                        };

                        match status {
                            Status::Hovered => {
                                base.with_background(Color::from_rgb(0.15, 0.15, 0.15))
                            }
                            Status::Pressed => {
                                base.with_background(Color::from_rgb(0.25, 0.25, 0.25))
                            }
                            _ => base,
                        }
                    })
                    .width(Length::Fill)
                    .on_press(Message::NormalPage(NPM::AgentSelectFromTopPanel(agent.0)));

                rows = rows.push(btn);
            }
        }

        rows.into()
    }
}

fn bottom_tab_button_style(_theme: &Theme, status: Status, is_current_page: bool) -> Style {
    let border_color = Color::from_rgb8(128, 135, 162);
    let text_color = Color::from_rgb8(202, 211, 245);
    let background = None;

    let background = match is_current_page {
        true => Some(Background::Color(Color::from_rgb8(73, 77, 100))),
        false => match status {
            Status::Hovered => Some(Background::Color(Color::from_rgb8(73, 77, 100))),
            Status::Pressed => Some(Background::Color(Color::from_rgb8(54, 58, 79))),
            // Status::Active => Some(Background::Color(Color::from_rgb8(145, 215, 227))),
            _ => background,
        },
    };

    Style {
        background,
        text_color,
        border: Border {
            radius: Radius::from(0.),
            width: 1.,
            color: border_color,
        },
        shadow: Shadow::default(),
    }
}
