//! The book of the tale: a leather chronicle on a candlelit oak table, drawn in 3D.
//!
//! The scene is rendered into a picture (`Book::view`) that the story page shows in place of its
//! flat painting, so it sits in the scaled stage like any other UI. Every printed face of paper is
//! a picture too: a little UI tree (parchment, framed painting, printed text) rendered by its own
//! camera into a texture, which the meshes wear.
//!
//! Pages turn as real paper does: the leaf being turned is a cloth of particles (`leaf`) held at
//! the spine. The reader takes hold of the right-hand page anywhere and carries it over; let go
//! past the spine, it falls onto the left and the story reads on, let go too early, it falls back.
//! When the story turns the page itself (a button, a key), an unseen hand turns the leaf.

use crate::art::Art;
use crate::audio::Mixer;
use crate::theme::Fonts;
use bevy::asset::RenderAssetUsages;
use bevy::audio::Volume;
use bevy::camera::RenderTarget;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::pointer::{PointerId, PointerLocation, PointerPressState};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use bevy::ui::UiTargetCamera;
use leaf::{Leaf, NX, NZ};
use std::f32::consts::PI;

mod leaf;

/// Size of the rendered scene, in pixels: the stage itself.
const VIEW: UVec2 = UVec2::new(1920, 1080);
/// Size of one page's texture; pages are 1 x PAGE_H in the scene.
const PAGE_PX: UVec2 = UVec2::new(1024, 1400);
const PAGE_W: f32 = 1.0;
const PAGE_H: f32 = 1.37;
/// Gap between the spine and the inner edge of a page.
const GUTTER: f32 = 0.005;
/// Height of the top of the page block above the table.
const TOP: f32 = 0.09;
/// Where a leaf lies: on the right-hand page, and on the left-hand one.
const LEAF_FLOOR: (f32, f32) = (TOP + 0.004, TOP + 0.009);
/// Distance from the edge of the frame texture to its inner window, in texture pixels.
const FRAME_SLICE: f32 = 380.0;
/// Where the reader's eye rests, and the point of the spread it looks at.
const EYE: Vec3 = Vec3::new(0.0, 1.66, 1.02);
const LOOK: Vec3 = Vec3::new(0.0, 0.0, 0.16);
/// How long the unseen hand takes to carry a leaf over, in seconds.
const AUTO_TURN: f32 = 0.9;
/// Stage pixels at the foot of the screen that belong to the story's buttons, not the book.
const BUTTON_BAND: f32 = 130.0;

pub fn plugin(app: &mut App) {
    app.add_message::<Turned>()
        .add_systems(Startup, setup)
        .add_systems(Update, ((open_cover, follow_story, hold_leaf, move_leaf, redraw_pages).chain(), flicker, sway_camera, fit_pages));
}

/// Sent when the reader has turned the page by hand: the story reads on.
#[derive(Message)]
pub struct Turned;

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

/// One side of a spread.
#[derive(Clone, Copy, PartialEq)]
enum Half {
    Left,
    Right,
}

/// The faces of paper in the scene, each wearing its own picture.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    /// The left-hand page, glued under the cover.
    Left,
    /// The right-hand page, on the block.
    Right,
    /// The leaf being turned: the side that faces up while it lies on the right...
    LeafFront,
    /// ...and the side that faces up once it lies on the left.
    LeafBack,
}

const FACES: [Face; 4] = [Face::Left, Face::Right, Face::LeafFront, Face::LeafBack];

/// Who is turning the leaf.
enum Hand {
    /// The reader, with the mouse.
    Reader,
    /// The story, by its button: carried over in an arc, `t` from 0 to 1.
    Unseen { t: f32, point: usize, z: f32 },
    /// Nobody: the leaf falls where it will.
    Gone,
}

struct Turning {
    /// The spread the leaf belongs to, and the one under it.
    from: Spread,
    to: Spread,
    /// The story has read on: the leaf must end up on the left.
    committed: bool,
    hand: Hand,
}

#[derive(Resource)]
pub struct Book {
    /// The scene as a picture, for the story page to show.
    pub view: Handle<Image>,
    /// Is the book wanted on the table: the story sets this while a tale is being read.
    pub wanted: bool,
    /// What the story wants on the spread.
    pub spread: Spread,
    /// The spread after this one, when there is one: what turning the leaf uncovers.
    pub next: Option<Spread>,
    /// May the reader turn the leaf by hand (not while a party reads a page together).
    pub drag_ok: bool,
    /// The spread the pages beneath show.
    shown: Option<Spread>,
    /// What each face is to wear, and what it wears now (indexed like `FACES`).
    faces: [Option<(Spread, Half)>; 4],
    drawn: [Option<(Spread, Half)>; 4],
    turn: Option<Turning>,
    leaf: Leaf,
    leaf_meshes: (Handle<Mesh>, Handle<Mesh>),
    /// 0 closed .. 1 open.
    open: f32,
    /// The cover's sounds have been played for this opening.
    heard: bool,
}

impl Book {
    fn wear(&mut self, face: Face, spread: &Spread, half: Half) {
        self.faces[face as usize] = Some((spread.clone(), half));
    }
}

#[derive(Component)]
struct BookCamera;
#[derive(Component)]
struct PageCamera;
#[derive(Component)]
struct Cover;
#[derive(Component)]
struct LeafMesh;
#[derive(Component)]
struct Candle {
    base: f32,
}
#[derive(Component)]
struct Flame;
/// The UI tree drawn onto one face of paper.
#[derive(Component)]
struct PageCanvas(Face);

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

/// The leaf as a mesh: its front faces up while it lies on the right, its back once on the left.
/// The back's pictures run the other way across, so the spine stays at the spine.
fn leaf_mesh(leaf: &Leaf, back: bool) -> Mesh {
    let mut uvs = Vec::with_capacity(NX * NZ);
    for j in 0..NZ {
        for i in 0..NX {
            let u = i as f32 / (NX - 1) as f32;
            uvs.push([if back { 1.0 - u } else { u }, j as f32 / (NZ - 1) as f32]);
        }
    }
    let mut indices = Vec::with_capacity((NX - 1) * (NZ - 1) * 6);
    for j in 0..NZ - 1 {
        for i in 0..NX - 1 {
            let (a, b, c, d) = (leaf::at(i, j), leaf::at(i, j + 1), leaf::at(i + 1, j), leaf::at(i + 1, j + 1));
            let quad = [a, b, c, c, b, d].map(|p| p as u32);
            if back {
                indices.extend(quad.iter().rev());
            } else {
                indices.extend(quad);
            }
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    shape_leaf(&mut mesh, leaf, back);
    mesh
}

fn shape_leaf(mesh: &mut Mesh, leaf: &Leaf, back: bool) {
    let positions: Vec<[f32; 3]> = leaf.points.iter().map(|p| p.to_array()).collect();
    let normals: Vec<[f32; 3]> = leaf.normals().iter().map(|n| if back { (-*n).to_array() } else { n.to_array() }).collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
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
    let textures = FACES.map(|_| target(&mut images, PAGE_PX));

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

    // the faces of paper, each wearing its own rendered picture; the right-hand page lies on the
    // block, the left-hand one is the first leaf and turns over with the cover
    let page_mat = |materials: &mut Assets<StandardMaterial>, tex: &Handle<Image>| {
        materials.add(StandardMaterial { base_color_texture: Some(tex.clone()), perceptual_roughness: 0.95, ..default() })
    };
    let page = meshes.add(Plane3d::default().mesh().size(PAGE_W, PAGE_H));
    let left_mat = page_mat(&mut materials, &textures[Face::Left as usize]);
    commands.spawn((
        Mesh3d(page.clone()),
        MeshMaterial3d(page_mat(&mut materials, &textures[Face::Right as usize])),
        Transform::from_xyz(PAGE_W / 2.0 + GUTTER, TOP + 0.002, 0.0),
    ));

    // the leaf being turned, hidden until a page turns
    let leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
    let front = meshes.add(leaf_mesh(&leaf, false));
    let back = meshes.add(leaf_mesh(&leaf, true));
    for (mesh, face) in [(front.clone(), Face::LeafFront), (back.clone(), Face::LeafBack)] {
        commands.spawn((LeafMesh, Mesh3d(mesh), MeshMaterial3d(page_mat(&mut materials, &textures[face as usize])), Transform::default(), Visibility::Hidden));
    }

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
                Transform::from_xyz(PAGE_W / 2.0 + GUTTER, -0.002, 0.0).with_rotation(Quat::from_rotation_x(PI) * Quat::from_rotation_y(PI)),
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

    // the painters: one camera and one UI tree per face of paper
    for (face, tex) in FACES.into_iter().zip(textures) {
        let cam = commands
            .spawn((PageCamera, Camera2d, Camera { order: -20, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.93, 0.88, 0.76)), ..default() }, RenderTarget::Image(tex.into())))
            .id();
        commands.spawn((PageCanvas(face), Node { width: px(PAGE_PX.x as f32), height: px(PAGE_PX.y as f32), ..default() }, UiTargetCamera(cam)));
    }

    commands.insert_resource(Book {
        view,
        wanted: false,
        spread: Spread::default(),
        next: None,
        drag_ok: false,
        shown: None,
        faces: Default::default(),
        drawn: Default::default(),
        turn: None,
        leaf,
        leaf_meshes: (front, back),
        open: 0.0,
        heard: false,
    });
}

fn play(commands: &mut Commands, art: &Art, assets: &AssetServer, mixer: &Mixer, name: &str) {
    if let Some(path) = art.find(&format!("sfx/{name}"), "wav", "flow") {
        commands.spawn((AudioPlayer::new(assets.load::<AudioSource>(path)), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.sfx))));
    }
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
        play(&mut commands, &art, &assets, &mixer, "book_clasp");
        play(&mut commands, &art, &assets, &mixer, "book_open");
    }
    // a heavy board: slow to lift, quicker as it falls over, and it lies flat open
    book.open = (book.open + time.delta_secs() / 1.4).min(1.0);
    let t = book.open;
    let eased = t * t * (3.0 - 2.0 * t);
    for mut tf in &mut cover {
        tf.rotation = Quat::from_rotation_z(eased * PI);
    }
}

/// Keeps the paper in step with the story: a new spread turns the leaf over to it.
fn follow_story(mut commands: Commands, mut book: ResMut<Book>, art: Res<Art>, assets: Res<AssetServer>, mixer: Res<Mixer>) {
    if !book.wanted {
        book.shown = None;
        book.turn = None;
        return;
    }
    let spread = book.spread.clone();
    if book.shown.as_ref() == Some(&spread) {
        return;
    }
    // the reader has carried the leaf over, and the story has read on to what it uncovered
    if let Some(turn) = book.turn.as_mut().filter(|t| t.to == spread) {
        turn.committed = true;
        book.shown = Some(spread);
        return;
    }
    match book.shown.take() {
        // the book has only just opened, or a turn is under way: no time for the leaf, the pages change
        Some(from) if book.open >= 1.0 && book.turn.is_none() => {
            book.leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
            let point = leaf::at(NX - 1, NZ - 3);
            let z = book.leaf.points[point].z;
            book.wear(Face::LeafFront, &from, Half::Right);
            book.wear(Face::LeafBack, &spread, Half::Left);
            book.wear(Face::Right, &spread, Half::Right);
            book.turn = Some(Turning { from, to: spread.clone(), committed: true, hand: Hand::Unseen { t: 0.0, point, z } });
            play(&mut commands, &art, &assets, &mixer, "book_flip");
        }
        _ => {
            book.turn = None;
            book.wear(Face::Left, &spread, Half::Left);
            book.wear(Face::Right, &spread, Half::Right);
        }
    }
    book.shown = Some(spread);
}

/// Where the cursor's ray meets the level `y` in the scene.
fn cursor_on(camera: &Camera, at: &GlobalTransform, cursor: Vec2, y: f32) -> Option<Vec3> {
    let ray = camera.viewport_to_world(at, cursor).ok()?;
    let t = (y - ray.origin.y) / ray.direction.y;
    (t > 0.0).then(|| ray.origin + ray.direction * t)
}

/// The reader takes hold of the right-hand page, carries it, and lets go. The mouse is read as
/// picking sees it, so the dev robot's mouse turns pages too.
fn hold_leaf(
    mut commands: Commands,
    mut book: ResMut<Book>,
    pointers: Query<(&PointerId, &PointerLocation, &PointerPressState)>,
    mut was_down: Local<bool>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<BookCamera>>,
    mut turned: MessageWriter<Turned>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mixer: Res<Mixer>,
) {
    let Ok((camera, at)) = camera.single() else { return };
    let mouse = pointers.iter().find(|(id, ..)| id.is_mouse());
    let cursor = mouse.and_then(|(_, loc, _)| loc.location()).map(|l| crate::stage_point(&window, scale.0, l.position));
    let down = mouse.is_some_and(|(.., press)| press.is_primary_pressed());
    let pressed = down && !*was_down;
    *was_down = down;
    let book = &mut *book;
    // take hold: only of a page lying still, with a page after it to uncover
    if pressed && book.turn.is_none() && book.drag_ok && book.open >= 1.0 {
        let (Some(c), Some(next), Some(shown)) = (cursor, book.next.clone(), book.shown.clone()) else { return };
        if c.y > crate::STAGE_H - BUTTON_BAND {
            return;
        }
        let Some(hit) = cursor_on(camera, at, c, LEAF_FLOOR.0) else { return };
        if !(GUTTER..GUTTER + PAGE_W).contains(&hit.x) || hit.z.abs() > PAGE_H / 2.0 {
            return;
        }
        book.leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
        let point = book.leaf.nearest(hit);
        book.leaf.grab = Some((point, book.leaf.points[point]));
        book.wear(Face::LeafFront, &shown, Half::Right);
        book.wear(Face::LeafBack, &next, Half::Left);
        book.wear(Face::Right, &next, Half::Right);
        book.turn = Some(Turning { from: shown, to: next, committed: false, hand: Hand::Reader });
        play(&mut commands, &art, &assets, &mixer, "book_rustle");
        return;
    }
    let Some(turn) = book.turn.as_mut() else { return };
    if !matches!(turn.hand, Hand::Reader) {
        return;
    }
    if !down {
        // let go: past the spine the leaf is turned, and the story reads on
        turn.hand = Hand::Gone;
        book.leaf.grab = None;
        if book.leaf.side() < 0.15 {
            turn.committed = true;
            turned.write(Turned);
            play(&mut commands, &art, &assets, &mixer, "book_flip");
        }
        return;
    }
    // carry: the point held keeps its distance from the spine, so it rides a dome over the book
    let (Some(c), Some((point, _))) = (cursor, book.leaf.grab) else { return };
    let Some(hit) = cursor_on(camera, at, c, LEAF_FLOOR.0) else { return };
    let reach = GUTTER + PAGE_W * (point % NX) as f32 / (NX - 1) as f32;
    let x = hit.x.clamp(-reach, reach);
    let rest_z = book.leaf.height() * ((point / NX) as f32 / (NZ - 1) as f32 - 0.5);
    let y = LEAF_FLOOR.0 + (reach * reach - x * x).max(0.0).sqrt() * 0.75;
    let z = rest_z + (hit.z - rest_z) * 0.35;
    book.leaf.grab = Some((point, Vec3::new(x, y, z)));
}

/// Runs the leaf's paper, moves the unseen hand, and lays the leaf down when it comes to rest.
fn move_leaf(
    time: Res<Time>,
    mut book: ResMut<Book>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shown: Query<&mut Visibility, With<LeafMesh>>,
) {
    let book = &mut *book;
    let turning = book.turn.is_some();
    for mut v in &mut shown {
        let want = if turning { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    let Some(turn) = book.turn.as_mut() else { return };
    let dt = time.delta_secs();
    if let Hand::Unseen { t, point, z } = &mut turn.hand {
        *t = (*t + dt / AUTO_TURN).min(1.0);
        // lifted at the corner, carried over in an arc, a little ahead of the paper
        let a = PI * (*t * *t * (3.0 - 2.0 * *t));
        let reach = GUTTER + PAGE_W * (*point % NX) as f32 / (NX - 1) as f32;
        book.leaf.grab = Some((*point, Vec3::new(reach * a.cos(), LEAF_FLOOR.0 + reach * a.sin() * 0.6, *z)));
        if *t >= 1.0 {
            turn.hand = Hand::Gone;
            book.leaf.grab = None;
        }
    }
    book.leaf.step(dt);
    for (handle, back) in [(&book.leaf_meshes.0, false), (&book.leaf_meshes.1, true)] {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            shape_leaf(&mut mesh, &book.leaf, back);
        }
    }
    if !matches!(turn.hand, Hand::Gone) || !book.leaf.still() {
        return;
    }
    // at rest: on the left the leaf becomes the left-hand page, on the right it was never turned
    let (from, to, committed) = (turn.from.clone(), turn.to.clone(), turn.committed);
    book.turn = None;
    if committed && book.leaf.side() < 0.0 {
        book.wear(Face::Left, &to, Half::Left);
    } else {
        book.wear(Face::Right, &from, Half::Right);
    }
}

/// Prints whatever each face of paper is to wear, when that changes.
fn redraw_pages(mut commands: Commands, mut book: ResMut<Book>, canvases: Query<(Entity, &PageCanvas)>, fonts: Res<Fonts>, art: Res<Art>, assets: Res<AssetServer>) {
    for (entity, canvas) in &canvases {
        let f = canvas.0 as usize;
        if book.faces[f] == book.drawn[f] {
            continue;
        }
        let Some((spread, half)) = book.faces[f].clone() else { continue };
        print_page(&mut commands, entity, &spread, half, &fonts, &art, &assets);
        book.drawn[f] = Some((spread, half));
    }
}

fn print_page(commands: &mut Commands, entity: Entity, spread: &Spread, half: Half, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    let parchment = art.image(assets, "book/parchment");
    // ornaments with the paper cut out from under them (tools/inkcut.py), printed onto the page
    let cut = |stem: &str| art.find(stem, "png", "cut").map(|p| assets.load::<Image>(p));
    let ink = Color::srgb(0.17, 0.11, 0.07);
    let cinnabar = Color::srgb(0.62, 0.13, 0.08);
    commands.entity(entity).despawn_children();
    let mut page = commands.entity(entity);
    if let Some(p) = &parchment {
        page.insert(ImageNode::new(p.clone()).with_mode(NodeImageMode::Stretch));
    }
    let number = match half {
        Half::Left => spread.numbers.0,
        Half::Right => spread.numbers.1,
    };
    match half {
        Half::Left => {
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
            let picture = match art.image(assets, &spread.picture) {
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
        }
        Half::Right => {
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
    }
    if number > 0 {
        commands.entity(entity).with_children(|p| {
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(0), bottom: px(56), width: px(PAGE_PX.x as f32), justify_content: JustifyContent::Center, ..default() },
                children![crate::theme::text(number.to_string(), &fonts.num, 40.0, ink.with_alpha(0.8))],
            ));
        });
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
