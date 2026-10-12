//! Battle screen (design doc, section 9): heroes left, enemies right, hand at
//! the bottom, party panel top-left, Ready seal bottom-right. Reads `Session`,
//! writes only to `Outbox`.

use crate::art::Art;
use crate::l10n::L10n;
use crate::session::*;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;
use nav_core::Event;

pub const CARD_W: f32 = 220.0;
pub const CARD_H: f32 = 310.0;
const HAND_TOP: f32 = 762.0;
const HERO_SIZE: Vec2 = Vec2::new(255.0, 340.0);
const ENEMY_SIZE: Vec2 = Vec2::new(230.0, 345.0);
/// Room above a unit's sprite for its intent.
const HEAD_ROOM: f32 = 62.0;
const UNIT_W: f32 = 280.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(AnimSpeed(1.0))
        .init_resource::<Outbox>()
        .init_resource::<RunOutbox>()
        .init_resource::<Cues>()
        .init_resource::<HoverTop>()
        .init_resource::<Shake>()
        .init_resource::<TimerText>()
        .insert_resource(Comfort { less_shake: false, ink: false })
        .init_resource::<TooltipText>()
        .add_systems(OnEnter(AppState::Battle), enter)
        .add_systems(OnExit(AppState::Battle), exit)
        .add_systems(
            Update,
            (
                playback,
                (sync_units, sync_hand, sync_potions, cue_effects).chain(),
                (pick_hover, layout_hand, layout_units, swap_form_art, animate_sprites, update_labels, update_bars, update_tooltip),
                (floaters, fly_ghosts, dying, style_buttons, shake_screen, turn_timer, send_pings, show_pings),
            )
                .chain()
                .run_if(in_state(AppState::Battle))
                .run_if(resource_exists::<Session>),
        )
        .add_observer(on_drag_start)
        .add_observer(on_drag)
        .add_observer(on_drag_end)
        .add_observer(on_drop);
}

// ---- components ----

#[derive(Component)]
struct BattleRoot;
#[derive(Component)]
struct UnitLayer;
#[derive(Component)]
struct HandLayer;
#[derive(Component)]
struct PotionBelt;
/// A potion on the belt: click to drink, drag onto a unit to throw.
#[derive(Component)]
struct PotionChip {
    slot: u8,
    id: String,
}
#[derive(Component)]
struct FloatLayer;
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct Dying(f32);
/// Marks a styled button; the colour is its resting background.
#[derive(Component)]
pub struct Btn(pub Color);

#[derive(Component)]
struct UnitWidget {
    unit: UnitRef,
    pos: Vec2,
}

#[derive(Component)]
struct UnitSprite {
    unit: UnitRef,
    /// Resting colour: white for real art, the hero colour for a missing sprite.
    base: Color,
    /// Art currently shown, so a change of form swaps the picture once.
    stem: String,
    t: f32,
    anim: Option<(Anim, f32)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Anim {
    Lunge,
    Hit,
    /// A skill, power or enemy ability: a short gathering gesture.
    Cast,
    /// Gained block: a cold flash.
    Guarded,
    /// Healed or raised: a warm swell.
    Healed,
    /// A buff took hold.
    Buffed,
    /// A curse or other debuff took hold: a dark shiver.
    Cursed,
}

impl Anim {
    /// A weightier animation is never cut short by a slighter one that starts the same instant.
    fn weight(self) -> u8 {
        match self {
            Anim::Hit => 5,
            Anim::Lunge => 4,
            Anim::Cast => 3,
            _ => 1,
        }
    }
}

/// A small card (or potion) flying from whoever used it to what it was used on,
/// so that every player sees every play, not only their own.
#[derive(Component)]
struct Ghost {
    from: Vec2,
    to: Vec2,
    t: f32,
}

#[derive(Component)]
struct HpFill(UnitRef);
#[derive(Component)]
struct BlockBadge(UnitRef);

/// Every piece of text that is refreshed from the session each frame.
#[derive(Component, Clone, Copy, PartialEq)]
enum Label {
    Hp(UnitRef),
    Block(UnitRef),
    Status(UnitRef),
    Intent(u32),
    Team(u8),
    Energy,
    Draw,
    Discard,
    Round,
    Ready,
    Error,
    Tooltip,
    Timer,
}

#[derive(Component)]
struct CardWidget {
    uid: u32,
    form: Form,
    /// Drawn face down: the hero is blinded this turn.
    hidden: bool,
    pos: Vec2,
    drag: Vec2,
    dragging: bool,
}

#[derive(Component)]
struct Floater {
    ttl: f32,
    life: f32,
}

/// Screen shake in progress: strength in stage pixels, fading out.
#[derive(Resource, Default)]
struct Shake {
    strength: f32,
    t: f32,
}

/// Comfort settings that never touch game state.
#[derive(Resource)]
pub struct Comfort {
    pub less_shake: bool,
    /// Blood shown as black ink.
    pub ink: bool,
}

impl Comfort {
    /// The colour of a wound: fresh blood, or ink for those who want less gore.
    pub fn blood(&self) -> Color {
        if self.ink { Color::srgb(0.03, 0.03, 0.05) } else { theme::FRESH_BLOOD }
    }
}

/// The one hovered card that is on top, since fanned cards overlap.
#[derive(Resource, Default)]
struct HoverTop(Option<u32>);

// ---- layout ----

fn hero_foot(slot: u8) -> Vec2 {
    // staggered front and back rows, far enough apart that every HP bar stays readable
    match slot {
        0 => Vec2::new(670.0, 700.0),
        1 => Vec2::new(480.0, 630.0),
        2 => Vec2::new(300.0, 700.0),
        _ => Vec2::new(120.0, 630.0),
    }
}

fn enemy_foot(i: usize, n: usize) -> Vec2 {
    let span = 780.0;
    let step = span / n.max(1) as f32;
    let x = 1060.0 + step * (i as f32 + 0.5);
    // a crowd staggers into two rows so sprites don't hide each other
    let y = if n > 3 && i % 2 == 1 { 610.0 } else { 700.0 };
    Vec2::new(x, y)
}

/// Feet position and sprite size of a unit on the stage.
fn unit_place(shown: &Combat, unit: UnitRef) -> Option<(Vec2, Vec2)> {
    match unit {
        UnitRef::Hero(i) => shown.hero(i).map(|_| (hero_foot(i), HERO_SIZE)),
        UnitRef::Enemy(uid) => {
            let i = shown.enemies.iter().position(|e| e.uid == uid)?;
            Some((enemy_foot(i, shown.enemies.len()), ENEMY_SIZE))
        }
    }
}

fn widget_origin(foot: Vec2, size: Vec2) -> Vec2 {
    Vec2::new(foot.x - UNIT_W / 2.0, foot.y - size.y - HEAD_ROOM)
}

fn hand_slot(i: usize, n: usize) -> Vec2 {
    let spacing = (1040.0 / n.max(1) as f32).min(CARD_W * 0.78);
    let total = spacing * (n.max(1) - 1) as f32;
    let x = STAGE_W / 2.0 - total / 2.0 + spacing * i as f32 - CARD_W / 2.0;
    // a shallow arc: the ends of the fan sit a little lower
    let mid = (n as f32 - 1.0) / 2.0;
    let off = (i as f32 - mid).abs();
    Vec2::new(x, HAND_TOP + off * off * 3.0)
}

// ---- screen setup ----

fn enter(
    mut commands: Commands,
    session: Option<Res<Session>>,
    net: Option<Res<crate::net::Net>>,
    profile: Res<crate::profile::Profile>,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    // the session is built from the run's state before this screen opens
    let Some(session) = session else {
        warn!("battle opened with nothing to fight");
        return;
    };
    let layer = || Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() };
    let act = net.as_ref().and_then(|n| n.node.state.as_ref()).map(|r| r.act.min(3)).unwrap_or(1);

    let root = commands
        .spawn((BattleRoot, layer(), UiTransform::IDENTITY, Pickable::IGNORE, ChildOf(*stage)))
        .with_children(|p| {
            // each act has its own backdrop; until it is painted, the forest stands in
            let backdrop = [format!("bg/act{act}_battle"), "bg/act1_battle".to_string()].into_iter().find(|s| art.image(&assets, s).is_some()).unwrap_or_default();
            p.spawn((layer(), art.node(&assets, &backdrop, theme::SOOT), Pickable::IGNORE));
            p.spawn((UnitLayer, layer(), Pickable::IGNORE));

            // party panel, top-left
            p.spawn((
                theme::panel(Node {
                    position_type: PositionType::Absolute,
                    left: px(16),
                    top: px(16),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(2)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                }),
                Pickable::IGNORE,
            ))
            .with_children(|team| {
                for h in &session.truth.heroes {
                    team.spawn((
                        Node { column_gap: px(10), align_items: AlignItems::Center, ..default() },
                        Pickable::IGNORE,
                        children![
                            (
                                Node { width: px(34), height: px(34), border: UiRect::all(px(2)), ..default() },
                                art.node(&assets, &format!("portraits/{}", h.def), theme::hero_color(&h.def)),
                                BorderColor::all(theme::hero_color(&h.def)),
                                Pickable::IGNORE,
                            ),
                            (Label::Team(h.slot), theme::text("", &fonts.body, 20.0, theme::BONE)),
                        ],
                    ));
                }
            });

            p.spawn((
                Label::Timer,
                theme::text("", &fonts.num, 30.0, theme::CANDLE),
                Node { position_type: PositionType::Absolute, right: px(24), top: px(56), ..default() },
            ));
            // the very first fight explains itself, briefly
            if profile.runs == 0 {
                p.spawn((
                    theme::panel(Node {
                        position_type: PositionType::Absolute,
                        left: px(560),
                        top: px(16),
                        width: px(800),
                        padding: UiRect::all(px(14)),
                        border: UiRect::all(px(2)),
                        ..default()
                    }),
                    Pickable::IGNORE,
                    children![theme::text(l10n.t("battle-tutorial"), &fonts.body, 20.0, theme::BONE)],
                ));
            }
            p.spawn((
                Label::Round,
                theme::text("", &fonts.title, 26.0, theme::BONE),
                Node { position_type: PositionType::Absolute, right: px(24), top: px(16), ..default() },
            ));

            // energy: burning splinters left of the hand
            p.spawn((
                theme::panel_edged(Node {
                    position_type: PositionType::Absolute,
                    left: px(300),
                    top: px(900),
                    width: px(120),
                    height: px(120),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::MAX,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                }, theme::CANDLE),
                Pickable::IGNORE,
                children![
                    (Label::Energy, theme::text("", &fonts.num, 46.0, theme::CANDLE)),
                    theme::text(l10n.t("battle-energy"), &fonts.body, 16.0, theme::DIM),
                ],
            ));

            let pile = |left: f32, label: Label| {
                (
                    theme::panel(Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(980),
                        width: px(200),
                        padding: UiRect::axes(px(12), px(8)),
                        border: UiRect::all(px(2)),
                        justify_content: JustifyContent::Center,
                        ..default()
                    }),
                    Pickable::IGNORE,
                    children![(label, theme::text("", &fonts.body, 20.0, theme::BONE))],
                )
            };
            p.spawn(pile(40.0, Label::Draw));
            p.spawn(pile(1680.0, Label::Discard));

            // the Ready seal
            p.spawn((
                Button,
                Hovered::default(),
                Btn(theme::RUST_BLOOD),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(1640),
                    top: px(860),
                    width: px(240),
                    height: px(96),
                    border: UiRect::all(px(3)),
                    border_radius: BorderRadius::all(px(48)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(theme::RUST_BLOOD),
                BorderColor::all(theme::CANDLE),
                children![(Label::Ready, theme::text("", &fonts.title, 26.0, theme::BONE), TextLayout::justify(Justify::Center))],
                observe(|_: On<Activate>, session: Res<Session>, mut outbox: ResMut<Outbox>| {
                    let h = session.my_hero();
                    if !h.downed && session.truth.phase == Phase::Heroes {
                        outbox.0.push(CombatAction::Ready(!h.ready));
                    }
                }),
            ));

            p.spawn((
                PotionBelt,
                Node { position_type: PositionType::Absolute, left: px(40), top: px(880), flex_direction: FlexDirection::Column, row_gap: px(6), ..default() },
                Pickable::IGNORE,
            ));
            p.spawn((HandLayer, layer(), Pickable::IGNORE));
            p.spawn((FloatLayer, layer(), Pickable::IGNORE, GlobalZIndex(50)));

            p.spawn((
                Label::Error,
                theme::text("", &fonts.body, 26.0, theme::FRESH_BLOOD),
                Node { position_type: PositionType::Absolute, top: px(700), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
                TextLayout::justify(Justify::Center),
                GlobalZIndex(60),
            ));

            p.spawn((
                Tooltip,
                theme::panel(Node {
                    position_type: PositionType::Absolute,
                    width: px(330),
                    padding: UiRect::all(px(12)),
                    border: UiRect::all(px(2)),
                    ..default()
                }),
                Visibility::Hidden,
                GlobalZIndex(70),
                Pickable::IGNORE,
                children![(Label::Tooltip, theme::text("", &fonts.body, 18.0, theme::BONE))],
            ));
        })
        .id();
    let _ = root;
}

fn exit(mut commands: Commands, roots: Query<Entity, With<BattleRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<Session>();
}

// ---- units ----

fn sync_units(
    mut commands: Commands,
    session: Res<Session>,
    layer: Single<Entity, With<UnitLayer>>,
    widgets: Query<(Entity, &UnitWidget, Has<Dying>)>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let shown = &session.shown;
    let have = |u: UnitRef| widgets.iter().find(|(_, w, _)| w.unit == u);
    let mut wanted: Vec<(UnitRef, String, String, Color)> = Vec::new();
    for h in &shown.heroes {
        if have(UnitRef::Hero(h.slot)).is_none() {
            wanted.push((UnitRef::Hero(h.slot), format!("heroes/{}", h.def), l10n.t(&format!("hero-{}", h.def)), theme::hero_color(&h.def)));
        }
    }
    for e in &shown.enemies {
        let unit = UnitRef::Enemy(e.uid);
        match have(unit) {
            None if e.alive => wanted.push((unit, format!("enemies/{}", e.def), l10n.t(&format!("enemy-{}", e.def)), theme::SWAMP)),
            Some((entity, _, false)) if !e.alive => {
                commands.entity(entity).insert(Dying(0.7));
            }
            _ => {}
        }
    }
    for (unit, stem, name, color) in wanted {
        let Some((foot, size)) = unit_place(shown, unit) else { continue };
        let origin = widget_origin(foot, size);
        let is_hero = matches!(unit, UnitRef::Hero(_));
        commands
            .spawn((
                UnitWidget { unit, pos: origin },
                Hovered::default(),
                Pickable::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(origin.x),
                    top: px(origin.y),
                    width: px(UNIT_W),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ZIndex(foot.y as i32),
                ChildOf(*layer),
            ))
            .with_children(|u| {
                u.spawn((
                    Node { height: px(HEAD_ROOM), align_items: AlignItems::Center, ..default() },
                    Pickable::IGNORE,
                    children![match unit {
                        UnitRef::Enemy(uid) => (Label::Intent(uid), theme::text("", &fonts.num, 26.0, theme::BONE)),
                        // heroes show nothing overhead; the slot keeps both sides aligned
                        UnitRef::Hero(_) => (Label::Intent(0), theme::text("", &fonts.num, 26.0, theme::BONE)),
                    }],
                ));
                let sprite = art.node(&assets, &stem, color);
                let base = sprite.0.color;
                u.spawn((
                    UnitSprite { unit, base, stem: stem.clone(), t: if is_hero { 0.0 } else { 1.3 }, anim: None },
                    Node { width: px(size.x), height: px(size.y), ..default() },
                    sprite,
                    UiTransform::IDENTITY,
                    Pickable::IGNORE,
                ));
                u.spawn((
                    Node {
                        width: px(180),
                        height: px(22),
                        margin: UiRect::top(px(8)),
                        border: UiRect::all(px(2)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(theme::SOOT),
                    BorderColor::all(Color::srgb(0.32, 0.26, 0.20)),
                    Pickable::IGNORE,
                    children![
                        (
                            HpFill(unit),
                            Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: percent(100), height: percent(100), ..default() },
                            BackgroundColor(theme::FRESH_BLOOD),
                            Pickable::IGNORE,
                        ),
                        (Label::Hp(unit), theme::text("", &fonts.num, 17.0, theme::BONE)),
                        (
                            BlockBadge(unit),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(-44),
                                top: px(-8),
                                width: px(40),
                                height: px(34),
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::all(px(6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(theme::NAV_BLUE),
                            BorderColor::all(theme::FROST),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                            children![(Label::Block(unit), theme::text("", &fonts.num, 20.0, theme::BONE))],
                        ),
                    ],
                ));
                u.spawn((theme::text(name, &fonts.body, 17.0, theme::BONE), Node { margin: UiRect::top(px(3)), ..default() }));
                u.spawn((
                    Label::Status(unit),
                    theme::text("", &fonts.body, 16.0, theme::ROTLIGHT),
                    TextLayout::justify(Justify::Center),
                    Node { max_width: px(UNIT_W), ..default() },
                ));
            });
    }
}

fn layout_units(session: Res<Session>, time: Res<Time>, mut widgets: Query<(&mut UnitWidget, &mut Node, &mut ZIndex)>) {
    let k = (time.delta_secs() * 10.0).min(1.0);
    for (mut w, mut node, mut z) in &mut widgets {
        let Some((foot, size)) = unit_place(&session.shown, w.unit) else { continue };
        let target = widget_origin(foot, size);
        if w.pos.distance(target) > 0.5 {
            w.pos = w.pos.lerp(target, k);
            node.left = px(w.pos.x);
            node.top = px(w.pos.y);
            z.0 = foot.y as i32;
        }
    }
}

/// The werewolf has a second body: when the form changes, so does the sprite.
fn swap_form_art(session: Res<Session>, art: Res<Art>, assets: Res<AssetServer>, mut sprites: Query<(&mut UnitSprite, &mut ImageNode)>) {
    for (mut s, mut image) in &mut sprites {
        let UnitRef::Hero(i) = s.unit else { continue };
        let Some(h) = session.shown.hero(i) else { continue };
        let want = if h.form == Form::Beast { format!("heroes/{}_beast", h.def) } else { format!("heroes/{}", h.def) };
        if s.stem == want {
            continue;
        }
        // no beast art for this hero: keep what is shown rather than blank the sprite
        if let Some(handle) = art.image(&assets, &want) {
            image.image = handle;
        }
        s.stem = want;
    }
}

fn animate_sprites(
    session: Res<Session>,
    time: Res<Time>,
    speed: Res<AnimSpeed>,
    comfort: Res<Comfort>,
    mut sprites: Query<(&mut UnitSprite, &mut UiTransform, &mut ImageNode)>,
) {
    let dt = time.delta_secs() * speed.0;
    for (mut s, mut tf, mut image) in &mut sprites {
        s.t += dt;
        // idle: slow breathing, feet planted
        let breath = (s.t * 2.5).sin();
        let mut scale = Vec2::new(1.0 - breath * 0.006, 1.0 + breath * 0.012);
        let mut offset = Vec2::new(0.0, -breath * 2.0);
        let toward = if matches!(s.unit, UnitRef::Hero(_)) { 1.0 } else { -1.0 };
        let mut tint = s.base;

        if let Some((anim, t)) = &mut s.anim {
            *t += dt;
            let dur = match anim {
                Anim::Lunge | Anim::Hit => 0.30,
                Anim::Cast => 0.45,
                Anim::Guarded | Anim::Cursed => 0.40,
                Anim::Healed | Anim::Buffed => 0.55,
            };
            let done = *t >= dur;
            let p = (*t / dur).clamp(0.0, 1.0);
            let swell = (p * std::f32::consts::PI).sin();
            match anim {
                Anim::Cast => {
                    // gather, then release toward the other side
                    offset.y -= 14.0 * swell;
                    offset.x += toward * 12.0 * swell;
                    scale *= 1.0 + 0.05 * swell;
                }
                Anim::Guarded => {
                    scale *= Vec2::new(1.0 + 0.04 * swell, 1.0 - 0.02 * swell);
                    tint = s.base.mix(&Color::srgb(0.45, 0.65, 1.0), 0.5 * swell);
                }
                Anim::Healed => {
                    offset.y -= 8.0 * swell;
                    scale *= 1.0 + 0.04 * swell;
                    tint = s.base.mix(&Color::srgb(0.65, 1.0, 0.55), 0.5 * swell);
                }
                Anim::Buffed => {
                    scale *= 1.0 + 0.07 * swell;
                    tint = s.base.mix(&Color::srgb(1.0, 0.8, 0.4), 0.5 * swell);
                }
                Anim::Cursed => {
                    offset.x += 6.0 * (p * 38.0).sin() * (1.0 - p);
                    tint = s.base.mix(&Color::srgb(0.25, 0.4, 0.15), 0.6 * swell);
                }
                Anim::Lunge => offset.x += toward * 70.0 * (p * std::f32::consts::PI).sin(),
                Anim::Hit => {
                    offset.x -= toward * 22.0 * (1.0 - p) * (p * 40.0).cos().abs();
                    scale.x *= 1.0 - 0.04 * (1.0 - p);
                    if p < 0.4 {
                        tint = s.base.mix(&if comfort.ink { Color::srgb(0.1, 0.1, 0.14) } else { Color::srgb(1.0, 0.2, 0.15) }, 0.55);
                    }
                }
            }
            if done {
                s.anim = None;
            }
        }
        if let UnitRef::Hero(i) = s.unit {
            if session.shown.hero(i).is_some_and(|h| h.downed) {
                // on the Edge: slumped and dark
                tint = s.base.mix(&Color::srgb(0.05, 0.05, 0.08), 0.72);
                offset.y += 46.0;
                scale *= Vec2::new(1.0, 0.86);
            }
        }
        tf.translation = Val2::px(offset.x, offset.y);
        tf.scale = scale;
        if image.color != tint {
            image.color = tint;
        }
    }
}

fn dying(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Dying, &Children)>, mut images: Query<&mut ImageNode>) {
    for (entity, mut d, children) in &mut q {
        d.0 -= time.delta_secs();
        for c in children.iter() {
            if let Ok(mut image) = images.get_mut(c) {
                image.color = Color::srgba(0.5, 0.55, 0.5, (d.0 / 0.7).clamp(0.0, 1.0));
            }
        }
        if d.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn update_bars(
    session: Res<Session>,
    mut fills: Query<(&HpFill, &mut Node)>,
    mut badges: Query<(&BlockBadge, &mut Visibility)>,
) {
    let stats = |u: UnitRef| match u {
        UnitRef::Hero(i) => session.shown.hero(i).map(|h| (h.hp, h.max_hp, h.block)),
        UnitRef::Enemy(i) => session.shown.enemy(i).map(|e| (e.hp, e.max_hp, e.block)),
    };
    for (fill, mut node) in &mut fills {
        if let Some((hp, max, _)) = stats(fill.0) {
            let w = percent(100.0 * hp.max(0) as f32 / max.max(1) as f32);
            if node.width != w {
                node.width = w;
            }
        }
    }
    for (badge, mut vis) in &mut badges {
        let want = if stats(badge.0).is_some_and(|s| s.2 > 0) { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

// ---- text ----

fn statuses_line(l10n: &L10n, map: &Statuses) -> String {
    map.iter().map(|(k, v)| format!("{} {}", l10n.t(&format!("status-{k:?}")), v)).collect::<Vec<_>>().join(" · ")
}

fn update_labels(session: Res<Session>, db: Res<Db>, l10n: Res<L10n>, tip: Res<TooltipText>, clock: Res<TimerText>, mut labels: Query<(&Label, &mut Text)>) {
    let (shown, truth) = (&session.shown, &session.truth);
    for (label, mut text) in &mut labels {
        let s = match *label {
            Label::Hp(UnitRef::Hero(i)) => shown.hero(i).map(|h| format!("{}/{}", h.hp, h.max_hp)),
            Label::Hp(UnitRef::Enemy(i)) => shown.enemy(i).map(|e| format!("{}/{}", e.hp, e.max_hp)),
            Label::Block(UnitRef::Hero(i)) => shown.hero(i).map(|h| h.block.to_string()),
            Label::Block(UnitRef::Enemy(i)) => shown.enemy(i).map(|e| e.block.to_string()),
            Label::Status(UnitRef::Hero(i)) => shown.hero(i).map(|h| {
                if h.downed {
                    l10n.t("battle-downed")
                } else {
                    // the hero's own resource first, then statuses
                    let mut parts = Vec::new();
                    let real = truth.hero(i).unwrap_or(h);
                    if h.mechanic == Mechanic::Moon {
                        parts.push(format!("{} · {} {}/5", l10n.t(&format!("form-{:?}", h.form)), l10n.t("res-moon"), real.moon));
                    }
                    if let Some(m) = h.mask {
                        parts.push(format!("{} {}", l10n.t("res-mask"), l10n.t(&format!("mask-{m:?}"))));
                    }
                    if !h.runes.is_empty() {
                        parts.push(format!("{}: {}", l10n.t("res-runes"), h.runes.iter().map(|r| l10n.t(&format!("rune-{r:?}"))).collect::<Vec<_>>().join(", ")));
                    }
                    if !real.cauldron.is_empty() {
                        parts.push(format!("{}: {}", l10n.t("res-cauldron"), real.cauldron.iter().map(|x| l10n.t(&format!("herb-{x}"))).collect::<Vec<_>>().join(", ")));
                    }
                    for flag in ["marked", "blind", "coiled", "confused", "mired", "asleep", "charmed", "frozen", "winter"] {
                        if real.flags.contains_key(flag) {
                            parts.push(l10n.t(&format!("trait-{flag}")));
                        }
                    }
                    let st = statuses_line(&l10n, &h.statuses);
                    if !st.is_empty() {
                        parts.push(st);
                    }
                    parts.join("\n")
                }
            }),
            Label::Status(UnitRef::Enemy(i)) => shown.enemy(i).map(|e| {
                let mut s = statuses_line(&l10n, &e.statuses);
                // the real state carries the traits and the countdown
                if let Some(real) = truth.enemy(i) {
                    let mut notes = Vec::new();
                    if real.flags.contains_key("immortal") {
                        notes.push(l10n.t("trait-immortal"));
                    }
                    if let Some(n) = real.flags.get("last_breath") {
                        notes.push(l10n.n("trait-last-breath", *n));
                    }
                    if !notes.is_empty() {
                        s = if s.is_empty() { notes.join(" · ") } else { format!("{}
{s}", notes.join(" · ")) };
                    }
                }
                s
            }),
            Label::Intent(uid) => shown.enemy(uid).map(|e| {
                let Some(intent) = &e.intent else { return String::new() };
                // the Sirin's song: nobody can tell what is coming
                if session.my_hero().flags.contains_key("no_intents") {
                    return "?".into();
                }
                let Some(mv) = db.0.enemies.get(&e.def).and_then(|d| d.moves.get(intent.mv)) else { return String::new() };
                let mut s = match bot::intent_damage(&db.0, shown, e) {
                    Some((d, 1)) => format!("{} {d}", l10n.t(&format!("intent-{:?}", mv.intent))),
                    Some((d, k)) => format!("{} {d}×{k}", l10n.t(&format!("intent-{:?}", mv.intent))),
                    None => l10n.t(&format!("intent-{:?}", mv.intent)),
                };
                // in a party the target matters; alone it is always you
                if let (Some(t), true) = (intent.target, shown.is_coop()) {
                    if let Some(h) = shown.hero(t) {
                        s.push_str(&format!(" → {}", l10n.t(&format!("hero-{}", h.def))));
                    }
                }
                s
            }),
            Label::Team(i) => shown.hero(i).zip(truth.hero(i)).map(|(h, real)| {
                let mut s = format!("{}  {}/{}", l10n.t(&format!("hero-{}", h.def)), h.hp, h.max_hp);
                if h.downed {
                    s.push_str(&format!("  ·  {}", l10n.t("battle-downed")));
                } else {
                    s.push_str(&format!("  ·  {} {}", l10n.t("battle-energy"), real.energy));
                    if real.ready {
                        s.push_str(&format!("  ·  {}", l10n.t("battle-ready")));
                    }
                }
                s
            }),
            Label::Energy => Some(format!("{}", session.my_hero().energy)),
            Label::Draw => Some(format!("{} {}", l10n.t("battle-draw"), session.my_hero().draw.len())),
            Label::Discard => {
                let h = session.my_hero();
                let mut s = format!("{} {}", l10n.t("battle-discard"), h.discard.len());
                if !h.exhaust.is_empty() {
                    s.push_str(&format!("  ·  {} {}", l10n.t("battle-exhaust"), h.exhaust.len()));
                }
                Some(s)
            }
            Label::Round => Some(l10n.n("battle-round", shown.round as i32)),
            Label::Ready => Some(if session.my_hero().ready {
                if truth.is_coop() {
                    l10n.t("battle-waiting")
                } else {
                    l10n.t("battle-unready")
                }
            } else {
                l10n.t("battle-ready")
            }),
            Label::Error => Some(session.error.as_ref().map(|e| e.0.clone()).unwrap_or_default()),
            Label::Tooltip => Some(tip.0.clone()),
            Label::Timer => Some(clock.0.clone()),
        };
        if let Some(s) = s {
            if text.0 != s {
                text.0 = s;
            }
        }
    }
}

// ---- hand ----

pub fn card_frame(def: &CardDef) -> Color {
    match (def.kind, def.rarity) {
        (CardType::Curse, _) => Color::srgb(0.16, 0.09, 0.17),
        (CardType::Status, _) => Color::srgb(0.17, 0.20, 0.18),
        (_, Rarity::Uncommon) => Color::srgb(0.30, 0.31, 0.34),
        (_, Rarity::Rare) => Color::srgb(0.46, 0.33, 0.12),
        _ => Color::srgb(0.24, 0.20, 0.15),
    }
}

/// Spawns a card face under `parent`: free-floating at `pos`, or in the parent's flow when `pos` is `None`.
/// `extra` carries whatever makes the card behave (drag in the hand, a button in a reward).
pub fn spawn_card<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    card: &Card,
    form: Form,
    pos: Option<Vec2>,
    hidden: bool,
    extra: B,
    db: &Content,
    l10n: &L10n,
    fonts: &Fonts,
    art: &Art,
    assets: &AssetServer,
) -> Option<Entity> {
    let def = db.cards.get(&card.def)?;
    let view = def.view(card.upgraded, form);
    let edge = if def.hero.is_empty() { theme::DIM } else { theme::hero_color(&def.hero) };
    let cost = if hidden || view.has(Keyword::Unplayable) {
        String::new()
    } else if view.x_cost {
        "X".into()
    } else {
        view.cost.to_string()
    };
    let id = commands
        .spawn((
            extra,
            Hovered::default(),
            Node {
                position_type: if pos.is_some() { PositionType::Absolute } else { PositionType::Relative },
                left: pos.map(|p| px(p.x)).unwrap_or(Val::Auto),
                top: pos.map(|p| px(p.y)).unwrap_or(Val::Auto),
                flex_shrink: 0.0,
                width: px(CARD_W),
                height: px(CARD_H),
                border: UiRect::all(px(4)),
                border_radius: BorderRadius::all(px(12)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(6)),
                ..default()
            },
            BackgroundColor(card_frame(def)),
            BorderColor::all(edge),
            UiTransform::IDENTITY,
            ZIndex(0),
            ChildOf(parent),
        ))
        .with_children(|c| {
            // a blinded hero sees only the backs: which card is which must be remembered
            if hidden {
                c.spawn((theme::text("?", &fonts.title, 120.0, theme::DIM), Node { margin: UiRect::top(px(70)), ..default() }));
                return;
            }
            let name = l10n.card_name(&card.def, card.upgraded);
            c.spawn((
                // long names shrink to stay on one line
                theme::text(name.clone(), &fonts.title, if name.chars().count() > 13 { 14.5 } else { 19.0 }, if card.upgraded { theme::ROTLIGHT } else { theme::BONE }),
                Node { height: px(28), ..default() },
            ));
            c.spawn((
                Node { width: px(196), height: px(147), border: UiRect::all(px(2)), ..default() },
                art.node(assets, &format!("cards/{}", card.def), Color::srgb(0.10, 0.09, 0.08)),
                BorderColor::all(theme::SOOT),
                Pickable::IGNORE,
            ));
            c.spawn((theme::text(l10n.t(&format!("type-{:?}", view.kind)), &fonts.body, 14.0, theme::DIM), Node { margin: UiRect::vertical(px(2)), ..default() }));
            c.spawn((
                theme::text(l10n.card_text(&card.def, def, &view), &fonts.body, 18.0, theme::BONE),
                TextLayout::justify(Justify::Center),
                Node { width: px(196), ..default() },
            ));
            if !cost.is_empty() {
                c.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-16),
                        top: px(-16),
                        width: px(48),
                        height: px(48),
                        border: UiRect::all(px(3)),
                        border_radius: BorderRadius::MAX,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(theme::SOOT),
                    BorderColor::all(theme::CANDLE),
                    Pickable::IGNORE,
                    children![theme::text(cost, &fonts.num, 28.0, theme::CANDLE)],
                ));
            }
        })
        .id();
    Some(id)
}

fn sync_hand(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<Db>,
    layer: Single<Entity, With<HandLayer>>,
    cards: Query<(Entity, &CardWidget)>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let h = session.my_hero();
    let blind = h.flags.contains_key("blind");
    for (entity, w) in &cards {
        // a form change flips every card, so the widget is rebuilt
        if !h.hand.iter().any(|c| c.uid == w.uid) || w.form != h.form || w.hidden != blind {
            commands.entity(entity).despawn();
        }
    }
    for card in &h.hand {
        if !cards.iter().any(|(_, w)| w.uid == card.uid && w.form == h.form && w.hidden == blind) {
            // new cards slide in from the draw pile
            let from = Vec2::new(60.0, 900.0);
            let widget = (
                CardWidget { uid: card.uid, form: h.form, hidden: blind, pos: from, drag: Vec2::ZERO, dragging: false },
                // let the pointer through, so a dragged card can be dropped on the unit under it
                Pickable { should_block_lower: false, is_hoverable: true },
            );
            spawn_card(&mut commands, *layer, card, h.form, Some(from), blind, widget, &db.0, &l10n, &fonts, &art, &assets);
        }
    }
}

/// Keeps the potion belt in step with the hero's potions.
fn sync_potions(
    mut commands: Commands,
    session: Res<Session>,
    db: Res<Db>,
    belt: Single<Entity, With<PotionBelt>>,
    chips: Query<(Entity, &PotionChip)>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let potions = &session.my_hero().potions;
    let same = chips.iter().count() == potions.len() && chips.iter().all(|(_, c)| potions.get(c.slot as usize) == Some(&c.id));
    if same {
        return;
    }
    for (e, _) in &chips {
        commands.entity(e).despawn();
    }
    for (i, id) in potions.iter().enumerate() {
        let needs_target = db.0.potions.get(id).is_some_and(|d| matches!(d.target, CardTarget::Enemy | CardTarget::Downed));
        let slot = i as u8;
        let mut e = commands.spawn((
            PotionChip { slot, id: id.clone() },
            Button,
            Hovered::default(),
            Btn(theme::SWAMP),
            Pickable { should_block_lower: false, is_hoverable: true },
            Node { width: px(200), padding: UiRect::axes(px(10), px(6)), border: UiRect::all(px(2)), justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(theme::SWAMP),
            BorderColor::all(theme::ROTLIGHT),
            UiTransform::IDENTITY,
            GlobalZIndex(40),
            children![theme::text(l10n.t(&format!("potion-{id}")), &fonts.body, 17.0, theme::BONE)],
            ChildOf(*belt),
        ));
        // untargeted and ally draughts are drunk on a click; the rest must be thrown at something
        if !needs_target {
            e.observe(move |_: On<Activate>, mut outbox: ResMut<Outbox>| outbox.0.push(CombatAction::UsePotion { slot, target: None }));
        }
        e.observe(|ev: On<PointerDrag>, scale: Res<UiScale>, mut q: Query<&mut UiTransform, With<PotionChip>>| {
            if let Ok(mut tf) = q.get_mut(ev.event_target()) {
                tf.translation = Val2::px(ev.distance.x / scale.0, ev.distance.y / scale.0);
            }
        });
        e.observe(|ev: On<PointerDragEnd>, mut q: Query<&mut UiTransform, With<PotionChip>>| {
            if let Ok(mut tf) = q.get_mut(ev.event_target()) {
                tf.translation = Val2::ZERO;
            }
        });
    }
}

fn pick_hover(session: Res<Session>, cards: Query<(&CardWidget, &Hovered)>, mut top: ResMut<HoverTop>) {
    let hand = &session.my_hero().hand;
    let dragging = cards.iter().find(|(w, _)| w.dragging).map(|(w, _)| w.uid);
    let best = dragging.or_else(|| {
        cards
            .iter()
            .filter(|(_, h)| h.get())
            .filter_map(|(w, _)| hand.iter().position(|c| c.uid == w.uid).map(|i| (i, w.uid)))
            .max()
            .map(|(_, uid)| uid)
    });
    if top.0 != best {
        top.0 = best;
    }
}

fn layout_hand(
    session: Res<Session>,
    db: Res<Db>,
    time: Res<Time>,
    top: Res<HoverTop>,
    mut cards: Query<(&mut CardWidget, &mut Node, &mut UiTransform, &mut ZIndex, &mut BackgroundColor)>,
) {
    let h = session.my_hero();
    let n = h.hand.len();
    let k = (time.delta_secs() * 14.0).min(1.0);
    for (mut w, mut node, mut tf, mut z, mut bg) in &mut cards {
        let Some(i) = h.hand.iter().position(|c| c.uid == w.uid) else { continue };
        let mut target = hand_slot(i, n);
        let lifted = top.0 == Some(w.uid);
        if lifted && !w.dragging {
            target.y = HAND_TOP - 70.0;
        }
        w.pos = w.pos.lerp(target, k);
        let p = w.pos + w.drag;
        node.left = px(p.x);
        node.top = px(p.y);
        let scale = if w.dragging { 0.86 } else if lifted { 1.1 } else { 1.0 };
        tf.scale = tf.scale.lerp(Vec2::splat(scale), k);
        let want_z = if lifted { 100 } else { i as i32 };
        if z.0 != want_z {
            z.0 = want_z;
        }
        // cards you can't afford sink into the dark
        let card = &h.hand[i];
        if let Some(def) = db.0.cards.get(&card.def) {
            let view = def.view(card.upgraded, h.form);
            let cost = if view.x_cost { 0 } else { view.cost };
            let ok = !view.has(Keyword::Unplayable) && cost <= h.energy && !h.downed;
            let frame = card_frame(def);
            let want = if ok { frame } else { frame.mix(&theme::SOOT, 0.45) };
            if bg.0 != want {
                bg.0 = want;
            }
        }
    }
}

fn on_drag_start(ev: On<PointerDragStart>, mut cards: Query<&mut CardWidget>) {
    if let Ok(mut w) = cards.get_mut(ev.event_target()) {
        w.dragging = true;
    }
}

fn on_drag(ev: On<PointerDrag>, scale: Res<UiScale>, mut cards: Query<&mut CardWidget>) {
    if let Ok(mut w) = cards.get_mut(ev.event_target()) {
        w.drag = ev.distance / scale.0;
    }
}

fn on_drag_end(
    ev: On<PointerDragEnd>,
    scale: Res<UiScale>,
    mut cards: Query<&mut CardWidget>,
    session: Option<Res<Session>>,
    db: Res<Db>,
    mut outbox: ResMut<Outbox>,
) {
    let Ok(mut w) = cards.get_mut(ev.event_target()) else { return };
    let lifted = ev.distance.y / scale.0 < -130.0;
    w.dragging = false;
    // keep the card where it was dropped; the hand layout eases it home or it leaves the hand
    let dropped = w.drag;
    w.pos += dropped;
    w.drag = Vec2::ZERO;
    let Some(session) = session else { return };
    let h = session.my_hero();
    let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { return };
    let Some(def) = db.0.cards.get(&card.def) else { return };
    let view = def.view(card.upgraded, h.form);
    // untargeted cards are played by flicking them up onto the field
    let alone = session.truth.standing_heroes().count() == 1;
    let untargeted = view.target == CardTarget::None || (view.target == CardTarget::Ally && alone);
    if lifted && untargeted {
        outbox.0.push(CombatAction::Play { card: w.uid, target: None });
    }
}

fn on_drop(
    ev: On<PointerDragDrop>,
    units: Query<&UnitWidget>,
    potions: Query<&PotionChip>,
    cards: Query<&CardWidget>,
    session: Option<Res<Session>>,
    db: Res<Db>,
    mut outbox: ResMut<Outbox>,
) {
    // a potion thrown at a unit
    if let (Ok(unit), Ok(chip)) = (units.get(ev.event_target()), potions.get(ev.dropped)) {
        outbox.0.push(CombatAction::UsePotion { slot: chip.slot, target: Some(unit.unit) });
        return;
    }
    let (Ok(unit), Ok(w), Some(session)) = (units.get(ev.event_target()), cards.get(ev.dropped), session) else { return };
    let h = session.my_hero();
    let Some(card) = h.hand.iter().find(|c| c.uid == w.uid) else { return };
    let Some(def) = db.0.cards.get(&card.def) else { return };
    let view = def.view(card.upgraded, h.form);
    let downed = matches!(unit.unit, UnitRef::Hero(i) if session.truth.hero(i).is_some_and(|a| a.downed));
    if downed && view.target != CardTarget::Downed {
        // any card on a fallen ally is the Raise action, paid with that card
        if let UnitRef::Hero(ally) = unit.unit {
            outbox.0.push(CombatAction::Raise { ally, discard: w.uid });
        }
    } else if view.target != CardTarget::None {
        outbox.0.push(CombatAction::Play { card: w.uid, target: Some(unit.unit) });
    }
}

// ---- tooltips ----

#[derive(Resource, Default)]
pub struct TooltipText(String);

fn update_tooltip(
    session: Res<Session>,
    db: Res<Db>,
    l10n: Res<L10n>,
    top: Res<HoverTop>,
    cards: Query<&CardWidget>,
    units: Query<(&UnitWidget, &Hovered)>,
    mut tip: ResMut<TooltipText>,
    mut panel: Single<(&mut Node, &mut Visibility), With<Tooltip>>,
) {
    let mut lines: Vec<(String, String)> = Vec::new();
    let mut at = Vec2::ZERO;
    let h = session.my_hero();
    if let Some(w) = top.0.and_then(|uid| cards.iter().find(|w| w.uid == uid && !w.dragging && !w.hidden)) {
        if let Some((card, def)) = h.hand.iter().find(|c| c.uid == w.uid).and_then(|c| db.0.cards.get(&c.def).map(|d| (c, d))) {
            lines = l10n.card_glossary(&def.view(card.upgraded, h.form));
            at = Vec2::new(w.pos.x + CARD_W * 1.08 + 14.0, HAND_TOP - 260.0);
            if at.x + 340.0 > STAGE_W {
                at.x = w.pos.x - 350.0;
            }
        }
    } else if let Some((w, _)) = units.iter().find(|(_, hov)| hov.get()) {
        let statuses = match w.unit {
            UnitRef::Hero(i) => session.shown.hero(i).map(|x| &x.statuses),
            UnitRef::Enemy(i) => session.shown.enemy(i).map(|x| &x.statuses),
        };
        if let UnitRef::Enemy(uid) = w.unit {
            if let Some(e) = session.shown.enemy(uid) {
                let sung = session.my_hero().flags.contains_key("no_intents");
                if let Some(mv) = e.intent.as_ref().filter(|_| !sung).and_then(|i| db.0.enemies.get(&e.def)?.moves.get(i.mv)) {
                    let key = format!("move-{}", mv.id);
                    let desc = format!("{key}-desc");
                    lines.push((l10n.t(&key), if l10n.has(&desc) { l10n.t(&desc) } else { l10n.t(&format!("intent-{:?}", mv.intent)) }));
                }
            }
        }
        for st in statuses.into_iter().flat_map(|m| m.keys()) {
            lines.push((l10n.t(&format!("status-{st:?}")), l10n.t(&format!("status-{st:?}-help"))));
        }
        at = Vec2::new((w.pos.x + UNIT_W + 6.0).min(STAGE_W - 350.0), w.pos.y + 80.0);
    }
    let body = lines.iter().map(|(k, v)| format!("{k} — {v}")).collect::<Vec<_>>().join("\n\n");
    let (node, vis) = &mut *panel;
    let want = if body.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
    if **vis != want {
        **vis = want;
    }
    if !body.is_empty() {
        node.left = px(at.x);
        node.top = px(at.y);
    }
    if tip.0 != body {
        tip.0 = body;
    }
}

// ---- juice ----

fn cue_effects(
    mut commands: Commands,
    cues: Res<Cues>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    mut sprites: Query<&mut UnitSprite>,
    mut shake: ResMut<Shake>,
    comfort: Res<Comfort>,
    db: Res<Db>,
) {
    let float = |commands: &mut Commands, unit: UnitRef, text: String, color: Color, size: f32, dx: f32| {
        let Some((foot, sz)) = unit_place(&session.shown, unit) else { return };
        commands.spawn((
            Floater { ttl: 0.95, life: 0.95 },
            theme::text(text, &fonts.num, size, color),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, left: px(foot.x - 20.0 + dx), top: px(foot.y - sz.y * 0.72), ..default() },
            ChildOf(*layer),
        ));
    };
    let mut play = |unit: UnitRef, anim: Anim| {
        for mut s in &mut sprites {
            if s.unit == unit && s.anim.is_none_or(|(now, _)| anim.weight() >= now.weight()) {
                s.anim = Some((anim, 0.0));
            }
        }
    };
    // where a unit's body is, for things that fly between units
    let body = |unit: UnitRef| unit_place(&session.shown, unit).map(|(foot, sz)| Vec2::new(foot.x, foot.y - sz.y * 0.5));
    let ghost = |commands: &mut Commands, from: Vec2, to: Vec2, text: String, edge: Color| {
        commands.spawn((
            Ghost { from, to, t: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                left: px(from.x - 70.0),
                top: px(from.y - 45.0),
                width: px(140),
                height: px(90),
                padding: UiRect::all(px(6)),
                border: UiRect::all(px(3)),
                border_radius: BorderRadius::all(px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme::PEAT),
            BorderColor::all(edge),
            UiTransform::IDENTITY,
            Pickable::IGNORE,
            children![(theme::text(text, &fonts.title, 15.0, theme::BONE), TextLayout::justify(Justify::Center))],
            ChildOf(*layer),
        ));
    };
    for e in &cues.0 {
        match e {
            Event::Damage { src, dst, blocked, lost, attack, .. } => {
                if *attack {
                    if let Some(s) = src {
                        play(*s, Anim::Lunge);
                    }
                }
                if *lost > 0 {
                    // the screen shakes with the size of the blow, 12 px at most
                    let s = (*lost as f32 * 0.5).min(12.0);
                    if s > shake.strength {
                        shake.strength = s;
                        shake.t = 0.0;
                    }
                    play(*dst, Anim::Hit);
                    float(&mut commands, *dst, format!("-{lost}"), comfort.blood(), 46.0 + (*lost as f32).min(30.0), 0.0);
                }
                if *blocked > 0 {
                    float(&mut commands, *dst, format!("-{blocked}"), theme::FROST, 34.0, -70.0);
                }
            }
            Event::Block { unit, amount, .. } => {
                play(*unit, Anim::Guarded);
                float(&mut commands, *unit, format!("+{amount}"), theme::NAV_BLUE.lighter(0.25), 36.0, -70.0)
            }
            Event::Heal { unit, amount, .. } => {
                play(*unit, Anim::Healed);
                float(&mut commands, *unit, format!("+{amount}"), theme::ROTLIGHT, 38.0, 40.0)
            }
            Event::Status { unit, status, delta, .. } if *delta > 0 => {
                play(*unit, if status.debuff() { Anim::Cursed } else { Anim::Buffed });
                let color = if status.debuff() { theme::ROTLIGHT } else { theme::CANDLE };
                float(&mut commands, *unit, format!("{} +{delta}", l10n.t(&format!("status-{status:?}"))), color, 24.0, 30.0)
            }
            Event::EnemySkipped { enemy, .. } => float(&mut commands, UnitRef::Enemy(*enemy), "…".into(), theme::BONE, 44.0, 0.0),
            // every card, anyone's, is seen leaving its player and arriving where it lands
            Event::CardPlayed { player, card, upgraded, target } => {
                let who = UnitRef::Hero(*player);
                let kind = db.0.cards.get(card).map(|d| d.kind);
                if kind != Some(CardType::Attack) {
                    play(who, Anim::Cast);
                }
                if let Some(from) = body(who) {
                    let hero = session.shown.hero(*player).map(|h| h.def.clone()).unwrap_or_default();
                    // an aimed card flies to its target; a sweeping one to the middle of the other side
                    let to = target.and_then(|t| body(t)).unwrap_or(Vec2::new(if kind == Some(CardType::Attack) { 1450.0 } else { from.x }, from.y - 150.0));
                    ghost(&mut commands, from, to, l10n.card_name(card, *upgraded), theme::hero_color(&hero));
                }
            }
            Event::PotionUsed { player, potion, target } => {
                let who = UnitRef::Hero(*player);
                play(who, Anim::Cast);
                if let Some(from) = body(who) {
                    let to = target.and_then(|t| body(t)).unwrap_or(Vec2::new(from.x, from.y - 150.0));
                    ghost(&mut commands, from, to, l10n.t(&format!("potion-{potion}")), theme::ROTLIGHT);
                }
            }
            // an enemy's ability is named over its head as it acts
            Event::EnemyMove { enemy, mv } => {
                let unit = UnitRef::Enemy(*enemy);
                let def = session.shown.enemy(*enemy).and_then(|e| db.0.enemies.get(&e.def));
                if let Some(m) = def.and_then(|d| d.moves.get(*mv)) {
                    if !matches!(m.intent, IntentKind::Attack | IntentKind::AttackAll) {
                        play(unit, Anim::Cast);
                    }
                    float(&mut commands, unit, l10n.t(&format!("move-{}", m.id)), theme::BONE, 26.0, -60.0);
                }
            }
            Event::RuneFired { player, rune } => {
                play(UnitRef::Hero(*player), Anim::Buffed);
                float(&mut commands, UnitRef::Hero(*player), l10n.t(&format!("rune-{rune:?}")), theme::FROST, 22.0, 50.0)
            }
            Event::RuneCarved { player, .. } | Event::HerbAdded { player, .. } | Event::MaskChanged { player, .. } | Event::Shifted { player, .. } => play(UnitRef::Hero(*player), Anim::Buffed),
            Event::Brewed { player, recipe } => {
                play(UnitRef::Hero(*player), Anim::Cast);
                let key = format!("recipe-{recipe}");
                if l10n.has(&key) {
                    float(&mut commands, UnitRef::Hero(*player), l10n.t(&key), theme::ROTLIGHT, 24.0, 50.0);
                }
            }
            Event::Raised { player, .. } => play(UnitRef::Hero(*player), Anim::Healed),
            Event::Summoned { enemy, .. } => play(UnitRef::Enemy(*enemy), Anim::Buffed),
            Event::Phase2 { enemy } => {
                play(UnitRef::Enemy(*enemy), Anim::Buffed);
                shake.strength = 12.0;
                shake.t = 0.0;
            }
            _ => {}
        }
    }
}

/// Flies each ghost along a shallow arc and lets it fade into its target.
fn fly_ghosts(mut commands: Commands, time: Res<Time>, speed: Res<AnimSpeed>, mut q: Query<(Entity, &mut Ghost, &mut Node, &mut UiTransform, &mut BackgroundColor)>) {
    for (entity, mut g, mut node, mut tf, mut bg) in &mut q {
        g.t += time.delta_secs() * speed.0 / 0.42;
        if g.t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // ease out: quick off the hand, settling on arrival
        let p = 1.0 - (1.0 - g.t).powi(2);
        let at = g.from.lerp(g.to, p) + Vec2::new(0.0, -90.0 * (p * std::f32::consts::PI).sin());
        node.left = px(at.x - 70.0);
        node.top = px(at.y - 45.0);
        tf.scale = Vec2::splat(1.0 - 0.45 * p);
        tf.rotation = Rot2::radians(0.25 * (1.0 - p) * if g.to.x >= g.from.x { 1.0 } else { -1.0 });
        bg.0.set_alpha(1.0 - 0.6 * p * p);
    }
}

fn floaters(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor)>) {
    for (entity, mut f, mut node, mut color) in &mut q {
        let dt = time.delta_secs();
        f.ttl -= dt;
        if f.ttl <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // pop up fast, then hang and fade
        let p = 1.0 - f.ttl / f.life;
        let speed = 190.0 * (1.0 - p).powi(2) + 14.0;
        if let Val::Px(top) = node.top {
            node.top = px(top - speed * dt);
        }
        color.0.set_alpha(((1.0 - p) * 2.2).min(1.0));
    }
}

/// Text of the turn clock, empty when the timer is off.
#[derive(Resource, Default)]
struct TimerText(String);

/// The optional turn timer: when it runs out, this player is marked ready.
fn turn_timer(
    session: Res<Session>,
    net: Option<Res<crate::net::Net>>,
    time: Res<Time>,
    mut clock: ResMut<TimerText>,
    mut outbox: ResMut<Outbox>,
    mut state: Local<(u32, f32)>,
) {
    let limit = net.as_ref().and_then(|n| n.node.lobby.turn_timer);
    let Some(limit) = limit else {
        if !clock.0.is_empty() {
            clock.0.clear();
        }
        return;
    };
    // the clock restarts with every round
    if state.0 != session.truth.round {
        *state = (session.truth.round, 0.0);
    }
    let h = session.my_hero();
    if session.truth.phase != Phase::Heroes || h.ready || h.downed {
        clock.0.clear();
        return;
    }
    if session.idle() {
        state.1 += time.delta_secs();
    }
    let left = (limit as f32 - state.1).ceil().max(0.0) as i32;
    let text = left.to_string();
    if clock.0 != text {
        clock.0 = text;
    }
    if left == 0 && outbox.0.is_empty() {
        outbox.0.push(CombatAction::Ready(true));
        state.1 = -5.0;
    }
}

/// Quick signals without chat: hover a unit and press 1-5.
fn send_pings(keys: Res<ButtonInput<KeyCode>>, units: Query<(&UnitWidget, &Hovered)>, mut net: Option<ResMut<crate::net::Net>>) {
    let Some(net) = net.as_mut() else { return };
    let kind = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5].iter().position(|k| keys.just_pressed(*k));
    let (Some(kind), Some((w, _))) = (kind, units.iter().find(|(_, h)| h.get())) else { return };
    net.node.ping(kind as u8, w.unit);
}

fn show_pings(
    mut commands: Commands,
    mut status: ResMut<crate::net::NetStatus>,
    session: Res<Session>,
    layer: Single<Entity, With<FloatLayer>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    for (from, kind, at) in status.pings.drain(..) {
        let Some((foot, size)) = unit_place(&session.shown, at) else { continue };
        let color = session.shown.hero(from).map(|h| theme::hero_color(&h.def).lighter(0.25)).unwrap_or(theme::BONE);
        commands.spawn((
            Floater { ttl: 2.2, life: 2.2 },
            theme::text(l10n.t(&format!("ping-{kind}")), &fonts.title, 26.0, color),
            TextShadow::default(),
            Node { position_type: PositionType::Absolute, left: px(foot.x - 90.0), top: px(foot.y - size.y - 30.0), ..default() },
            ChildOf(*layer),
        ));
    }
}

fn shake_screen(time: Res<Time>, comfort: Res<Comfort>, mut shake: ResMut<Shake>, mut root: Single<&mut UiTransform, With<BattleRoot>>) {
    if shake.strength <= 0.01 {
        return;
    }
    shake.t += time.delta_secs();
    // a quarter of a second, dying away
    let left = (1.0 - shake.t / 0.25).max(0.0);
    let amp = shake.strength * left * if comfort.less_shake { 0.25 } else { 1.0 };
    root.translation = Val2::px((shake.t * 95.0).sin() * amp, (shake.t * 71.0).cos() * amp * 0.6);
    if left <= 0.0 {
        shake.strength = 0.0;
        root.translation = Val2::ZERO;
    }
}

pub fn style_buttons(mut buttons: Query<(&Btn, &Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>) {
    for (btn, hovered, pressed, mut bg) in &mut buttons {
        let want = match (hovered.get(), pressed) {
            (_, true) => btn.0.darker(0.06),
            (true, false) => btn.0.lighter(0.08),
            _ => btn.0,
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
