//! A simplified implementation of the classic game "Breakout".

use std::{
    collections::{HashMap, HashSet},
    ops::{Add, Mul},
};

use bevy::{
    prelude::*,
    window::{WindowResized, WindowResolution},
};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
use web_sys;

// The shared library between server and client
use shared::GameClient;

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PLAYER_SIZE: Vec3 = Vec3::new(120.0, 120.0, 0.0);
const GAP_BETWEEN_PLAYER_AND_FLOOR: f32 = 60.0;
const PLAYER_SPEED: f32 = 500.0;
const PLAYER_COLOR: Color = Color::srgb(0.3, 0.3, 0.7);

// Map constants
const BACKGROUND_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const MAP_SIZE: u32 = 5000;
const GRID_WIDTH: f32 = 1.0;
const UNITS_BETWEEN_LINES: f32 = 100.0;

fn main() {
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
                sync_clients_to_players,
                update_existing_player_positions,
                // collide_player,
            )
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(Update, (handle_zoom, on_resize_system))
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

#[cfg(target_arch = "wasm32")]
fn console_log(message: &String) {
    web_sys::console::log_1(&message.into());
}
#[cfg(not(target_arch = "wasm32"))]
fn console_log(message: &String) {
    println!("{}", &message);
}

// Add the game's entities to our world
fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Camera
    commands.spawn(Camera2d::default());

    // Player
    let player_y = -300.0 + GAP_BETWEEN_PLAYER_AND_FLOOR;
    commands.spawn((
        Transform {
            // Position player forward, in-front of the background
            translation: Vec3::new(0., player_y, 1.),
            scale: PLAYER_SIZE,
            ..default()
        },
        Mesh2d(meshes.add(Mesh::from(Circle::default())).into()),
        MeshMaterial2d(materials.add(ColorMaterial::from(PLAYER_COLOR))),
        Player::default(),
        Collider,
    ));
}

fn setup_map(mut commands: Commands) {
    // Horizontal lines
    for i in 0..=(MAP_SIZE / UNITS_BETWEEN_LINES as u32) {
        commands.spawn((
            Sprite::from_color(
                Color::srgb(0.27, 0.27, 0.27),
                Vec2::new(MAP_SIZE as f32, GRID_WIDTH),
            ),
            Transform::from_translation(Vec3::new(
                0.,
                ((i as f32) * UNITS_BETWEEN_LINES) - MAP_SIZE as f32 / 2.,
                0.,
            )),
        ));
    }

    // Vertical lines
    for i in 0..=(MAP_SIZE / UNITS_BETWEEN_LINES as u32) {
        commands.spawn((
            Sprite::from_color(
                Color::srgb(0.27, 0.27, 0.27),
                Vec2::new(GRID_WIDTH, MAP_SIZE as f32),
            ),
            Transform::from_translation(Vec3::new(
                ((i as f32) * UNITS_BETWEEN_LINES) - MAP_SIZE as f32 / 2.,
                0.,
                0.,
            )),
        ));
    }
}

/// This system shows how to respond to a window being resized.
fn on_resize_system(
    mut windows: Query<&mut Window>,
    mut resize_reader: EventReader<WindowResized>,
) {
    let mut window = windows.single_mut().unwrap();
    for e in resize_reader.read() {
        window.resolution.set(e.width, e.height);
    }
}

fn collide_player(
    other_players_query: Query<&Transform, (With<Collider>, With<OtherPlayer>, Without<Player>)>,
    mut player_query: Query<&mut Transform, (With<Collider>, With<Player>, Without<OtherPlayer>)>,
) {
    let mut player_transform = player_query.single_mut().unwrap();
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
    gamepads: Query<&Gamepad>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query_set: ParamSet<(
        Query<&mut Transform, (With<Player>, Without<OtherPlayer>)>,
        Query<&Transform, (With<OtherPlayer>, Without<Player>)>,
        Query<&mut Transform, (With<Camera>, Without<Player>)>,
    )>,
    time: Res<Time>,
) {
    let mut x = 0.0;
    let mut y = 0.0;

    // Handle gamepad input
    if let Some(gamepad) = gamepads.iter().next() {
        let x_axis = gamepad.get(GamepadAxis::LeftStickX);
        let y_axis = gamepad.get(GamepadAxis::LeftStickY);

        if let (Some(xval), Some(yval)) = (x_axis, y_axis) {
            if xval.abs() > 0.1 {
                x = xval;
            }
            if yval.abs() > 0.1 {
                y = yval;
            }
        }
    }

    // Handle keyboard input
    {
        if keyboard_input.pressed(KeyCode::KeyS) {
            y = -1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyW) {
            y = 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            x = -1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            x = 1.0;
        }
    }

    let new_translation = {
        query_set.p0().single().unwrap().translation.add(
            Vec3::new(1.0, 1.0, 0.0)
                .mul(Vec3::new(x, y, 0.0))
                .mul(PLAYER_SPEED * time.delta_secs()),
        )
    };

    // Check for collision, assume players are round
    for other_player in query_set.p1().iter() {
        if new_translation.distance(other_player.translation) < PLAYER_SIZE.x {
            return;
        }
    }

    // Now move player
    let mut player_query = query_set.p0();
    let mut player_transform = player_query.single_mut().unwrap();
    player_transform.translation = new_translation;

    // Set camera center to match player's
    // let mut cameras = query_set.p2();
    // let mut cameras = query_set.p2();
    // for mut transform in cameras.iter_mut() {
    for mut camera_transform in query_set.p2().iter_mut() {
        camera_transform.translation.x = new_translation.x;
        camera_transform.translation.y = new_translation.y;
    }
}

fn handle_zoom(
    gamepads: Query<&Gamepad>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut proj_query: Query<&mut Projection, With<Camera>>,
    time: Res<Time>,
) {
    for mut projection in proj_query.iter_mut() {
        let scale_amt = 1.5;

        let Some(ortho) = (match projection.as_mut() {
            Projection::Orthographic(o) => Some(o),
            _ => None,
        }) else {
            continue;
        };
        let mut log_scale = ortho.scale.ln();

        // Keyboard input
        if keyboard_input.pressed(KeyCode::PageUp) {
            log_scale -= scale_amt * time.delta_secs();
        }
        if keyboard_input.pressed(KeyCode::PageDown) {
            log_scale += scale_amt * time.delta_secs();
        }

        // Gamepad input
        if let Some(gamepad) = gamepads.iter().next() {
            if let Some(left_trigger) = gamepad.get(GamepadButton::LeftTrigger2) {
                if left_trigger.abs() > 0.01 {
                    log_scale += left_trigger * scale_amt * time.delta_secs();
                }
            }

            if let Some(right_trigger) = gamepad.get(GamepadButton::RightTrigger2) {
                if right_trigger.abs() > 0.01 {
                    log_scale -= right_trigger * scale_amt * time.delta_secs();
                }
            }
        }

        // Set new value
        ortho.scale = log_scale.exp();
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
            commands.entity(entity).despawn();
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
                Transform {
                    translation: Vec3::new(position[0], position[1], 1.),
                    scale: PLAYER_SIZE,
                    ..default()
                },
                Mesh2d(meshes.add(Circle::default())),
                MeshMaterial2d(materials.add(ColorMaterial::from(PLAYER_COLOR))),
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
                // parent.spawn(Text2dBundle {
                //     text: Text {
                //         sections: vec![TextSection::new(uuid.to_string(), TextStyle::default())],
                //         alignment: TextAlignment::Center,
                //         linebreak_behavior: BreakLineOn::AnyCharacter,
                //     },
                //     text_2d_bounds: Text2dBounds {
                //         // Wrap text in the rectangle
                //         size: PLAYER_SIZE.xy(),
                //     },
                //     // ensure the text is drawn on top of the box
                //     transform: Transform {
                //         translation: Vec3::new(0., 0., 2.),
                //         rotation: Quat::IDENTITY,
                //         scale: Vec3::new(0.01, 0.01, 10.),
                //     },
                //     ..default()
                // });
                //
                // parent.spawn((
                //     Text2D {},
                //     TextLayout {

                //     }
                // ));
            });
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}
