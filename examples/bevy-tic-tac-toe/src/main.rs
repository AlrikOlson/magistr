use bevy::{
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::{Activate, Button},
};
use magistr_tic_tac_toe::{Mark, Match, Outcome};

#[derive(Resource, Default)]
struct Game(Match);
#[derive(Component)]
struct CellLabel(usize);
#[derive(Component)]
struct StatusLabel;
#[derive(Component)]
struct ScoreLabel;
#[derive(Component)]
struct RoleLabel;
#[derive(Component)]
struct Cell(usize);
#[derive(Component)]
struct Rematch;

const INK: Color = Color::srgb(0.12, 0.15, 0.19);
const PAPER: Color = Color::srgb(0.96, 0.95, 0.91);
const CELL: Color = Color::srgb(1.0, 0.99, 0.96);
const X_COLOR: Color = Color::srgb(0.12, 0.34, 0.52);
const O_COLOR: Color = Color::srgb(0.65, 0.24, 0.12);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tic-tac-toe".into(),
                resolution: (640, 620).into(),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(PAPER))
        .init_resource::<Game>()
        .add_systems(Startup, setup)
        .add_systems(Update, (keyboard, computer, draw).chain())
        .run();
}

fn label(text: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(INK),
    )
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(10),
            padding: UiRect::all(px(12)),
            ..default()
        })
        .with_children(|root| {
            root.spawn(label("Tic-tac-toe", 30.0));
            root.spawn((label("You are X", 15.0), RoleLabel));
            root.spawn((label("Your turn", 18.0), StatusLabel));
            root.spawn(Node {
                display: Display::Grid,
                width: px(264),
                height: px(264),
                flex_shrink: 0.0,
                column_gap: px(6),
                row_gap: px(6),
                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                grid_template_rows: RepeatedGridTrack::flex(3, 1.0),
                ..default()
            })
            .with_children(|grid| {
                for cell in 0..9 {
                    grid.spawn((
                        Button,
                        Cell(cell),
                        Node {
                            width: percent(100),
                            height: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BackgroundColor(CELL),
                        BorderColor::all(Color::srgb(0.57, 0.59, 0.55)),
                    ))
                    .with_child((label((cell + 1).to_string(), 38.0), CellLabel(cell)))
                    .observe(
                        move |_: On<Activate>, mut game: ResMut<Game>| {
                            let _ = game.0.human_move(cell);
                        },
                    );
                }
            });
            root.spawn((label("You 0   Computer 0   Draws 0", 14.0), ScoreLabel));
            root.spawn((
                Button,
                Rematch,
                InteractionDisabled,
                Node {
                    width: px(190),
                    height: px(42),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(PAPER),
                BorderColor::all(Color::srgb(0.57, 0.59, 0.55)),
            ))
            .with_child(label("Play again [R]", 16.0))
            .observe(|_: On<Activate>, mut game: ResMut<Game>| {
                game.0.next_round();
            });
            root.spawn(label("Choose a square: click or 1-9", 13.0));
        });
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>) {
    if keys.just_pressed(KeyCode::KeyR) {
        game.0.next_round();
        return;
    }
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (cell, key) in digits.into_iter().enumerate() {
        if keys.just_pressed(key) {
            let _ = game.0.human_move(cell);
            break;
        }
    }
}

fn computer(mut game: ResMut<Game>) {
    if game.0.board.turn() != game.0.human && game.0.board.outcome() == Outcome::Playing {
        game.0.computer_move();
    }
}

fn draw(
    game: Res<Game>,
    mut commands: Commands,
    mut labels: Query<(
        &mut Text,
        &mut TextColor,
        Option<&CellLabel>,
        Has<StatusLabel>,
        Has<ScoreLabel>,
        Has<RoleLabel>,
    )>,
    mut surfaces: Query<(Entity, Option<&Cell>, Has<Rematch>, &mut BackgroundColor)>,
) {
    if !game.is_changed() {
        return;
    }
    let message = game.0.status();
    let [you, computer, draws] = game.0.score;
    for (mut text, mut color, cell, is_status, is_score, is_role) in &mut labels {
        if let Some(cell) = cell {
            let mark = game.0.board.cells()[cell.0];
            **text = mark.map_or_else(|| (cell.0 + 1).to_string(), |m| m.label().into());
            color.0 = match mark {
                Some(Mark::X) => X_COLOR,
                Some(Mark::O) => O_COLOR,
                None => Color::srgb(0.49, 0.51, 0.47),
            };
        } else if is_status {
            **text = message.clone();
        } else if is_score {
            **text = format!("You {you}   Computer {computer}   Draws {draws}");
        } else if is_role {
            **text = format!(
                "You are {}   Computer is {}",
                game.0.human.label(),
                game.0.human.other().label()
            );
        }
    }
    for (entity, cell, rematch, mut background) in &mut surfaces {
        if let Some(cell) = cell {
            let winning =
                matches!(game.0.board.outcome(), Outcome::Won(_, line) if line.contains(&cell.0));
            background.0 = if winning {
                Color::srgb(0.79, 0.87, 0.77)
            } else {
                CELL
            };
        } else if rematch {
            if game.0.board.outcome() == Outcome::Playing {
                commands.entity(entity).insert(InteractionDisabled);
                background.0 = PAPER;
            } else {
                commands.entity(entity).remove::<InteractionDisabled>();
                background.0 = Color::srgb(0.79, 0.87, 0.77);
            }
        }
    }
    announce(&game.0, &message);
}

// Read-only Rust state for screen readers and browser verification.
fn announce(game: &Match, message: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Some(element) = document.get_element_by_id("game-status") {
            element.set_text_content(Some(message));
            let cells: String = game
                .board
                .cells()
                .iter()
                .map(|m| m.map_or('.', |m| if m == Mark::X { 'X' } else { 'O' }))
                .collect();
            let _ = element.set_attribute("data-cells", &cells);
            let _ = element.set_attribute("data-turn", game.board.turn().label());
            let _ = element.set_attribute("data-human", game.human.label());
            let _ = element.set_attribute(
                "data-score",
                &format!("{},{},{}", game.score[0], game.score[1], game.score[2]),
            );
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (game, message);
}
