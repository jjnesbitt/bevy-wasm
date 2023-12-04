//! A simplified implementation of the classic game "Breakout".

use std::ops::{Add, Mul};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use bevy::{
    input::gamepad::{GamepadButtonInput, GamepadEvent},
    prelude::*,
    sprite::collide_aabb::{collide, Collision},
    sprite::MaterialMesh2dBundle,
};
use web_sys;
use web_sys::{Request, RequestInit, RequestMode, Response};

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PLAYER_SIZE: Vec3 = Vec3::new(120.0, 120.0, 0.0);
const GAP_BETWEEN_PLAYER_AND_FLOOR: f32 = 60.0;
const PLAYER_SPEED: f32 = 500.0;

const WALL_THICKNESS: f32 = 10.0;
// x coordinates
const LEFT_WALL: f32 = -450.;
const RIGHT_WALL: f32 = 450.;
// y coordinates
const BOTTOM_WALL: f32 = -300.;
const TOP_WALL: f32 = 300.;

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const PLAYER_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);
const WALL_COLOR: Color = Color::rgb(0.8, 0.8, 0.8);

#[wasm_bindgen]
pub async fn myfunc() -> Result<JsValue, JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let url = "http://localhost:3000";
    let request = Request::new_with_str_and_init(&url, &opts)?;
    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let json = JsFuture::from(resp.json()?).await?;

    // Send the JSON response back to JS.
    Ok(json)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Scoreboard { score: 0 })
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_event::<CollisionEvent>()
        .add_systems(Startup, setup)
        // Add our gameplay simulation systems to the fixed timestep schedule
        // which runs at 64 Hz by default
        .add_systems(
            FixedUpdate,
            (
                apply_velocity,
                move_player,
                check_for_wall_collisions,
                check_for_ball_collisions,
            )
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(Update, (read_gamepads, bevy::window::close_on_esc))
        .run();
}

// #[derive(Component)]
// struct Position {
//     x: f32,
//     y: f32,
// }

#[derive(Event, Default)]
struct MoveEvent {
    key: String,
}

#[derive(Component)]
struct Player;

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

#[derive(Component)]
struct Wall;

// This bundle is a collection of the components that define a "wall" in our game
#[derive(Bundle)]
struct WallBundle {
    // You can nest bundles inside of other bundles like this
    // Allowing you to compose their functionality
    sprite_bundle: SpriteBundle,
    collider: Collider,
    wall: Wall,
}

/// Which side of the arena is this wall located on?
enum WallLocation {
    Left,
    Right,
    Bottom,
    Top,
}

impl WallLocation {
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL, 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL, 0.),
            WallLocation::Bottom => Vec2::new(0., BOTTOM_WALL),
            WallLocation::Top => Vec2::new(0., TOP_WALL),
        }
    }

    fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        // Make sure we haven't messed up our constants
        assert!(arena_height > 0.0);
        assert!(arena_width > 0.0);

        match self {
            WallLocation::Left | WallLocation::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            WallLocation::Bottom | WallLocation::Top => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
        }
    }
}

impl WallBundle {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    fn new(location: WallLocation) -> WallBundle {
        WallBundle {
            sprite_bundle: SpriteBundle {
                transform: Transform {
                    // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                    // This is used to determine the order of our sprites
                    translation: location.position().extend(0.0),
                    // The z-scale of 2D objects must always be 1.0,
                    // or their ordering will be affected in surprising ways.
                    // See https://github.com/bevyengine/bevy/issues/4149
                    scale: location.size().extend(1.0),
                    ..default()
                },
                sprite: Sprite {
                    color: WALL_COLOR,
                    ..default()
                },
                ..default()
            },
            collider: Collider,
            wall: Wall,
        }
    }
}

// This resource tracks the game's score
#[derive(Resource)]
struct Scoreboard {
    score: usize,
}

fn console_log(message: &String) {
    web_sys::console::log_1(&message.into());
}

// fn setup_window_keyevents(mut move_events: EventWriter<MoveEvent>) {
//     let window = web_sys::window().expect("global window does not exists");
//     let listener = EventListener::new(&window, "keydown", move |event| {
//         let keyboard_event = event.clone().dyn_into::<web_sys::KeyboardEvent>().unwrap();
//         move_events.send(MoveEvent {
//             key: keyboard_event.key(),
//         });
//         web_sys::console::log_1(&keyboard_event.key().into());
//     });
//     listener.forget();
// }

// Add the game's entities to our world
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Player
    let player_y = BOTTOM_WALL + GAP_BETWEEN_PLAYER_AND_FLOOR;

    commands.spawn((
        SpriteBundle {
            transform: Transform {
                translation: Vec3::new(0.0, player_y, 0.0),
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

    // Walls
    commands.spawn(WallBundle::new(WallLocation::Left));
    commands.spawn(WallBundle::new(WallLocation::Right));
    commands.spawn(WallBundle::new(WallLocation::Bottom));
    commands.spawn(WallBundle::new(WallLocation::Top));

    // Balls
    const BALL_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
    const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -50.0, 1.0);
    const BALL_SIZE: Vec3 = Vec3::new(30.0, 30.0, 0.0);
    const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);
    const BALL_SPEED: f32 = 400.0;
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::default().into()).into(),
            material: materials.add(ColorMaterial::from(BALL_COLOR)),
            transform: Transform::from_translation(BALL_STARTING_POSITION).with_scale(BALL_SIZE),
            ..default()
        },
        Ball,
        Velocity(INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED),
        Collider,
    ));
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::default().into()).into(),
            material: materials.add(ColorMaterial::from(BALL_COLOR)),
            transform: Transform::from_translation(Vec3::new(0.0, 60.0, 12.0))
                .with_scale(BALL_SIZE),
            ..default()
        },
        Ball,
        Velocity(Vec2::new(0.5, 0.5).normalize() * BALL_SPEED),
        Collider,
    ));
}

// fn read_gamepads(
//     mut gamepad_evr: EventReader<GamepadEvent>,
//     mut query: Query<&mut Transform, With<Player>>,
//     time: Res<Time>,
// ) {
//     let mut translation = Vec3::new(0.0, 0.0, 0.0);
//     for event in gamepad_evr.read() {
//         console_log(&format!("{:?}", event));
//         match event {
//             GamepadEvent::Axis(axis_event) => match axis_event.axis_type {
//                 GamepadAxisType::LeftStickX => {
//                     translation[0] = axis_event.value;
//                 }
//                 GamepadAxisType::LeftStickY => {
//                     translation[1] = axis_event.value;
//                 }
//                 _ => {}
//             },
//             _ => {}
//         }
//     }

//     // Set new location
//     let mut player_transform = query.single_mut();
//     player_transform.translation = player_transform
//         .translation
//         .add(translation.mul(PLAYER_SPEED * time.delta_seconds()));
// }

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
}

fn check_for_ball_collisions(
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut balls_query: Query<
        (
            Entity,
            &mut Handle<ColorMaterial>,
            &Transform,
            &mut Velocity,
        ),
        (With<Ball>, With<Collider>),
    >,
) {
    // Inefficient collision logic
    for ball1 in balls_query.iter() {
        for ball2 in balls_query.iter() {
            // Entity check
            if ball1.0 == ball2.0 {
                continue;
            }

            let trans1 = ball1.2;
            let trans2 = ball2.2;
            let collision = collide(
                trans1.translation,
                trans1.scale.truncate(),
                trans2.translation,
                trans2.scale.truncate(),
            );

            if collision.is_some() {
                if let Some(color_material) = materials.get_mut(ball1.1.id()) {
                    color_material.color = Color::rgb(
                        rand::random::<f32>(),
                        rand::random::<f32>(),
                        rand::random::<f32>(),
                    );
                }

                if let Some(color_material) = materials.get_mut(ball2.1.id()) {
                    color_material.color = Color::rgb(
                        rand::random::<f32>(),
                        rand::random::<f32>(),
                        rand::random::<f32>(),
                    );
                }
            }
        }
    }
}

fn check_for_wall_collisions(
    walls_query: Query<&Transform, (With<Wall>, With<Collider>)>,
    mut balls_query: Query<(&mut Velocity, &Transform), (With<Ball>, With<Collider>)>,
) {
    for wall_transform in walls_query.iter() {
        for (mut ball_vel, ball_trans) in balls_query.iter_mut() {
            let collision = collide(
                ball_trans.translation,
                ball_trans.scale.truncate(),
                wall_transform.translation,
                wall_transform.scale.truncate(),
            );
            if let Some(collision) = collision {
                // reflect the ball when it collides
                let mut reflect_x = false;
                let mut reflect_y = false;

                // only reflect if the ball's velocity is going in the opposite direction of the
                // collision
                match collision {
                    Collision::Left => reflect_x = ball_vel.x > 0.0,
                    Collision::Right => reflect_x = ball_vel.x < 0.0,
                    Collision::Top => reflect_y = ball_vel.y < 0.0,
                    Collision::Bottom => reflect_y = ball_vel.y > 0.0,
                    Collision::Inside => { /* do nothing */ }
                }

                // reflect velocity on the x-axis if we hit something on the x-axis
                if reflect_x {
                    ball_vel.x = -ball_vel.x;
                }

                // reflect velocity on the y-axis if we hit something on the y-axis
                if reflect_y {
                    ball_vel.y = -ball_vel.y;
                }
            }
        }
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}
