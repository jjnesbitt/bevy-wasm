//! A simplified implementation of the classic game "Breakout".

use std::{
    collections::{HashMap, HashSet},
    ops::{Add, Mul},
};
use wasm_bindgen::prelude::*;

use bevy::{
    prelude::*,
    sprite::MaterialMesh2dBundle,
    text::{BreakLineOn, Text2dBounds},
    utils::Uuid,
    window::{WindowResized, WindowResolution},
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
    fn readLatestMessage() -> Option<String>;
}

fn main() {
    // Create web socket before starting app
    // The code to create a websocket in rust is extremely verbose and complicated. So I opted
    // to create the websocket in JS and expose the method to send data
    createWebSocket();

    // Start app
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#game-canvas".into()),
                resolution: WindowResolution::new(1920., 1080.),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .insert_resource(ClientPositions { map: default() })
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
                read_client_messages,
                sync_clients_to_players,
                update_existing_player_positions,
                collide_player,
            )
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(
            Update,
            (handle_zoom, on_resize_system, bevy::window::close_on_esc),
        )
        .run();
}

#[derive(Component, Default)]
struct Player {
    colliding: bool,
}

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

#[derive(Resource)]
struct ClientPositions {
    map: HashMap<Uuid, [f32; 2]>,
}

fn console_log(message: &String) {
    web_sys::console::log_1(&message.into());
}

// Add the game's entities to our world
fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Player
    let player_y = -300.0 + GAP_BETWEEN_PLAYER_AND_FLOOR;
    commands.spawn((
        MaterialMesh2dBundle {
            transform: Transform {
                // Position player forward, in-front of the background
                translation: Vec3::new(0., player_y, 1.),
                scale: PLAYER_SIZE,
                ..default()
            },
            mesh: meshes.add(Mesh::from(shape::Circle::default())).into(),
            material: materials.add(ColorMaterial::from(PLAYER_COLOR)),
            ..default()
        },
        Player::default(),
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

/// This system shows how to respond to a window being resized.
fn on_resize_system(
    mut windows: Query<&mut Window>,
    mut resize_reader: EventReader<WindowResized>,
) {
    let mut window = windows.single_mut();
    for e in resize_reader.read() {
        console_log(&format!("{:.2}, {:.2}", e.width, e.height));
        window.resolution.set(e.width, e.height);
    }
}

fn collide_player(
    other_players_query: Query<&Transform, (With<Collider>, With<OtherPlayer>, Without<Player>)>,
    mut player_query: Query<&mut Transform, (With<Collider>, With<Player>, Without<OtherPlayer>)>,
) {
    let mut player_transform = player_query.single_mut();
    for other_player_transform in other_players_query.iter() {
        let dist_to_other_player = other_player_transform
            .translation
            .distance(player_transform.translation);

        // Assumes player is round
        if dist_to_other_player >= PLAYER_SIZE.x {
            continue;
        }

        // Place player at the edge
        let vector = player_transform.translation.xy() - other_player_transform.translation.xy();
        let scale_factor = PLAYER_SIZE.x / vector.length();
        let vector_addition = vector * scale_factor;
        player_transform.translation = other_player_transform
            .translation
            .add(vector_addition.extend(0.));
    }
}

fn move_player(
    gamepads: Res<Gamepads>,
    gamepad_axis: Res<Axis<GamepadAxis>>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<Player>)>,
    time: Res<Time>,
) {
    let mut x = 0.0;
    let mut y = 0.0;

    // Handle gamepad input
    if let Some(gamepad) = gamepads.iter().next() {
        let x_axis = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickX,
        };
        let y_axis = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickY,
        };

        if let (Some(gamepad_x), Some(gamepad_y)) =
            (gamepad_axis.get(x_axis), gamepad_axis.get(y_axis))
        {
            (x, y) = (gamepad_x, gamepad_y);
        }
    }

    // Handle keyboard input
    {
        if keyboard_input.pressed(KeyCode::S) {
            y = -1.0;
        }
        if keyboard_input.pressed(KeyCode::W) {
            y = 1.0;
        }
        if keyboard_input.pressed(KeyCode::A) {
            x = -1.0;
        }
        if keyboard_input.pressed(KeyCode::D) {
            x = 1.0;
        }
    }

    // Now move player
    let mut player_transform = query.single_mut();
    player_transform.translation = player_transform.translation.add(
        Vec3::new(1.0, 1.0, 0.0)
            .mul(Vec3::new(x, y, 0.0))
            .mul(PLAYER_SPEED * time.delta_seconds()),
    );

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

fn read_client_messages(mut positions: ResMut<ClientPositions>) {
    let Some(msg) = readLatestMessage() else {
        return;
    };
    let Some(clients) = serde_json::from_str::<Vec<GameClient>>(&msg).ok() else {
        return;
    };

    positions.map.clear();
    for client in clients.iter() {
        positions.map.insert(client.uuid, client.position);
    }
}

fn update_existing_player_positions(
    mut query: Query<(&mut Transform, &OtherPlayer)>,
    positions: Res<ClientPositions>,
) {
    for (mut transform, player) in query.iter_mut() {
        if let Some(pos) = positions.map.get(&player.client.uuid) {
            transform.translation.x = pos[0];
            transform.translation.y = pos[1];
        }
    }
}

fn sync_clients_to_players(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(Entity, &OtherPlayer)>,
    clients_pos: Res<ClientPositions>,
) {
    // Get existing set of active players
    // Remove any players that aren't in the active client list
    let mut player_set = HashSet::<Uuid>::new();
    for (entity, player) in query.iter() {
        if clients_pos.map.contains_key(&player.client.uuid) {
            player_set.insert(player.client.uuid);
        } else {
            commands.entity(entity).despawn_recursive();
        }
    }

    // Determine new clients by checking against keys in ClientPositions
    let new_clients = clients_pos
        .map
        .iter()
        .filter(|(&uuid, _)| !player_set.contains(&uuid));

    // Add new clients
    for (uuid, position) in new_clients {
        commands
            .spawn((
                MaterialMesh2dBundle {
                    // Position player forward, in-front of the background
                    transform: Transform {
                        translation: Vec3::new(position[0], position[1], 1.),
                        scale: PLAYER_SIZE,
                        ..default()
                    },
                    mesh: meshes.add(Mesh::from(shape::Circle::default())).into(),
                    material: materials.add(ColorMaterial::from(PLAYER_COLOR)),
                    ..default()
                },
                Collider,
                OtherPlayer {
                    client: GameClient {
                        uuid: uuid.clone(),
                        position: position.clone(),
                    },
                },
            ))
            // Add text to display other player name/id
            .with_children(|parent| {
                parent.spawn(Text2dBundle {
                    text: Text {
                        sections: vec![TextSection::new(uuid.to_string(), TextStyle::default())],
                        alignment: TextAlignment::Center,
                        linebreak_behavior: BreakLineOn::AnyCharacter,
                    },
                    text_2d_bounds: Text2dBounds {
                        // Wrap text in the rectangle
                        size: PLAYER_SIZE.xy(),
                    },
                    // ensure the text is drawn on top of the box
                    transform: Transform {
                        translation: Vec3::new(0., 0., 2.),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.01, 0.01, 10.),
                    },
                    ..default()
                });
            });
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}
