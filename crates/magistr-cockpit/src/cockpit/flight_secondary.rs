//! Settings, search and keyboard help over the live workspace.

use crate::cockpit::{
    Binding, FlightStyle as S, Model, Search, SearchScope, SettingsGroup, ThemeChoice, Widget,
};
use crate::layout::{Length, Node, TextRole};
use crate::widget_id::WidgetId;

pub(super) struct FlightSecondary;

impl FlightSecondary {
    pub(super) fn settings(model: &Model, s: &S) -> Node {
        let state = model.settings().expect("open settings");
        let width = (model.flight_layout().screen_width - 40.0).clamp(240.0, 760.0);
        let mut nav = Node::row().wrap();
        for group in state.visible() {
            nav = nav
                .child(button(
                    s,
                    group_name(group),
                    Widget::settings_nav(group.index()),
                    group == state.group(),
                ))
                .child(S::hspace(6.0));
        }
        let query = editor(s, state.query());
        let mut body = Node::column()
            .child(query)
            .child(S::vspace(12.0))
            .child(nav)
            .child(S::vspace(16.0))
            .child(s.heading(group_name(state.group())))
            .child(S::vspace(8.0));
        let data = model.settings_data();
        match state.group() {
            SettingsGroup::Appearance => {
                body = body.child(s.prose("Choose how Magistr looks during this session.")).child(S::vspace(12.0));
                let mut choices = Node::row().wrap();
                for (index, choice) in ThemeChoice::ALL.into_iter().enumerate() {
                    let label = match choice { ThemeChoice::Sky => "Dark", ThemeChoice::Chart => "Light", ThemeChoice::FollowOs => "Follow terminal" };
                    choices = choices.child(button(s, label, Widget::settings_theme(index), choice == data.theme)).child(S::hspace(8.0));
                }
                body = body.child(choices);
            }
            SettingsGroup::Motion => {
                let reduced = !model.theme().motion().is_full();
                body = body.child(s.prose("Reduce movement and animation throughout Magistr.")).child(S::vspace(12.0)).child(button(s, if reduced { "Reduced motion: on" } else { "Reduced motion: off" }, Widget::settings_switch(2), reduced));
            }
            SettingsGroup::Rest => body = body.child(s.prose("Magistr keeps your conversation and draft while it waits for input.")),
            SettingsGroup::Shortcuts => body = body.child(s.prose("Open Keyboard help from Commands to see shortcuts for messages, records, permissions and running work.")),
            SettingsGroup::Gates => body = body.child(s.prose("Project checks are configured in the project settings file. Run them from Commands or from the starting screen.")),
            SettingsGroup::Budget => {
                body = body.child(s.prose("Review and change your spending and work limits in the project settings file."));
            }
            SettingsGroup::Charter => body = body.child(s.prose("Your project settings describe the work Magistr may carry out and when it should ask you.")),
            SettingsGroup::Backend => body = body.child(s.prose("Choose and configure the agent in your project settings file.")),
            SettingsGroup::Members => body = body.child(s.prose(data.team_size.map_or_else(|| "No team has been configured.".to_owned(), |count| format!("{count} team members are configured.")))),
            SettingsGroup::Policy => body = body.child(s.prose("Team policy is managed in the team settings file. These rules are read-only here.")),
        }
        body = body.child(S::vspace(20.0)).child(button(
            s,
            "Open settings file",
            Widget::SettingsOpenFile.id(),
            false,
        ));
        let close = s.label("Close", s.accent).id(Widget::SettingsClose.id());
        s.outlined(
            Node::column()
                .child(s.section_heading("Settings", close))
                .child(s.inset(body, 16.0, 16.0)),
            s.panel,
            s.line,
        )
        .width(Length::Px(width))
    }

    pub(super) fn search(model: &Model, s: &S) -> Node {
        let state = model.search().expect("open search");
        let results = Search::results(model);
        let width = (model.flight_layout().screen_width - 40.0).clamp(160.0, 760.0);
        let mut scopes = Node::row().wrap();
        for (index, scope) in SearchScope::ALL.into_iter().enumerate() {
            let name = match scope {
                SearchScope::Everything => "Everything",
                SearchScope::Reports => "Reports",
                SearchScope::Receipts => "Actions",
                SearchScope::Trace => "Reasoning",
                SearchScope::Sessions => "Sessions",
            };
            scopes = scopes
                .child(button(
                    s,
                    name,
                    Widget::search_scope(index),
                    scope == state.scope(),
                ))
                .child(S::hspace(6.0));
        }
        let line = f32::from(model.cell_size().height);
        let row_height = line * 2.0 + 16.0;
        let available = ((model.flight_layout().screen_height - 210.0) / row_height)
            .floor()
            .max(1.0) as usize;
        let top = (state.chosen() + 1).saturating_sub(available);
        let mut body = Node::column()
            .child(editor(s, state.query()).id(Widget::OverlayQuery.id()))
            .child(S::vspace(12.0))
            .child(scopes)
            .child(S::vspace(12.0));
        if results.rows.is_empty() {
            body = body.child(s.prose(if state.query().is_empty() {
                "Search reasoning, reports, actions and saved sessions."
            } else {
                "No matching records. Try a different word or choose another category."
            }));
        }
        for (index, hit) in results.rows.iter().enumerate().skip(top).take(available) {
            let row = Node::column()
                .child(
                    s.label(
                        hit.title.as_deref().unwrap_or(match hit.kind {
                            crate::cockpit::SearchKind::Report => "Report",
                            crate::cockpit::SearchKind::Trace => "Step",
                            crate::cockpit::SearchKind::Receipt => "Action",
                            crate::cockpit::SearchKind::Session => "Session",
                        }),
                        s.accent,
                    )
                    .height(Length::Px(line)),
                )
                .child(s.prose(&hit.line).height(Length::Px(line)).viewport());
            body = body.child(
                s.ground(
                    s.inset(row, 10.0, 8.0),
                    if index == state.chosen() {
                        s.selected
                    } else {
                        s.panel
                    },
                )
                .id(Widget::search_row(index))
                .height(Length::Px(row_height))
                .viewport(),
            );
        }
        s.outlined(
            Node::column()
                .child(s.section_heading("Search recorded work", s.label("Esc to close", s.muted)))
                .child(s.inset(body, 16.0, 12.0)),
            s.panel,
            s.line,
        )
        .width(Length::Px(width))
    }

    pub(super) fn help(model: &Model, s: &S) -> Node {
        let width = (model.flight_layout().screen_width - 40.0).clamp(160.0, 760.0);
        let line = f32::from(model.cell_size().height);
        let mut rows = Vec::new();
        let mut group = "";
        for binding in Binding::ALL {
            if binding.group != group {
                group = binding.group;
                rows.push(S::vspace(line));
                rows.push(
                    s.heading(group_name_for_keys(group))
                        .height(Length::Px(line)),
                );
            }
            rows.push(
                Node::row()
                    .height(Length::Px(line))
                    .shrink(0.0)
                    .child(
                        s.label(binding.keys, s.accent)
                            .width(Length::Px((width * 0.3).max(130.0)))
                            .shrink(0.0),
                    )
                    .child(s.label(binding.does, s.ink).min_width(Length::Px(0.0))),
            );
        }
        let (_, height) = model.sheet_window();
        let top = model.sheet_top();
        let body = Node::column()
            .viewport()
            .height(Length::Px(height as f32 * line))
            .children(rows.into_iter().skip(top).take(height));
        s.outlined(
            Node::column()
                .child(s.section_heading(
                    "Keyboard help",
                    s.label("Esc to close · Page Up / Down to scroll", s.muted),
                ))
                .child(s.inset(body, 16.0, 12.0)),
            s.panel,
            s.line,
        )
        .width(Length::Px(width))
    }
}

fn button(s: &S, label: impl Into<String>, id: WidgetId, selected: bool) -> Node {
    s.chip(
        label,
        s.accent,
        if selected { s.selected } else { s.raised },
    )
    .id(id)
    .height(Length::Px(s.line_height + 8.0))
    .shrink(0.0)
}

fn editor(s: &S, text: &str) -> Node {
    s.ground(
        s.inset(
            Node::editor(text, text.len())
                .text_role(TextRole::Steer)
                .style(s.style(s.ink))
                .height(Length::Px(s.line_height + 6.0))
                .min_width(Length::Px(0.0)),
            10.0,
            5.0,
        ),
        s.raised,
    )
}

fn group_name(group: SettingsGroup) -> &'static str {
    match group {
        SettingsGroup::Gates => "Checks",
        SettingsGroup::Charter => "Project rules",
        SettingsGroup::Backend => "Agent",
        SettingsGroup::Rest => "While waiting",
        _ => group.label(),
    }
}

fn group_name_for_keys(group: &str) -> &str {
    match group {
        "inspector" => "Details",
        "global" => "Navigation",
        "commands" => "Commands",
        "launcher" => "Getting started",
        "roadmap" => "Roadmap",
        "prompt" => "Messages",
        "completion" => "Suggestions",
        "stage" => "Conversation",
        "needs-you" => "Permissions",
        "loop" => "Running work",
        _ => group,
    }
}
