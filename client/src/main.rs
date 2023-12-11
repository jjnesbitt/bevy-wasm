//! A simplified implementation of the classic game "Breakout".

use std::ops::{Add, Mul};
use wasm_bindgen::prelude::*;

use bevy::{
    prelude::*,
    text::{BreakLineOn, Text2dBounds},
};
use web_sys;

// The shared library between server and client
use shared::GameClient;

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PLAYER_SIZE: Vec3 = Vec3::new(120.0, 120.0, 0.0);
const GAP_BETWEEN_PLAYER_AND_FLOOR: f32 = 60.0;
const PLAYER_SPEED: f32 = 500.0;
const PLAYER_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);

// Map constants
const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const MAP_SIZE: u32 = 5000;
const GRID_WIDTH: f32 = 1.0;
const UNITS_BETWEEN_LINES: f32 = 100.0;

#[wasm_bindgen(module = "/js/foo.js")]
extern "C" {
    fn sendPosition(x: f32, y: f32);
    fn createWebSocket();
    fn readMessages() -> Vec<String>;
}

fn main() {
    // Create web socket before starting app
    // The code to create a websocket in rust is extremely verbose and complicated. So I opted
    // to create the websocket in JS and expose the method to send data
    createWebSocket();

    // Start app
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_event::<CollisionEvent>()
        .add_systems(Startup, (setup, setup_map))
        // Add our gameplay simulation systems to the fixed timestep schedule
        // which runs at 64 Hz by default
        .add_systems(
            FixedUpdate,
            (
                apply_velocity,
                move_player,
                send_player_position,
                read_web_socket,
            )
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(
            Update,
            (read_gamepads, handle_zoom, bevy::window::close_on_esc),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct OtherPlayer {
    client: GameClient,
}

#[derive(Component)]
struct Ball;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Component)]
struct Collider;

#[derive(Event, Default)]
struct CollisionEvent;

#[derive(Component)]
struct Brick;

#[derive(Resource)]
struct CollisionSound(Handle<AudioSource>);

fn console_log(message: &String) {
    web_sys::console::log_1(&message.into());
}

// Add the game's entities to our world
fn setup(mut commands: Commands) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Player
    let player_y = -300.0 + GAP_BETWEEN_PLAYER_AND_FLOOR;
    commands.spawn((
        SpriteBundle {
            transform: Transform {
                translation: Vec3::new(0.0, player_y, 50.0),
                scale: PLAYER_SIZE,
                ..default()
            },
            sprite: Sprite {
                color: PLAYER_COLOR,
                ..default()
            },
            ..default()
        },
        Player,
        Collider,
    ));
}

fn setup_map(mut commands: Commands) {
    // Horizontal lines
    for i in 0..=(MAP_SIZE / UNITS_BETWEEN_LINES as u32) {
        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                0.,
                ((i as f32) * UNITS_BETWEEN_LINES) - MAP_SIZE as f32 / 2.,
                0.,
            )),
            sprite: Sprite {
                color: Color::rgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(MAP_SIZE as f32, GRID_WIDTH)),
                ..default()
            },
            ..default()
        });
    }

    // Vertical lines
    for i in 0..=(MAP_SIZE / UNITS_BETWEEN_LINES as u32) {
        commands.spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(
                ((i as f32) * UNITS_BETWEEN_LINES) - MAP_SIZE as f32 / 2.,
                0.,
                0.,
            )),
            sprite: Sprite {
                color: Color::rgb(0.27, 0.27, 0.27),
                custom_size: Some(Vec2::new(GRID_WIDTH, MAP_SIZE as f32)),
                ..default()
            },
            ..default()
        });
    }
}

fn read_gamepads(
    gamepads: Res<Gamepads>,
    gamepad_axis: Res<Axis<GamepadAxis>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    let mut translation = Vec3::new(1.0, 1.0, 0.0);
    let Some(gamepad) = gamepads.iter().next() else {
        return;
    };

    let x_axis = GamepadAxis {
        gamepad,
        axis_type: GamepadAxisType::LeftStickX,
    };
    let y_axis = GamepadAxis {
        gamepad,
        axis_type: GamepadAxisType::LeftStickY,
    };

    if let (Some(x), Some(y)) = (gamepad_axis.get(x_axis), gamepad_axis.get(y_axis)) {
        translation = translation
            .mul(Vec3::new(x, y, 0.0))
            .mul(PLAYER_SPEED * time.delta_seconds());
    }

    // Set new location
    let mut player_transform = query.single_mut();
    player_transform.translation = player_transform.translation.add(translation);
}

fn move_player(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<Player>)>,
    time: Res<Time>,
) {
    let mut player_transform = query.single_mut();
    let mut x_direction = 0.0;
    let mut y_direction = 0.0;

    if keyboard_input.pressed(KeyCode::S) {
        y_direction = -1.0;
    }
    if keyboard_input.pressed(KeyCode::W) {
        y_direction = 1.0;
    }
    if keyboard_input.pressed(KeyCode::A) {
        x_direction = -1.0;
    }
    if keyboard_input.pressed(KeyCode::D) {
        x_direction = 1.0;
    }

    // Calculate the new horizontal player position based on player input
    player_transform.translation.x =
        player_transform.translation.x + x_direction * PLAYER_SPEED * time.delta_seconds();
    player_transform.translation.y =
        player_transform.translation.y + y_direction * PLAYER_SPEED * time.delta_seconds();

    // Set camera center to match player's
    for mut transform in &mut cameras {
        transform.translation.x = player_transform.translation.x;
        transform.translation.y = player_transform.translation.y;
    }
}

fn handle_zoom(
    gamepads: Res<Gamepads>,
    keyboard_input: Res<Input<KeyCode>>,
    gamepad_input: Res<Input<GamepadButton>>,
    mut proj_query: Query<&mut OrthographicProjection, With<Camera>>,
    time: Res<Time>,
) {
    for mut projection in proj_query.iter_mut() {
        let scale_amt = 1.5;
        let mut log_scale = projection.scale.ln();

        // Keyboard input
        if keyboard_input.pressed(KeyCode::PageUp) {
            log_scale -= scale_amt * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::PageDown) {
            log_scale += scale_amt * time.delta_seconds();
        }

        // Gamepad input
        if let Some(gamepad) = gamepads.iter().next() {
            let left_trigger = GamepadButton {
                gamepad,
                button_type: GamepadButtonType::LeftTrigger2,
            };
            if gamepad_input.pressed(left_trigger) {
                log_scale += scale_amt * time.delta_seconds();
            }

            let right_trigger = GamepadButton {
                gamepad,
                button_type: GamepadButtonType::RightTrigger2,
            };
            if gamepad_input.pressed(right_trigger) {
                log_scale -= scale_amt * time.delta_seconds();
            }
        }

        // Set new value
        projection.scale = log_scale.exp();
    }
}

fn send_player_position(query: Query<&Transform, With<Player>>) {
    let player_transform = query.single();
    let Vec2 { x, y } = player_transform.translation.xy();

    sendPosition(x, y);
}

fn read_web_socket(mut commands: Commands, query: Query<(Entity, &OtherPlayer)>) {
    let messages = readMessages();

    // Each message is a list of clients, so just take the last message
    let Some(msg) = messages.last() else {
        return;
    };

    // TODO: This sucks and should be refactored
    // We should instead only remove entities that have actually disconnected,
    // and should just update the ones that are still present

    // Despawn all players
    for (entity, _) in query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Create an entity for each other player found
    if let Some(clients) = serde_json::from_str::<Vec<GameClient>>(&msg).ok() {
        for client in clients.iter() {
            commands
                .spawn((
                    SpriteBundle {
                        transform: Transform {
                            // Position player forward, in-front of the background
                            translation: Vec3::new(client.position[0], client.position[1], 50.0),
                            ..default()
                        },
                        sprite: Sprite {
                            color: PLAYER_COLOR,
                            custom_size: Some(PLAYER_SIZE.xy()),
                            ..default()
                        },
                        ..default()
                    },
                    Collider,
                    OtherPlayer {
                        client: client.clone(),
                    },
                ))
                // Add text to display other player name/id
                .with_children(|parent| {
                    parent.spawn(Text2dBundle {
                        text: Text {
                            sections: vec![TextSection::new(
                                &client.uuid.to_string(),
                                TextStyle::default(),
                            )],
                            alignment: TextAlignment::Center,
                            linebreak_behavior: BreakLineOn::AnyCharacter,
                        },
                        text_2d_bounds: Text2dBounds {
                            // Wrap text in the rectangle
                            size: PLAYER_SIZE.xy(),
                        },
                        // ensure the text is drawn on top of the box
                        transform: Transform::from_translation(Vec3::Z),
                        ..default()
                    });
                });
        }
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}
