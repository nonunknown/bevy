use bevy::{
    core::FixedTimestep,
    prelude::*,
    render::pass::ClearColor,
    sprite::collide_aabb::{collide, Collision},
};

use components::*;
use resources::*;

/// Constants that can be used to fine-tune the behavior of our game
mod config {
    use super::Velocity;
    use bevy::math::Vec2;
    use bevy::render::color::Color;
    use bevy::transform::components::Transform;
    use bevy::ui::Val;

    pub const TIME_STEP: f32 = 1.0 / 60.0;
    pub const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);

    pub const PADDLE_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
    pub const PADDLE_STARTING_LOCATION: Transform = Transform::from_xyz(0.0, -215.0, 0.0);
    pub const PADDLE_SIZE: Vec2 = Vec2::new(120.0, 30.0);
    pub const PADDLE_SPEED: f32 = 500.0;

    pub const BALL_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
    // We set the z-value to one to ensure it appears on top of our other objects in case of overlap
    pub const BALL_STARTING_LOCATION: Transform = Transform::from_xyz(0.0, -50.0, 1.0);
    // Our ball is actually a square. Shhh...
    pub const BALL_SIZE: Vec2 = Vec2::new(30.0, 30.0);
    const BALL_STARTING_DIRECTION: Vec2 = Vec2::new(0.5, -0.5).normalize();
    const BALL_STARTING_SPEED: f32 = 400.0;
    pub const BALL_STARTING_VELOCITY: Velocity = Velocity {
        x: BALL_STARTING_DIRECTION.x * BALL_STARTING_SPEED,
        y: BALL_STARTING_DIRECTION.y * BALL_STARTING_SPEED,
    };

    pub const ARENA_BOUNDS: Vec2 = Vec2::new(900.0, 600.0);
    pub const WALL_THICKNESS: f32 = 10.0;
    pub const WALL_COLOR: Color = Color::rgb(0.8, 0.8, 0.8);

    pub const BRICK_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);

    pub const SCOREBOARD_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
    pub const SCORE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
    pub const SCORE_FONT_SIZE: f32 = 40.0;
    pub const SCORE_PADDING: Val = Val::Px(5.0);
}

/// A simple implementation of the classic game "Breakout"
fn main() {
    App::build()
        .add_plugins(DefaultPlugins)
        .insert_resource(ClearColor(config::BACKGROUND_COLOR))
        // This adds the Score resource with its default values: 0
        .init_resource::<Score>()
        // These systems run only once, before all other systems
        .add_startup_system(spawn_cameras.system())
        .add_startup_system(spawn_paddle.system())
        .add_startup_system(spawn_ball.system())
        .add_startup_system(spawn_walls.system())
        .add_startup_system(spawn_scoreboard.system())
        // These systems run repeatedly, whnever the FixedTimeStep's duration has elapsed
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(config::TIME_STEP as f64))
                .with_system(kinematics.system())
                .with_system(paddle_input.system())
                .with_system(ball_collision.system()),
        )
        // Ordinary systems run every frame
        .add_system(bound_paddle.system())
        .add_system(update_scoreboard.system())
        .run();
}

mod resources {
    #[derive(Default)]
    pub struct Score(pub usize);
}

mod components {
    pub struct Paddle {
        pub speed: f32,
    }
    // These are data-less marker components
    // which let us query for the correct entities
    // and specialize behavior
    pub struct Ball;
    pub struct Brick;
    pub struct Scoreboard;
    pub struct Collides;

    // The derived default values of numeric fields in Rust are zero
    #[derive(Default)]
    pub struct Velocity {
        pub x: f32,
        pub y: f32,
    }
}

fn spawn_cameras(mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());
}

fn spawn_paddle(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands
        .spawn_bundle(SpriteBundle {
            material: materials.add(config::PADDLE_COLOR.into()),
            transform: config::PADDLE_STARTING_LOCATION,
            sprite: Sprite::new(config::PADDLE_SIZE),
            ..Default::default()
        })
        .insert(Paddle {
            speed: config::PADDLE_SPEED,
        })
        .insert(Collides)
        .insert(Velocity::default());
}

fn spawn_ball(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    commands
        .spawn_bundle(SpriteBundle {
            material: materials.add(config::BALL_COLOR.into()),
            transform: config::BALL_STARTING_LOCATION,
            sprite: Sprite::new(config::BALL_SIZE),
            ..Default::default()
        })
        .insert(Ball)
        .insert(Collides)
        // Adds a `Velocity` component with the value defined in the `config` module
        .insert(config::BALL_STARTING_VELOCITY);
}

/// Defines which side of the arena a wall is part of
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    fn wall_coord(self, bounds: Vec2) -> Transform {
        let (x, y) = match self {
            Side::Top => (0.0, bounds.y / 2.0),
            Side::Bottom => (0.0, -bounds.y / 2.0),
            Side::Left => (-bounds.x / 2.0, 0.0),
            Side::Right => (bounds.x / 2.0, 0.0),
        };
        // We need to convert these coordinates into a 3D transform to add to our SpriteBundle
        Transform::from_xyz(x, y, 0.0)
    }

    fn wall_size(self, bounds: Vec2, thickness: f32) -> Vec2 {
        match self {
            Side::Top => Vec2::new(thickness, bounds.y + thickness),
            Side::Bottom => Vec2::new(thickness, bounds.y + thickness),
            Side::Left => Vec2::new(bounds.x + thickness, thickness),
            Side::Right => Vec2::new(bounds.x + thickness, thickness),
        }
    }
}

// By creating our own bundles, we can avoid duplicating code
#[derive(Bundle)]
struct WallBundle {
    #[bundle]
    sprite_bundle: SpriteBundle,
    collides: Collides,
}

impl WallBundle {
    fn new(side: Side, material_handle: Handle<ColorMaterial>) -> Self {
        let bounds = config::ARENA_BOUNDS;
        let thickness = config::WALL_THICKNESS;

        WallBundle {
            sprite_bundle: SpriteBundle {
                material: material_handle.clone(),
                transform: side.wall_coord(bounds),
                sprite: Sprite::new(side.wall_size(bounds, thickness)),
                ..Default::default()
            },
            collides: Collides,
        }
    }
}

fn spawn_walls(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    let material_handle = materials.add(config::WALL_COLOR.into());

    commands.spawn_bundle(WallBundle::new(Side::Top, material_handle));
    commands.spawn_bundle(WallBundle::new(Side::Bottom, material_handle));
    commands.spawn_bundle(WallBundle::new(Side::Left, material_handle));
    commands.spawn_bundle(WallBundle::new(Side::Right, material_handle));
}

fn add_bricks(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    let brick_material = materials.add(config::BRICK_COLOR.into());

    // Brick layout constants
    const brick_rows: i8 = 4;
    const brick_columns: i8 = 5;
    const brick_spacing: f32 = 20.0;
    const brick_size: Vec2 = Vec2::new(150.0, 30.0);

    // Compute the total width that all of the bricks take
    let total_width = brick_columns as f32 * (brick_size.x + brick_spacing) - brick_spacing;
    // Center the bricks and move them up a bit
    let bricks_offset = Vec3::new(-(total_width - brick_size.x) / 2.0, 100.0, 0.0);

    // Add the bricks
    for row in 0..brick_rows {
        for column in 0..brick_columns {
            let brick_position = Vec3::new(
                column as f32 * (brick_size.x + brick_spacing),
                row as f32 * (brick_size.y + brick_spacing),
                0.0,
            ) + bricks_offset;
            // Adding one brick at a time
            commands
                .spawn_bundle(SpriteBundle {
                    material: brick_material.clone(),
                    sprite: Sprite::new(brick_size),
                    transform: Transform::from_translation(brick_position),
                    ..Default::default()
                })
                .insert(Brick)
                .insert(Collides);
        }
    }
}

fn spawn_scoreboard(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(TextBundle {
        text: Text {
            sections: vec![
                TextSection {
                    value: "Score: ".to_string(),
                    style: TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: config::SCORE_FONT_SIZE,
                        color: config::SCOREBOARD_COLOR,
                    },
                },
                TextSection {
                    value: "".to_string(),
                    style: TextStyle {
                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: config::SCORE_FONT_SIZE,
                        color: config::SCORE_COLOR,
                    },
                },
            ],
            ..Default::default()
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                top: config::SCORE_PADDING,
                left: config::SCORE_PADDING,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    });
}

/// Moves everything with both a Transform and a Velovity accordingly
fn kinematics(mut query: Query<(&mut Transform, &Velocity)>) {
    for (transform, velocity) in query.iter_mut() {
        transform.translation.x += velocity.x * config::TIME_STEP;
        transform.translation.y += velocity.y * config::TIME_STEP;
    }
}

/// Turns left and right arrow key inputs to set paddle velocity
fn paddle_input(keyboard_input: Res<Input<KeyCode>>, mut query: Query<(&Paddle, &mut Velocity)>) {
    let (paddle, mut velocity) = query.single_mut().unwrap();

    let mut direction = 0.0;
    if keyboard_input.pressed(KeyCode::Left) {
        direction -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::Right) {
        direction += 1.0;
    }

    velocity.x += direction * paddle.speed;
}

/// Ensures our paddle never goes out of bounds
fn bound_paddle(mut query: Query<&mut Transform, With<Paddle>>) {
    let mut paddle_transform = query.single_mut().unwrap();
    paddle_transform.translation.x = paddle_transform.translation.x.min(380.0).max(-380.0);
}

fn ball_collision(
    mut ball_query: Query<(&Transform, &mut Velocity, &Sprite), With<Ball>>,
    // Option<&C> returns Some(c: C) if the component exists on the entity, and None if it does not
    collider_query: Query<
        (Entity, &Transform, &Sprite, Option<&Brick>),
        (With<Collides>, Without<Ball>),
    >,
    mut commands: Commands,
    mut score: ResMut<Score>,
) {
    let (ball_transform, mut ball_velocity, ball_sprite) = ball_query.single_mut().unwrap();
    let ball_size = ball_sprite.size;

    for (collider_entity, collider_transform, collider_sprite, maybe_brick) in collider_query.iter()
    {
        // Check for collisions
        let collider_size = collider_sprite.size;
        let potential_collision = collide(
            ball_transform.translation,
            ball_size,
            collider_transform.translation,
            collider_size,
        );

        // Handle collisions
        if let Some(collision) = potential_collision {
            // Reflect the ball when it collides
            let mut reflect_x = false;
            let mut reflect_y = false;

            // Only reflect if the ball's velocity is going
            // in the opposite direction of the collision
            match collision {
                Collision::Left => reflect_x = ball_velocity.x > 0.0,
                Collision::Right => reflect_x = ball_velocity.x < 0.0,
                Collision::Top => reflect_y = ball_velocity.y < 0.0,
                Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
            }

            // Reflect velocity on the x-axis if we hit something on the x-axis
            if reflect_x {
                ball_velocity.x = -ball_velocity.x;
            }

            // Reflect velocity on the y-axis if we hit something on the y-axis
            if reflect_y {
                ball_velocity.y = -ball_velocity.y;
            }

            // Perform special brick collision behavior
            if maybe_brick.is_some() {
                // Despawn bricks that are hit
                commands.entity(collider_entity).despawn();

                // Increase the score by 1 for each brick hit
                score.0 += 1;
            }
        }
    }
}

/// Updates the Scoreboard entity based on the Score resource
fn update_scoreboard(score: Res<Score>, mut query: Query<&mut Text, With<Scoreboard>>) {
    let mut scoreboard_text = query.single_mut().unwrap();
    scoreboard_text.sections[0].value = format!("Score: {}", score.0);
}
