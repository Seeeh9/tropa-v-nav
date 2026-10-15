//! The book of the tale: a leather chronicle on a candlelit oak table, drawn in 3D.
//!
//! The scene is rendered into a picture (`Book::view`) that the story page shows in place of its
//! flat painting, so it sits in the scaled stage like any other UI. The two pages of the open
//! spread are pictures too: each is a little UI tree (parchment, framed painting, printed text)
//! rendered by its own camera into a texture, which the page meshes wear.
//!
//! This is the first step: the scene, the cover opening and the spread with its text. Pages are
//! still turned by the story's buttons; dragging and tearing them come next.

use crate::art::Art;
use crate::audio::Mixer;
use crate::theme::Fonts;
use bevy::audio::Volume;
use bevy::camera::RenderTarget;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::UiTargetCamera;
use std::f32::consts::PI;

/// Size of the rendered scene, in pixels: the stage itself.
const VIEW: UVec2 = UVec2::new(1920, 1080);
/// Size of one page's texture; pages are 1 x PAGE_H in the scene.
const PAGE_PX: UVec2 = UVec2::new(1024, 1400);
const PAGE_W: f32 = 1.0;
const PAGE_H: f32 = 1.37;
/// Height of the top of the page block above the table.
const TOP: f32 = 0.09;
/// Distance from the edge of the frame texture to its inner window, in texture pixels.
const FRAME_SLICE: f32 = 380.0;
/// Where the reader's eye rests, and the point of the spread it looks at.
const EYE: Vec3 = Vec3::new(0.0, 1.66, 1.02);
const LOOK: Vec3 = Vec3::new(0.0, 0.0, 0.16);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, (open_cover, flicker, sway_camera, redraw_pages, fit_pages));
}

/// The stage's `UiScale` fits the UI to the window, and it reaches UI drawn into textures too; the
/// pages undo it so one page pixel stays one texture pixel at any window size.
fn fit_pages(scale: Res<UiScale>, mut targets: Query<&mut RenderTarget, With<PageCamera>>) {
    let want = 1.0 / scale.0;
    for mut target in &mut targets {
        let stale = matches!(target.as_ref(), RenderTarget::Image(image) if (image.scale_factor - want).abs() > 1e-4);
        if stale {
            if let RenderTarget::Image(image) = target.as_mut() {
                image.scale_factor = want;
            }
        }
    }
}

/// What the open spread shows.
#[derive(Clone, PartialEq, Default)]
pub struct Spread {
    /// Painting on the left page, an asset stem such as `story/pro_1`.
    pub picture: String,
    /// Heading over the painting, already translated.
    pub title: Option<String>,
    /// The page's text, already translated.
    pub text: String,
    /// Page numbers printed at the foot of the two pages.
    pub numbers: (u32, u32),
}

#[derive(Resource)]
pub struct Book {
    /// The scene as a picture, for the story page to show.
    pub view: Handle<Image>,
    left: Handle<Image>,
    right: Handle<Image>,
    /// Is the book wanted on the table: the story sets this while a tale is being read.
    pub wanted: bool,
    /// What the story wants on the spread; drawn when it changes.
    pub spread: Spread,
    drawn: Option<Spread>,
    /// 0 closed .. 1 open.
    open: f32,
    /// The cover's sounds have been played for this opening.
    heard: bool,
}

#[derive(Component)]
struct BookCamera;
#[derive(Component)]
struct PageCamera;
#[derive(Component)]
struct Cover;
#[derive(Component)]
struct Candle {
    base: f32,
}
#[derive(Component)]
struct Flame;
/// The UI tree drawn onto one page: `false` the left page, `true` the right.
#[derive(Component)]
struct PageCanvas(bool);

fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None))
}

fn tiled(assets: &AssetServer, art: &Art, stem: &str) -> Option<Handle<Image>> {
    let path = art.find(stem, "png", "gemini")?;
    Some(assets.load_builder().with_settings(|s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::linear();
        s.sampler.get_or_init_descriptor().set_address_mode(ImageAddressMode::Repeat);
    }).load(path))
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    art: Res<Art>,
) {
    let view = target(&mut images, VIEW);
    let left = target(&mut images, PAGE_PX);
    let right = target(&mut images, PAGE_PX);

    // the scene's camera, asleep until a tale is read
    commands.spawn((
        BookCamera,
        Camera3d::default(),
        Camera { order: -10, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.015, 0.01)), ..default() },
        RenderTarget::Image(view.clone().into()),
        Transform::from_translation(EYE).looking_at(LOOK, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(1.0, 0.85, 0.7), brightness: 60.0, ..default() });

    let mat = |materials: &mut Assets<StandardMaterial>, tex: Option<Handle<Image>>, tint: Color, rough: f32, repeat: f32| {
        materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: tex,
            perceptual_roughness: rough,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(repeat)),
            ..default()
        })
    };

    // the table
    let oak = mat(&mut materials, tiled(&assets, &art, "book/table"), Color::WHITE, 0.85, 3.0);
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(9.0, 6.0))), MeshMaterial3d(oak), Transform::default()));

    // the block of pages and the back cover under it
    let edges = mat(&mut materials, tiled(&assets, &art, "book/edges"), Color::WHITE, 0.9, 1.0);
    let leather = mat(&mut materials, tiled(&assets, &art, "book/leather"), Color::WHITE, 0.7, 2.0);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0 * PAGE_W + 0.12, 0.025, PAGE_H + 0.08))),
        MeshMaterial3d(leather.clone()),
        Transform::from_xyz(0.0, 0.0125, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0 * PAGE_W + 0.02, TOP - 0.025, PAGE_H))),
        MeshMaterial3d(edges),
        Transform::from_xyz(0.0, 0.025 + (TOP - 0.025) / 2.0, 0.0),
    ));

    // the two pages of the spread, each wearing its own rendered picture; the right one lies on
    // the block, the left one is the first leaf and turns over with the cover
    let page_mat = |materials: &mut Assets<StandardMaterial>, tex: Handle<Image>| {
        materials.add(StandardMaterial { base_color_texture: Some(tex), perceptual_roughness: 0.95, ..default() })
    };
    let page = meshes.add(Plane3d::default().mesh().size(PAGE_W, PAGE_H));
    let left_mat = page_mat(&mut materials, left.clone());
    commands.spawn((Mesh3d(page.clone()), MeshMaterial3d(page_mat(&mut materials, right.clone())), Transform::from_xyz(PAGE_W / 2.0 + 0.005, TOP + 0.002, 0.0)));

    // the front cover hinges on the spine; closed it lies over the right page
    let cover_front = materials.add(StandardMaterial {
        base_color_texture: art.image(&assets, "book/cover"),
        perceptual_roughness: 0.6,
        ..default()
    });
    let endpaper = mat(&mut materials, tiled(&assets, &art, "book/endpaper"), Color::WHITE, 0.8, 1.0);
    let board = Vec3::new(PAGE_W + 0.06, 0.022, PAGE_H + 0.08);
    commands
        .spawn((Cover, Transform::from_xyz(0.0, TOP + 0.004, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(meshes.add(Cuboid::new(board.x, board.y, board.z))), MeshMaterial3d(leather), Transform::from_xyz(board.x / 2.0, board.y / 2.0, 0.0)));
            // the painted face on top, the endpaper beneath
            let face = meshes.add(Plane3d::default().mesh().size(board.x, board.z));
            c.spawn((Mesh3d(face.clone()), MeshMaterial3d(cover_front), Transform::from_xyz(board.x / 2.0, board.y + 0.001, 0.0)));
            c.spawn((
                Mesh3d(face),
                MeshMaterial3d(endpaper),
                Transform::from_xyz(board.x / 2.0, -0.001, 0.0).with_rotation(Quat::from_rotation_x(PI)),
            ));
            // the first leaf under the board: upside down while shut, upright once the cover lies
            // open (a half turn about x, then one about y, undoes the cover's half turn about z)
            c.spawn((
                Mesh3d(page),
                MeshMaterial3d(left_mat),
                Transform::from_xyz(PAGE_W / 2.0 + 0.005, -0.002, 0.0).with_rotation(Quat::from_rotation_x(PI) * Quat::from_rotation_y(PI)),
            ));
        });

    // the candle at the top right, and its light
    let wax = materials.add(StandardMaterial { base_color: Color::srgb(0.86, 0.78, 0.6), perceptual_roughness: 0.5, ..default() });
    let candle_at = Vec3::new(1.3, 0.0, -0.95);
    // its own light sits right over it: a shadow from the wax would black out the table
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.09, 0.55))),
        MeshMaterial3d(wax),
        Transform::from_translation(candle_at + Vec3::Y * 0.275),
        bevy::light::NotShadowCaster,
    ));
    let flame_mat = materials.add(StandardMaterial {
        base_color_texture: art.image(&assets, "book/flame"),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((Flame, Mesh3d(meshes.add(Rectangle::new(0.16, 0.16))), MeshMaterial3d(flame_mat), Transform::from_translation(candle_at + Vec3::Y * 0.62)));
    commands.spawn((
        Candle { base: 90_000.0 },
        PointLight { color: Color::srgb(1.0, 0.72, 0.42), intensity: 90_000.0, range: 12.0, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(candle_at + Vec3::Y * 0.66),
    ));

    // the pages' painters: one camera and one UI tree per page
    for (side, tex) in [(false, left.clone()), (true, right.clone())] {
        let cam = commands
            .spawn((PageCamera, Camera2d, Camera { order: -20, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.93, 0.88, 0.76)), ..default() }, RenderTarget::Image(tex.into())))
            .id();
        commands.spawn((
            PageCanvas(side),
            Node { width: px(PAGE_PX.x as f32), height: px(PAGE_PX.y as f32), ..default() },
            UiTargetCamera(cam),
        ));
    }

    commands.insert_resource(Book { view, left, right, wanted: false, spread: Spread::default(), drawn: None, open: 0.0, heard: false });
}

/// Wakes the scene while a tale is read, and swings the cover open on its spine.
fn open_cover(
    mut commands: Commands,
    time: Res<Time>,
    mut book: ResMut<Book>,
    mut cams: Query<&mut Camera, Or<(With<BookCamera>, With<PageCamera>)>>,
    mut cover: Query<&mut Transform, With<Cover>>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mixer: Res<Mixer>,
) {
    for mut cam in &mut cams {
        if cam.is_active != book.wanted {
            cam.is_active = book.wanted;
        }
    }
    if !book.wanted {
        book.open = 0.0;
        book.heard = false;
        return;
    }
    if !book.heard {
        book.heard = true;
        for (name, delay) in [("book_clasp", 0.0), ("book_open", 0.35)] {
            if let Some(path) = art.find(&format!("sfx/{name}"), "wav", "flow") {
                let _ = delay;
                commands.spawn((AudioPlayer::new(assets.load::<AudioSource>(path)), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.sfx))));
            }
        }
    }
    // a heavy board: slow to lift, quicker as it falls over
    book.open = (book.open + time.delta_secs() / 1.4).min(1.0);
    let t = book.open;
    let eased = t * t * (3.0 - 2.0 * t);
    for mut tf in &mut cover {
        tf.rotation = Quat::from_rotation_z(eased * PI * 0.985);
    }
}

/// The candle's light breathes, and its flame turns to face the camera.
fn flicker(time: Res<Time>, mut lights: Query<(&Candle, &mut PointLight)>, mut flames: Query<&mut Transform, With<Flame>>, cam: Query<&Transform, (With<BookCamera>, Without<Flame>)>) {
    let t = time.elapsed_secs();
    let wobble = (t * 7.3).sin() * 0.06 + (t * 13.1).sin() * 0.04 + (t * 2.1).sin() * 0.05;
    for (candle, mut light) in &mut lights {
        light.intensity = candle.base * (1.0 + wobble);
    }
    let Ok(cam) = cam.single() else { return };
    for mut tf in &mut flames {
        let to = cam.translation - tf.translation;
        tf.rotation = Quat::from_rotation_y(to.x.atan2(to.z)) * Quat::from_rotation_z(wobble * 0.6);
        tf.scale = Vec3::new(1.0, 1.0 + wobble * 1.5, 1.0);
    }
}

/// The camera leans a little after the cursor, as if the reader bent over the book.
fn sway_camera(window: Single<&Window>, scale: Res<UiScale>, mut cam: Query<&mut Transform, With<BookCamera>>, time: Res<Time>) {
    let Some(c) = crate::stage_cursor(&window, scale.0) else { return };
    let off = (c / Vec2::new(crate::STAGE_W, crate::STAGE_H) - 0.5) * Vec2::new(0.22, 0.12);
    let want = EYE + Vec3::new(off.x, 0.0, off.y);
    for mut tf in &mut cam {
        tf.translation = tf.translation.lerp(want, (time.delta_secs() * 3.0).min(1.0));
        *tf = tf.looking_at(LOOK, Vec3::Y);
    }
}

/// Prints the spread onto its two pages whenever the story turns to another.
fn redraw_pages(mut commands: Commands, mut book: ResMut<Book>, canvases: Query<(Entity, &PageCanvas)>, fonts: Res<Fonts>, art: Res<Art>, assets: Res<AssetServer>) {
    if book.drawn.as_ref() == Some(&book.spread) {
        return;
    }
    let spread = book.spread.clone();
    let parchment = art.image(&assets, "book/parchment");
    // ornaments with the paper cut out from under them (tools/inkcut.py), printed onto the page
    let cut = |stem: &str| art.find(stem, "png", "cut").map(|p| assets.load::<Image>(p));
    let ink = Color::srgb(0.17, 0.11, 0.07);
    let cinnabar = Color::srgb(0.62, 0.13, 0.08);
    for (entity, canvas) in &canvases {
        commands.entity(entity).despawn_children();
        let mut page = commands.entity(entity);
        if let Some(p) = &parchment {
            page.insert(ImageNode::new(p.clone()).with_mode(NodeImageMode::Stretch));
        }
        let number = if canvas.0 { spread.numbers.1 } else { spread.numbers.0 };
        if !canvas.0 {
            // the painting in its knotwork frame, the chapter's name beneath; the frame is sliced
            // so its corners keep their knots and its sides repeat along a picture of any shape
            const FRAME_W: f32 = 900.0;
            const BAND: f32 = 84.0;
            let inner_w = FRAME_W - 2.0 * BAND;
            let frame_h = inner_w / art.aspect(&spread.picture).unwrap_or(16.0 / 9.0) + 2.0 * BAND;
            let frame = cut("book/frame").map(|h| {
                ImageNode::new(h).with_mode(NodeImageMode::Sliced(TextureSlicer {
                    border: BorderRect::all(FRAME_SLICE),
                    center_scale_mode: SliceScaleMode::Stretch,
                    sides_scale_mode: SliceScaleMode::Tile { stretch_value: BAND / FRAME_SLICE },
                    max_corner_scale: BAND / FRAME_SLICE,
                }))
            });
            let picture = match art.image(&assets, &spread.picture) {
                Some(h) => ImageNode::new(h).with_mode(NodeImageMode::Stretch),
                None => ImageNode::solid_color(Color::BLACK),
            };
            let left = (PAGE_PX.x as f32 - FRAME_W) / 2.0;
            page.with_children(|p| {
                // the picture runs a little under the frame's inner line, so no paper shows
                // between them; the frame is printed after it, on top
                let tuck = BAND - 6.0;
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(left + tuck), top: px(150.0 + tuck), width: px(FRAME_W - 2.0 * tuck), height: px(frame_h - 2.0 * tuck), ..default() },
                    picture,
                ));
                if let Some(frame) = frame {
                    p.spawn((Node { position_type: PositionType::Absolute, left: px(left), top: px(150), width: px(FRAME_W), height: px(frame_h), ..default() }, frame));
                }
                if let Some(title) = &spread.title {
                    p.spawn((
                        Node { position_type: PositionType::Absolute, left: px(0), top: px(150.0 + frame_h + 48.0), width: px(PAGE_PX.x as f32), justify_content: JustifyContent::Center, ..default() },
                        children![crate::theme::text(title.clone(), &fonts.title, 64.0, cinnabar)],
                    ));
                }
            });
        } else {
            // the headpiece, a red initial, and the text in ink
            let mut chars = spread.text.chars();
            let initial: String = chars.next().map(|c| c.to_string()).unwrap_or_default();
            let rest: String = chars.collect();
            page.with_children(|p| {
                if let Some(h) = cut("book/headpiece") {
                    // only the band of the square sheet the ornament is painted on
                    let band = Rect::new(0.0, 184.0, 1024.0, 758.0);
                    let w = 680.0;
                    p.spawn((
                        Node { position_type: PositionType::Absolute, left: px((PAGE_PX.x as f32 - w) / 2.0), top: px(70), width: px(w), height: px(w * band.height() / band.width()), ..default() },
                        ImageNode { rect: Some(band), ..ImageNode::new(h).with_mode(NodeImageMode::Stretch) },
                    ));
                }
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(110), top: px(470), width: px(804), column_gap: px(14), ..default() },
                    children![
                        crate::theme::text(initial, &fonts.title, 150.0, cinnabar),
                        (Node { flex_grow: 1.0, padding: UiRect::top(px(10)), ..default() }, children![crate::theme::text(rest, &fonts.body, 44.0, ink)]),
                    ],
                ));
            });
        }
        if number > 0 {
            commands.entity(entity).with_children(|p| {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0), bottom: px(56), width: px(1024.0), justify_content: JustifyContent::Center, ..default() },
                    children![crate::theme::text(number.to_string(), &fonts.num, 40.0, ink.with_alpha(0.8))],
                ));
            });
        }
    }
    book.drawn = Some(spread);
    let _ = (&book.left, &book.right);
}
