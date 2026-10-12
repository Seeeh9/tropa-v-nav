"""The third card set: fills every hero up to 75 cards + 5 co-op ones.

The tables below are the source; this script writes
assets/data/cards/set3.ron and assets/i18n/{ru,en}/cards3.ftl from them.
Card text is not written here: the game composes it from the effects.

    python tools/cards3.py

A row is
    c(id, kind, rarity, cost, target, effects, base, up, ru, en, kw="", upcost=None)
kind:    A attack, S skill, H herb, P power (effects are then "Trigger|effects")
rarity:  C common, U uncommon, R rare
target:  E enemy, H any hero, A ally, D downed, - none
effects: RON with {0} {1}.. for the numbers; `base` and `up` fill them in
kw:      letters: x exhaust, r retain, e ethereal, c co-op, X x-cost
upcost:  the upgrade changes the cost instead of (or as well as) the numbers
beast:   the werewolf's other side: (kind, target, effects, base, up)

Balance (design doc, section 4): 1 energy = 6 damage to one, 5 block, 4 damage to all,
2 stacks of a debuff; a card drawn = 0.5; uncommon +15%, rare +30% or a unique effect;
upgrade = +30-50% or -1 cost.
"""
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
KIND = {"A": "Attack", "S": "Skill", "P": "Power", "H": "Herb"}
RARITY = {"C": "Common", "U": "Uncommon", "R": "Rare"}
TARGET = {"E": "Enemy", "H": "AnyHero", "A": "Ally", "D": "Downed", "-": None}
KW = {"x": "Exhaust", "r": "Retain", "e": "Ethereal", "c": "Coop"}

CARDS = {}
_hero = None


def hero(name):
    global _hero
    _hero = name
    CARDS[name] = []


def c(cid, kind, rarity, cost, target, fx, base, up, ru, en, kw="", upcost=None, beast=None):
    CARDS[_hero].append((cid, kind, rarity, cost, target, fx, base, up, ru, en, kw, upcost, beast))


def tup(v):
    return v if isinstance(v, tuple) else (v,)


# ======================= Богатырь =======================
hero("bogatyr")
c("bog_cleave", "A", "C", 1, "-", "Damage(N({0}), AllFoes)", 4, 6, "Рубящий мах", "Cleaving Blow")
c("bog_heavy_swing", "A", "C", 2, "E", "Damage(N({0}), Target)", 13, 18, "Тяжёлый замах", "Heavy Swing")
c("bog_pommel", "A", "C", 0, "E", "Damage(N({0}), Target)", 4, 6, "Удар навершием", "Pommel Strike")
c("bog_shove", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N({1}), Target)", (4, 1), (5, 2), "Толчок", "Shove")
c("bog_hold_line", "S", "C", 1, "-", "Block(N({0}), Me), Apply(Valor, N({1}), Me)", (4, 1), (6, 2), "Держать строй", "Hold the Line")
c("bog_cover", "S", "C", 1, "H", "Guard(N({0}), Target)", 9, 12, "Прикрыть", "Cover")
c("bog_raise_shield", "S", "C", 2, "-", "Block(N({0}), Me)", 11, 15, "Поднять щит", "Raise the Shield")
c("bog_mace_blow", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Valor, N(1), Me)", 5, 8, "Удар шестопёром", "Mace Blow")
c("bog_second_wind", "S", "C", 1, "-", "Block(N({0}), Me), Draw(N(1))", 3, 6, "Второе дыхание", "Second Wind")
c("bog_shoulder", "A", "C", 1, "E", "Damage(N({0}), Target), If(Link(Skill), [Damage(N({1}), Target)])", (6, 3), (8, 5), "Плечом к плечу", "Shoulder to Shoulder")
c("bog_breakthrough", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target)", 4, 7, "Пролом", "Breakthrough")
c("bog_lend_shield", "S", "C", 1, "H", "Block(N({0}), Target), Draw(N(1))", 4, 7, "Одолжить щит", "Lend a Shield")
c("bog_grit", "S", "C", 0, "-", "Block(N({0}), Me)", 3, 5, "Стиснуть зубы", "Grit")
c("bog_double_blow", "A", "C", 1, "E", "Hits(N({0}), 2, Target)", 3, 5, "Двойной удар", "Double Blow")
c("bog_trample", "A", "C", 2, "-", "Damage(N({0}), AllFoes)", 8, 11, "Потоптать", "Trample")
c("bog_brave_step", "S", "C", 1, "-", "Apply(Valor, N({0}), Me), Block(N(3), Me)", 2, 3, "Смелый шаг", "Brave Step")
c("bog_valiant_strike", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Stat(Valor, Me, 1), Target)", 5, 8, "Удалой удар", "Valiant Strike")
c("bog_close_ranks", "S", "C", 1, "-", "Block(N({0}), AllFriends)", 3, 5, "Сомкнуть ряды", "Close Ranks")
c("bog_backhand", "A", "C", 1, "-", "Damage(N({0}), RandomFoe), Damage(N({0}), RandomFoe)", 4, 6, "Наотмашь по толпе", "Backhand Sweep")
c("bog_on_guard", "S", "C", 1, "-", "Block(N({0}), Me), Apply(Thorns, N({1}), Me)", (4, 1), (5, 2), "Настороже", "On Guard")
c("bog_rebuke", "A", "C", 1, "E", "Damage(N({0}), Target), If(Has(Weak, Target, 1), [Damage(N({1}), Target)])", (6, 4), (8, 6), "Отповедь", "Rebuke")
c("bog_lunge", "A", "C", 1, "E", "Damage(N({0}), Target), Draw(N(1))", 3, 6, "Выпад", "Lunge")
c("bog_catch_breath", "S", "C", 1, "-", "Heal(N({0}), Me), Block(N(3), Me)", 2, 4, "Перевести дух", "Catch Your Breath")
c("bog_wide_guard", "S", "C", 2, "H", "Guard(N({0}), Target), Block(N({1}), Me)", (10, 5), (13, 7), "Широкий заслон", "Wide Guard")
c("bog_ram", "A", "C", 2, "E", "Damage(N({0}), Target), Apply(Weak, N(2), Target)", 9, 13, "Таран", "Ram")
c("bog_vow", "S", "C", 1, "-", "Apply(TempStrength, N({0}), Me), Block(N(4), Me)", 2, 4, "Зарок", "Vow")

c("bog_shatter", "A", "U", 2, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(2), Target)", 10, 14, "Расколоть", "Shatter")
c("bog_iron_wall", "S", "U", 2, "-", "Block(N({0}), Me), Apply(Thorns, N({1}), Me)", (12, 2), (15, 3), "Железная стена", "Iron Wall")
c("bog_avenger", "A", "U", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Apply(Valor, N(2), Me)])", 7, 10, "Мститель", "Avenger")
c("bog_bear_hug", "A", "U", 3, "E", "Damage(N({0}), Target), Apply(Stun, N(1), Target)", 10, 14, "Медвежья хватка", "Bear Hug", kw="x")
c("bog_whirl", "A", "U", 2, "-", "Damage(N({0}), AllFoes), Apply(Valor, N(1), Me)", 9, 12, "Вихрь", "Whirl")
c("bog_guardian_oath", "P", "U", 1, "-", "TurnStart|Block(N({0}), AllFriends)", 2, 3, "Клятва стража", "Guardian's Oath")
c("bog_blood_price", "S", "U", 0, "-", "LoseHp(N(3), Me), Apply(Valor, N({0}), Me)", 3, 4, "Цена крови", "Blood Price")
c("bog_stalwart", "P", "U", 1, "-", "Hurt|Block(N({0}), Me)", 3, 5, "Несгибаемый", "Stalwart")
c("bog_feint", "A", "U", 1, "E", "Damage(N({0}), Target), Apply(Weak, N({1}), Target), Draw(N(1))", (4, 1), (6, 2), "Обманный удар", "Feint")
c("bog_stone_skin", "S", "U", 1, "-", "Block(N({0}), Me), Cleanse(Me)", 5, 8, "Каменная кожа", "Stone Skin")
c("bog_mighty_arm", "P", "U", 2, "-", "TurnStart|Apply(TempStrength, N({0}), Me)", 2, 3, "Могучая рука", "Mighty Arm")
c("bog_wallbreaker", "A", "U", 2, "E", "Damage(N({0}), Target), Damage(Stat(Valor, Me, 2), Target)", 8, 12, "Стенолом", "Wallbreaker")
c("bog_shield_the_weak", "S", "U", 1, "H", "Guard(N({0}), Target), Heal(N({1}), Target)", (6, 3), (8, 5), "Защитить слабого", "Shield the Weak")
c("bog_grim_resolve", "S", "U", 1, "-", "Draw(N({0})), Apply(Valor, N(1), Me)", 2, 3, "Мрачная решимость", "Grim Resolve")
c("bog_riposte", "P", "U", 1, "-", "Hurt|Damage(N({0}), RandomFoe)", 4, 6, "Ответный удар", "Riposte")
c("bog_bell_ringer", "A", "U", 1, "E", "Damage(N({0}), Target), If(Has(Vulnerable, Target, 1), [Apply(Weak, N(2), Target)])", 7, 10, "Звонарь", "Bell-Ringer")
c("bog_long_watch", "S", "U", 2, "-", "Block(N({0}), AllFriends), Draw(N(1))", 6, 9, "Долгий дозор", "The Long Watch")
c("bog_sunder_rot", "A", "U", 1, "E", "Consume(Rot, Target), Damage(N({0}), Target), Damage(Consumed(2), Target)", 6, 9, "Рассечь гниль", "Sunder the Rot")
c("bog_brothers_in_arms", "S", "U", 1, "-", "Apply(TempStrength, N({0}), OtherFriends), Block(N({1}), OtherFriends)", (2, 4), (3, 6), "Братья по оружию", "Brothers in Arms", kw="c")
c("bog_take_the_blow", "S", "U", 0, "A", "Guard(N({0}), Target), Apply(Valor, N(1), Me)", 8, 12, "Принять удар", "Take the Blow", kw="c")

c("bog_earthshaker", "A", "R", 3, "-", "Damage(N({0}), AllFoes), Apply(Weak, N(1), AllFoes)", 14, 19, "Землетряс", "Earthshaker")
c("bog_unbreakable", "P", "R", 3, "-", "TurnEnd|Block(N({0}), Me), Apply(Valor, N(1), Me)", 6, 8, "Несокрушимый", "Unbreakable")
c("bog_face_of_rod", "S", "R", 2, "-", "Block(N({0}), AllFriends), Apply(Thorns, N({1}), AllFriends)", (10, 2), (13, 3), "Лик Рода", "The Face of Rod", kw="x")
c("bog_giant_slayer", "A", "R", 2, "E", "Damage(N({0}), Target), If(Fatal, [Apply(Strength, N({1}), Me)])", (15, 1), (18, 2), "Великаноборец", "Giant-Slayer")
c("bog_hero_heart", "P", "R", 2, "-", "Kill|Heal(N({0}), AllFriends)", 3, 5, "Сердце богатыря", "A Hero's Heart")
c("bog_mountain", "S", "R", 3, "-", "Block(N({0}), Me), Taunt", 24, 32, "Гора", "The Mountain")
c("bog_war_song", "P", "R", 2, "-", "TurnStart|Apply(TempStrength, N({0}), AllFriends)", 1, 2, "Ратная песнь", "War Song")
c("bog_judgement", "A", "R", 1, "E", "Damage(Stat(Valor, Me, {0}), Target)", 3, 4, "Суд", "Judgement", kw="x")
c("bog_thousand_blows", "A", "R", 2, "E", "Hits(N({0}), 4, Target)", 4, 5, "Тысяча ударов", "A Thousand Blows")
c("bog_iron_oath", "S", "R", 1, "-", "Apply(Fortitude, N({0}), Me), Apply(Strength, N(1), Me)", 2, 3, "Железная клятва", "Iron Oath", kw="x")
c("bog_old_guard", "S", "R", 2, "H", "Guard(N({0}), Target), Apply(Valor, N({1}), Me), Draw(N(1))", (14, 2), (18, 3), "Старая гвардия", "The Old Guard")
c("bog_storm_of_steel", "A", "R", 0, "-", "Repeat(X(1), [Damage(N({0}), AllFoes)])", 5, 7, "Буря стали", "Storm of Steel", kw="X")
c("bog_shield_for_all", "S", "R", 2, "-", "Block(N({0}), OtherFriends), Apply(Fortitude, N(1), OtherFriends)", 9, 12, "Щит для всех", "A Shield for All", kw="c")
c("bog_as_one", "S", "R", 1, "-", "Energy(N({0}), OtherFriends), Block(N(4), AllFriends)", 1, 2, "Все как один", "As One", kw="cx")



# ======================= Ворожея =======================
hero("vorozheya")
c("vor_thorn", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Rot, N({1}), Target)", (4, 2), (6, 3), "Шип", "Thorn")
c("vor_evil_eye", "S", "C", 1, "E", "Apply(Hex, N({0}), Target), Apply(Weak, N(1), Target)", 1, 2, "Дурной глаз", "Ill Glance")
c("vor_mutter", "S", "C", 0, "E", "Apply(Rot, N({0}), Target)", 2, 3, "Бормотание", "Mutter")
c("vor_ash_circle", "S", "C", 1, "-", "Block(N({0}), Me), Apply(Rot, N(1), AllFoes)", 4, 6, "Круг из золы", "Ash Circle")
c("vor_spit", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N(1), Target)", 4, 6, "Плевок через плечо", "Spit Over the Shoulder")
c("vor_scratch", "A", "C", 0, "E", "Damage(N({0}), Target), Apply(Hex, N(1), Target)", 2, 4, "Царапина", "Scratch")
c("vor_black_shawl", "S", "C", 1, "-", "Block(N({0}), Me)", 6, 9, "Чёрный платок", "Black Shawl")
c("vor_rot_touch", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Stat(Rot, Target, 1), Target)", 4, 6, "Гнилое касание", "Rotting Touch")
c("vor_midges", "S", "C", 1, "-", "Apply(Rot, N({0}), AllFoes)", 2, 3, "Мошкара", "Midges")
c("vor_dark_whisper", "S", "C", 1, "-", "Draw(N(2)), LoseHp(N(2), Me)", 0, 0, "Тёмный шёпот", "Dark Whisper", upcost=0)
c("vor_bone_pin", "A", "C", 2, "E", "Damage(N({0}), Target), Apply(Hex, N(2), Target)", 8, 12, "Костяная булавка", "Bone Pin")
c("vor_wax_figure", "S", "C", 1, "E", "Apply(Rot, N({0}), Target), Apply(Hex, N(1), Target)", 3, 4, "Восковая фигурка", "Wax Figure")
c("vor_ill_word", "A", "C", 1, "E", "Damage(N({0}), Target), If(Has(Hex, Target, 1), [Damage(N({1}), Target)])", (5, 4), (7, 6), "Худое слово", "Ill Word")
c("vor_veil", "S", "C", 1, "H", "Block(N({0}), Target), Apply(Mist, N(1), Target)", 3, 5, "Пелена", "Veil")
c("vor_blight", "A", "C", 1, "-", "Damage(N({0}), AllFoes), Apply(Rot, N(1), AllFoes)", 2, 4, "Поветрие", "Blight")
c("vor_knot", "S", "C", 1, "E", "Apply(Weak, N({0}), Target), Draw(N(1))", 1, 2, "Узелок", "Knot")
c("vor_blood_drop", "S", "C", 0, "-", "LoseHp(N(2), Me), Draw(N({0}))", 2, 3, "Капля крови", "Drop of Blood")
c("vor_dead_mark", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target)", 4, 7, "Метка покойника", "Dead Man's Mark")
c("vor_wither", "A", "C", 2, "E", "Damage(N({0}), Target), Apply(Rot, N({1}), Target)", (8, 4), (11, 5), "Иссушить", "Wither")
c("vor_salt_line", "S", "C", 1, "-", "Block(N({0}), Me), If(Link(Skill), [Block(N({1}), Me)])", (5, 3), (7, 5), "Соляная черта", "Salt Line")
c("vor_crow_call", "A", "C", 1, "-", "Damage(N({0}), RandomFoe), Apply(Hex, N(1), RandomFoe)", 5, 8, "Вороний грай", "Crow Call")
c("vor_grave_chill", "S", "C", 1, "E", "Apply(Weak, N(2), Target), Block(N({0}), Me)", 2, 4, "Могильный холод", "Grave Chill")
c("vor_sour_milk", "S", "C", 1, "E", "Apply(Rot, N({0}), Target), If(Link(Attack), [Apply(Rot, N(2), Target)])", 3, 4, "Скисшее молоко", "Soured Milk")
c("vor_cut_thread", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Curses(Target, 1), Target)", 4, 6, "Перерезать нить", "Cut the Thread")
c("vor_hen_egg", "S", "C", 1, "-", "Block(N({0}), Me), Heal(N(2), Me)", 4, 6, "Яйцо чёрной курицы", "Black Hen's Egg")
c("vor_hiss", "S", "C", 1, "-", "Apply(Weak, N(1), AllFoes), Block(N({0}), Me)", 2, 5, "Шипение", "Hiss")

c("vor_foul_air", "S", "U", 2, "-", "Apply(Rot, N({0}), AllFoes), Apply(Weak, N(1), AllFoes)", 3, 4, "Дурной воздух", "Foul Air")
c("vor_drain", "A", "U", 1, "E", "Damage(N({0}), Target), Heal(N({1}), Me)", (5, 3), (7, 4), "Вытянуть жизнь", "Drain")
c("vor_hex_them_all", "S", "U", 2, "-", "Apply(Hex, N({0}), AllFoes)", 2, 3, "Сглаз на всех", "Hex Them All")
c("vor_overripe", "S", "U", 1, "E", "Apply(Rot, Stat(Rot, Target, 1), Target)", 0, 0, "Перезреть", "Overripe", upcost=0)
c("vor_blood_circle", "P", "U", 1, "-", "TurnStart|Apply(Rot, N({0}), RandomFoe)", 2, 3, "Кровавый круг", "Circle of Blood")
c("vor_malice", "P", "U", 1, "-", "Hurt|Apply(Hex, N({0}), AllFoes)", 1, 2, "Чёрная злость", "Malice")
c("vor_coffin_nail", "A", "U", 2, "E", "Damage(N({0}), Target), Damage(Curses(Target, 2), Target)", 6, 9, "Гробовой гвоздь", "Coffin Nail")
c("vor_trade", "S", "U", 0, "-", "LoseHp(N(4), Me), Energy(N({0}), Me)", 2, 3, "Обмен", "A Trade")
c("vor_canker", "P", "U", 2, "-", "TurnEnd|Apply(Rot, N({0}), AllFoes)", 1, 2, "Червоточина", "Canker")
c("vor_gloat", "S", "U", 1, "E", "Block(N(3), Me), Block(Curses(Target, {0}), Me)", 2, 3, "Злорадство", "Gloat")
c("vor_lance_boil", "A", "U", 1, "E", "Consume(Rot, Target), Damage(Consumed({0}), Target), Apply(Rot, N(2), Target)", 2, 3, "Вскрыть нарыв", "Lance the Boil")
c("vor_shared_pain", "A", "U", 1, "E", "Consume(Hex, Target), Damage(N({0}), Target), Damage(Consumed(4), AllFoes)", 5, 8, "Общая боль", "Shared Pain")
c("vor_sleepless", "S", "U", 1, "E", "Apply(Weak, N({0}), Target), Apply(Vulnerable, N({0}), Target)", 1, 2, "Бессонница", "Sleeplessness")
c("vor_mourning_veil", "S", "U", 2, "-", "Block(N({0}), Me), Apply(Hex, N(1), AllFoes)", 10, 13, "Траурная фата", "Mourning Veil")
c("vor_old_grudge", "A", "U", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Apply(Hex, N(2), Target)])", 6, 9, "Старая обида", "Old Grudge")
c("vor_draw_sickness", "S", "U", 1, "H", "Heal(N({0}), Target), LoseHp(N(2), Me)", 6, 9, "Отвести хворь", "Draw Out the Sickness")
c("vor_evil_hour", "P", "U", 2, "-", "CardPlayed|OncePerTurn(\"evil_hour\", [Apply(Rot, N({0}), AllFoes)])", 1, 2, "Недобрый час", "The Evil Hour")
c("vor_dead_tongue", "A", "U", 2, "-", "Damage(N({0}), AllFoes), Apply(Weak, N(1), AllFoes)", 6, 9, "Мёртвый язык", "Dead Tongue")
c("vor_guide_hand", "S", "U", 1, "A", "Apply(TempStrength, N({0}), Target), Apply(Hex, N(2), AllFoes)", 3, 4, "Навести руку", "Guide the Hand", kw="c")
c("vor_hex_circle", "S", "U", 1, "-", "Apply(Hex, N({0}), AllFoes), Block(N(3), OtherFriends)", 1, 2, "Сглаз на круг", "A Hex Around the Circle", kw="c")

c("vor_black_wedding", "S", "R", 2, "-", "Apply(Rot, N({0}), AllFoes), Apply(Hex, N({1}), AllFoes), Apply(Weak, N(2), AllFoes)", (4, 2), (6, 3), "Чёрная свадьба", "The Black Wedding", kw="x")
c("vor_soul_thread", "A", "R", 2, "E", "Damage(N({0}), Target), If(Fatal, [Apply(Rot, N({1}), AllFoes)])", (14, 4), (18, 6), "Нить души", "Soul Thread")
c("vor_carrion_feast", "P", "R", 2, "-", "Kill|Heal(N({0}), Me), Draw(N(1))", 3, 5, "Пир падальщиц", "Carrion Feast")
c("vor_thrice_cursed", "S", "R", 1, "E", "Apply(Rot, N({0}), Target), Apply(Hex, N({0}), Target), Apply(Weak, N({0}), Target)", 2, 3, "Трижды проклят", "Thrice Cursed")
c("vor_plague_mother", "P", "R", 3, "-", "TurnStart|Apply(Rot, N({0}), AllFoes)", 2, 3, "Мать мора", "Mother of Plagues")
c("vor_last_rites", "A", "R", 3, "E", "Damage(N({0}), Target), Damage(Curses(Target, 3), Target)", 12, 16, "Отпевание", "Last Rites")
c("vor_bargain", "S", "R", 0, "-", "LoseHp(N(6), Me), Energy(N(2), Me), Draw(N({0}))", 2, 3, "Сделка", "The Bargain", kw="x")
c("vor_wailing", "A", "R", 2, "-", "Hits(N({0}), 4, RandomFoe), Apply(Rot, N(1), AllFoes)", 4, 5, "Причитание", "Wailing")
c("vor_death_mask", "S", "R", 2, "-", "Block(N({0}), Me), Apply(Thorns, N({1}), Me), Apply(Fear, N(25), AllFoes)", (12, 3), (16, 4), "Посмертная маска", "Death Mask")
c("vor_ill_star", "P", "R", 2, "-", "AttackPlayed|OncePerTurn(\"ill_star\", [Apply(Hex, N({0}), AllFoes)])", 1, 2, "Недобрая звезда", "Ill Star")
c("vor_unmake", "A", "R", 1, "E", "ConsumeCurses(Target), Damage(Consumed({0}), AllFoes)", 2, 3, "Развеять", "Unmake", kw="x")
c("vor_witch_mark", "S", "R", 1, "E", "Apply(HuntMark, N({0}), Target), Apply(Hex, N(2), Target)", 4, 6, "Ведьмина метка", "Witch's Mark", kw="c")
c("vor_coven", "S", "R", 2, "-", "Apply(Rot, N({0}), AllFoes), Heal(N(3), AllFriends)", 4, 6, "Шабаш", "The Coven", kw="c")
c("vor_common_grief", "S", "R", 1, "-", "Cleanse(AllFriends), Apply(Weak, N({0}), AllFoes)", 1, 2, "Общее горе", "Common Grief", kw="cx")

# ======================= Волхв =======================
hero("volhv")
c("vol_flash", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Runes(Thunder, 2), Target)", 5, 7, "Вспышка", "Flash")
c("vol_fire_carving", "S", "C", 1, "H", "Rune(Flame, Target), Block(N({0}), Target)", 2, 5, "Огненный рез", "Fire Carving")
c("vol_fog_carving", "S", "C", 1, "H", "Rune(Mist, Target), Block(N({0}), Target)", 2, 5, "Резать по туману", "Carve the Fog")
c("vol_green_carving", "S", "C", 1, "H", "Rune(Zhiva, Target), Heal(N({0}), Target)", 1, 3, "Зелёный рез", "Green Carving")
c("vol_deep_cut", "A", "C", 1, "E", "Damage(N({0}), Target), Rune(Thunder, Me)", 3, 6, "Глубокий рез", "Deep Cut")
c("vol_drone", "S", "C", 1, "-", "Block(N({0}), Me), Draw(N(1))", 3, 6, "Гудение", "Drone")
c("vol_oaken_staff", "A", "C", 2, "E", "Damage(N({0}), Target), Block(N({1}), Me)", (8, 4), (11, 6), "Дубовый посох", "Oaken Staff")
c("vol_hail", "A", "C", 1, "-", "Damage(N({0}), AllFoes)", 4, 6, "Град", "Hail")
c("vol_rime", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Runes(Frost, 2), Target), Apply(Weak, N(1), Target)", 3, 5, "Иней", "Rime")
c("vol_ember", "A", "C", 0, "E", "Damage(N({0}), Target)", 3, 5, "Уголёк", "Ember")
c("vol_warding_circle", "S", "C", 1, "-", "Block(N({0}), Me), Block(Runes(Frost, 2), Me)", 4, 6, "Обережный круг", "Warding Circle")
c("vol_listen", "S", "C", 0, "-", "Draw(N({0}))", 1, 2, "Слушать ветер", "Listen to the Wind")
c("vol_bolt", "A", "C", 2, "E", "Damage(N({0}), Target)", 13, 18, "Удар молнии", "Lightning Strike")
c("vol_offering_smoke", "S", "C", 1, "-", "Apply(Mist, N(1), Me), Block(N({0}), Me)", 3, 5, "Жертвенный дым", "Offering Smoke")
c("vol_gust", "A", "C", 1, "E", "Damage(N({0}), Target), Draw(N(1))", 3, 6, "Порыв ветра", "Gust")
c("vol_bind", "S", "C", 1, "E", "Apply(Weak, N({0}), Target), Apply(Vulnerable, N(1), Target)", 1, 2, "Связать словом", "Bind with a Word")
c("vol_cinders", "A", "C", 1, "-", "Damage(N({0}), AllFoes), Damage(Runes(Flame, 1), AllFoes)", 3, 5, "Зола и искры", "Cinders")
c("vol_stone_idol", "S", "C", 2, "-", "Block(N({0}), Me), Rune(Frost, Me)", 8, 11, "Каменный идол", "Stone Idol")
c("vol_thunderclap", "A", "C", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Apply(Vulnerable, N(1), Target)])", 6, 9, "Раскат", "Thunderclap")
c("vol_gift_of_frost", "S", "C", 1, "H", "Block(N({0}), Target), If(Link(Skill), [Rune(Frost, Target)])", 5, 8, "Дар стужи", "Gift of Frost")
c("vol_warm_rain", "S", "C", 1, "-", "Heal(N({0}), AllFriends)", 2, 3, "Тёплый дождь", "Warm Rain")
c("vol_sacred_fire", "A", "C", 2, "-", "Damage(N({0}), AllFoes), Rune(Flame, Me)", 5, 8, "Священный огонь", "Sacred Fire")
c("vol_staff_sweep", "A", "C", 1, "E", "Hits(N({0}), 2, Target)", 3, 5, "Посохом наотмашь", "Staff Sweep")
c("vol_old_words", "S", "C", 1, "-", "Block(N({0}), AllFriends)", 3, 5, "Старые слова", "Old Words")
c("vol_omen_bird", "S", "C", 1, "-", "Draw(N({0}))", 2, 3, "Вещая птица", "Omen Bird")
c("vol_ash_mark", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Rot, N(2), Target)", 3, 5, "Пепельная метка", "Ash Mark")

c("vol_twin_carving", "S", "U", 2, "H", "Rune(Thunder, Target), Rune(Flame, Target)", 0, 0, "Двойной рез", "Twin Carving", upcost=1)
c("vol_chain_lightning", "A", "U", 2, "-", "Damage(N(3), AllFoes), Damage(Runes(Thunder, {0}), AllFoes)", 3, 4, "Цепная молния", "Chain Lightning")
c("vol_wall_of_ice", "S", "U", 2, "-", "Block(N({0}), AllFriends), Block(Runes(Frost, 2), AllFriends)", 5, 7, "Ледяная стена", "Wall of Ice")
c("vol_call_fire", "A", "U", 1, "-", "Damage(N(3), AllFoes), Damage(Runes(Flame, {0}), AllFoes)", 3, 5, "Воззвать к огню", "Call the Fire")
c("vol_offering", "S", "U", 1, "-", "Rune(Thunder, Me), Rune(Frost, Me)", 0, 0, "Подношение", "Offering", upcost=0)
c("vol_seer", "P", "U", 2, "-", "TurnStart|Draw(N(1))", 0, 0, "Провидец", "Seer", upcost=1)
c("vol_carved_ward", "P", "U", 1, "-", "TurnEnd|Block(Runes(Frost, {0}), Me)", 2, 3, "Резной оберег", "Carved Ward")
c("vol_sky_wrath", "A", "U", 2, "E", "Damage(N({0}), Target), Rune(Thunder, Me)", 9, 13, "Гнев небес", "Wrath of the Sky")
c("vol_fog_bank", "S", "U", 1, "-", "Apply(Mist, N(1), AllFriends), Block(N({0}), Me)", 2, 5, "Туманная гряда", "Fog Bank")
c("vol_word_and_deed", "S", "U", 2, "-", "Invoke, Draw(N({0}))", 1, 2, "Слово и дело", "Word and Deed")
c("vol_scorch", "A", "U", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N({1}), Target)", (5, 1), (7, 2), "Опалить", "Scorch")
c("vol_tree_of_life", "S", "U", 2, "H", "Rune(Zhiva, Target), Rune(Zhiva, Target), Heal(N({0}), Target)", 3, 6, "Древо жизни", "Tree of Life")
c("vol_cleansing_fire", "S", "U", 1, "H", "Cleanse(Target), Rune(Flame, Target)", 0, 0, "Очищающий огонь", "Cleansing Fire", upcost=0)
c("vol_burn_rot", "A", "U", 1, "E", "Consume(Rot, Target), Damage(N({0}), Target), Damage(Consumed(3), AllFoes)", 4, 6, "Выжечь гниль", "Burn Out the Rot")
c("vol_elder_song", "P", "U", 2, "-", "SkillPlayed|OncePerTurn(\"elder_song\", [Block(N({0}), AllFriends)])", 2, 3, "Песнь старцев", "Elder Song")
c("vol_sky_iron", "A", "U", 2, "E", "Hits(N({0}), 3, Target), Apply(Weak, N(1), Target)", 4, 5, "Небесное железо", "Sky Iron")
c("vol_dawn", "S", "U", 1, "-", "Energy(N({0}), Me), Draw(N(1))", 1, 2, "Рассвет", "Dawn", kw="x")
c("vol_sign_of_veles", "S", "U", 1, "-", "Apply(Fortitude, N({0}), Me), Rune(Frost, Me)", 1, 2, "Знак Велеса", "Sign of Veles")
c("vol_rune_for_each", "S", "U", 1, "-", "Rune(Frost, AllFriends)", 0, 0, "Реза на каждого", "A Rune for Each", kw="c", upcost=0)
c("vol_storm_brotherhood", "A", "U", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Damage(N({1}), Target), Rune(Thunder, Me)])", (6, 4), (8, 6), "Грозовое братство", "Storm Brotherhood", kw="c")

c("vol_sky_hammer", "A", "R", 3, "-", "Damage(N({0}), AllFoes), Damage(Runes(Thunder, 3), AllFoes)", 10, 14, "Небесный молот", "Sky Hammer")
c("vol_world_tree", "P", "R", 3, "-", "TurnStart|Rune(Zhiva, Me)", 0, 0, "Мировое древо", "The World Tree", upcost=2)
c("vol_svarog_forge", "P", "R", 2, "-", "TurnStart|Damage(Runes(Flame, {0}), AllFoes)", 2, 3, "Кузня Сварога", "Svarog's Forge")
c("vol_heart_of_winter", "S", "R", 2, "-", "Rune(Frost, AllFriends), Block(N({0}), AllFriends)", 4, 7, "Сердце зимы", "Heart of Winter")
c("vol_word_of_thunder", "A", "R", 2, "E", "Damage(N({0}), Target), Invoke", 10, 14, "Громовое слово", "Word of Thunder")
c("vol_five_runes", "S", "R", 3, "-", "Rune(Thunder, Me), Rune(Frost, Me), Rune(Mist, Me), Rune(Flame, Me), Rune(Zhiva, Me)", 0, 0, "Пять рез", "Five Runes", kw="x", upcost=2)
c("vol_prophecy", "S", "R", 1, "-", "Draw(N({0})), Energy(N(1), Me)", 2, 3, "Пророчество", "Prophecy", kw="x")
c("vol_sky_falls", "A", "R", 0, "E", "Repeat(X(1), [Damage(N({0}), Target), Rune(Thunder, Me)])", 5, 7, "Небо падает", "The Sky Falls", kw="X")
c("vol_ancient_oak", "P", "R", 2, "-", "Hurt|Rune(Frost, Me)", 0, 0, "Древний дуб", "The Ancient Oak", upcost=1)
c("vol_rain_of_fire", "A", "R", 2, "-", "Hits(N({0}), 3, RandomFoe), Rune(Flame, Me)", 5, 7, "Огненный дождь", "Rain of Fire")
c("vol_double_voice", "P", "R", 2, "-", "SkillPlayed|OncePerTurn(\"double_voice\", [Damage(N(2), RandomFoe), Damage(Runes(Thunder, {0}), RandomFoe)])", 3, 5, "Двойной голос", "Double Voice")
c("vol_blessing", "S", "R", 1, "A", "Rune(Thunder, Target), Rune(Frost, Target), Rune(Zhiva, Target)", 0, 0, "Благословение волхва", "The Volkhv's Blessing", kw="c", upcost=0)
c("vol_circle_of_stones", "S", "R", 2, "-", "Rune(Mist, AllFriends), Apply(Fortitude, N({0}), AllFriends)", 1, 2, "Круг камней", "Circle of Stones", kw="c")
c("vol_calling", "S", "R", 1, "-", "Invoke, Energy(N({0}), OtherFriends)", 1, 2, "Призыв", "The Calling", kw="cx")

# ======================= Волколак =======================
hero("volkolak")
c("wolf_gash", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Bleed, N(1), Target)", 5, 7, "Рваная рана", "Gash")
c("wolf_crouch", "S", "C", 1, "-", "Block(N({0}), Me), Draw(N(1))", 3, 6, "Припасть к земле", "Crouch", beast=("A", "E", "Damage(N({0}), Target), Draw(N(1))", 4, 7))
c("wolf_bite", "A", "C", 1, "E", "Damage(N({0}), Target)", 7, 10, "Укус", "Bite", beast=("A", "E", "Damage(N({0}), Target), Heal(N(2), Me)", 7, 10))
c("wolf_snap", "A", "C", 0, "E", "Damage(N({0}), Target)", 3, 5, "Щёлкнуть зубами", "Snap")
c("wolf_sheepskin", "S", "C", 2, "-", "Block(N({0}), Me)", 11, 15, "Кожух", "Sheepskin Coat", beast=("A", "-", "Damage(N({0}), AllFoes)", 7, 10))
c("wolf_stalk", "S", "C", 1, "E", "Apply(Vulnerable, N({0}), Target), Draw(N(1))", 1, 2, "Красться", "Stalk")
c("wolf_rend", "A", "C", 2, "E", "Damage(N({0}), Target), Apply(Bleed, N({1}), Target)", (9, 2), (12, 3), "Рвать", "Rend")
c("wolf_sniff", "S", "C", 0, "E", "Apply(HuntMark, N({0}), Target)", 2, 4, "Чуять", "Scent")
c("wolf_lope", "S", "C", 1, "-", "Block(N({0}), Me), Apply(TempStrength, N(1), Me)", 4, 6, "Волчья рысь", "Wolf's Lope", beast=("S", "-", "Apply(TempStrength, N({0}), Me), Draw(N(1))", 2, 3))
c("wolf_spring", "A", "C", 1, "E", "Damage(N({0}), Target), If(Form(Beast), [Apply(Weak, N(1), Target)], [Block(N(3), Me)])", 5, 8, "Прыжок из травы", "Spring from the Grass")
c("wolf_bared_teeth", "A", "C", 1, "-", "Damage(N({0}), AllFoes)", 4, 6, "Оскал", "Bared Teeth")
c("wolf_lick_wounds", "S", "C", 1, "-", "Heal(N({0}), Me)", 4, 6, "Зализать раны", "Lick Your Wounds")
c("wolf_throw_knife", "A", "C", 1, "E", "Damage(N({0}), Target), Draw(N(1))", 3, 6, "Метнуть нож", "Throw the Knife")
c("wolf_hackles", "S", "C", 1, "-", "Block(N({0}), Me), Apply(Thorns, N(1), Me)", 4, 6, "Шерсть дыбом", "Hackles Up", beast=("S", "-", "Apply(Thorns, N({0}), Me), Apply(TempStrength, N(1), Me)", 3, 4))
c("wolf_both_paws", "A", "C", 1, "E", "Hits(N({0}), 2, Target)", 3, 5, "Обе лапы", "Both Paws")
c("wolf_ambush", "A", "C", 2, "E", "Damage(N({0}), Target), If(Has(HuntMark, Target, 1), [Damage(N({1}), Target)])", (10, 5), (13, 8), "Засада", "Ambush")
c("wolf_moon_gaze", "S", "C", 1, "-", "Draw(N({0}))", 2, 3, "Взгляд на луну", "Gaze at the Moon")
c("wolf_tear_flesh", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(Stat(Bleed, Target, 1), Target)", 4, 6, "Рвать плоть", "Tear Flesh")
c("wolf_feint_step", "S", "C", 1, "-", "Block(N({0}), Me), If(Link(Attack), [Block(N(4), Me)])", 5, 7, "Обманный шаг", "Feint Step", beast=("A", "E", "Damage(N({0}), Target), If(Link(Attack), [Damage(N(4), Target)])", 6, 9))
c("wolf_hamstring", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N({1}), Target)", (4, 1), (5, 2), "Подрезать жилу", "Hamstring")
c("wolf_growl", "S", "C", 1, "-", "Apply(Fear, N({0}), AllFoes)", 20, 30, "Рык", "Growl")
c("wolf_pack_tactics", "A", "C", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Damage(N({1}), Target)])", (5, 4), (7, 6), "Стайная охота", "Pack Tactics")
c("wolf_behind_my_back", "S", "C", 1, "H", "Block(N({0}), Target)", 6, 9, "Спрятать за спиной", "Behind My Back")
c("wolf_mauling", "A", "C", 2, "E", "Hits(N({0}), 3, Target)", 4, 5, "Трёпка", "Mauling")
c("wolf_raw_meat", "S", "C", 1, "-", "Heal(N({0}), Me), Apply(TempStrength, N(1), Me)", 2, 4, "Сырое мясо", "Raw Meat")
c("wolf_run_down", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(HuntMark, N(2), Target)", 4, 6, "Загнать", "Run Down")

c("wolf_bloodlust", "P", "U", 1, "-", "Kill|Apply(Strength, N({0}), Me)", 1, 2, "Жажда крови", "Bloodlust")
c("wolf_open_vein", "A", "U", 1, "E", "Apply(Bleed, N({0}), Target), Damage(N(4), Target)", 3, 5, "Вскрыть жилу", "Open a Vein")
c("wolf_change_skins", "S", "U", 1, "-", "Shift, Block(N({0}), Me), Draw(N(1))", 4, 7, "Сменить шкуру", "Change Skins")
c("wolf_savagery", "A", "U", 2, "E", "Damage(N({0}), Target), If(Form(Beast), [Damage(N({1}), Target)])", (10, 6), (13, 9), "Свирепость", "Savagery")
c("wolf_moonlit", "S", "U", 1, "-", "Apply(TempStrength, N({0}), Me), Draw(N(1))", 2, 4, "Лунный свет", "Moonlit")
c("wolf_for_the_throat", "A", "U", 2, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(2), Target)", 10, 14, "За горло", "Throat Grip")
c("wolf_thick_hide", "P", "U", 1, "-", "TurnEnd|If(Form(Human), [Block(N({0}), Me)])", 3, 5, "Толстая шкура", "Thick Hide")
c("wolf_chase", "A", "U", 1, "E", "Damage(N({0}), Target), If(Fatal, [Energy(N(1), Me), Draw(N(1))])", 7, 10, "Погоня", "The Chase")
c("wolf_howl_at_moon", "S", "U", 1, "-", "Apply(Fear, N({0}), AllFoes), Apply(TempStrength, N(1), AllFriends)", 20, 30, "Вой на луну", "Howl at the Moon")
c("wolf_cornered", "S", "U", 1, "-", "Block(N({0}), Me), Apply(Thorns, N({1}), Me)", (6, 2), (8, 3), "Загнанный", "Cornered", beast=("A", "-", "Damage(N({0}), AllFoes), Apply(Thorns, N({1}), Me)", (5, 2), (7, 3)))
c("wolf_scent_of_blood", "P", "U", 1, "-", "AttackPlayed|OncePerTurn(\"scent_of_blood\", [Apply(Bleed, N({0}), RandomFoe)])", 1, 2, "Привкус крови", "Taste of Blood")
c("wolf_lunar_tide", "S", "U", 0, "-", "Shift, Apply(TempStrength, N({0}), Me)", 1, 2, "Лунный прилив", "Lunar Tide")
c("wolf_rip_apart", "A", "U", 1, "E", "Consume(Bleed, Target), Damage(N({0}), Target), Damage(Consumed(3), Target)", 5, 8, "Разорвать", "Rip Apart")
c("wolf_guard_the_pack", "S", "U", 1, "H", "Block(N({0}), Target), Apply(TempStrength, N(1), Target)", 5, 8, "Стеречь своих", "Guard the Pack")
c("wolf_gone_feral", "A", "U", 1, "-", "Damage(N({0}), RandomFoe), Damage(N({0}), RandomFoe), Damage(N({0}), RandomFoe)", 3, 4, "Дикая кровь", "Wild Blood")
c("wolf_patience", "S", "U", 1, "-", "Block(N({0}), Me), Draw(N(2))", 3, 5, "Терпение охотника", "Hunter's Patience", kw="r")
c("wolf_hexed_bite", "A", "U", 1, "E", "Consume(Hex, Target), Damage(N({0}), Target), Damage(Consumed(4), Target), Apply(Bleed, N(2), Target)", 5, 7, "Укус по сглазу", "Hexed Bite")
c("wolf_night_hunter", "P", "U", 2, "-", "TurnStart|Apply(HuntMark, N({0}), RandomFoe)", 2, 3, "Ночной охотник", "Night Hunter")
c("wolf_pack_leader", "S", "U", 1, "-", "Apply(TempStrength, N({0}), OtherFriends), Draw(N(1))", 2, 3, "Вожак", "Pack Leader", kw="c")
c("wolf_share_kill", "A", "U", 1, "E", "Damage(N({0}), Target), If(Fatal, [Heal(N({1}), AllFriends)])", (7, 4), (10, 6), "Делить добычу", "Share the Kill", kw="c")

c("wolf_alpha", "P", "R", 2, "-", "Shifted|OncePerTurn(\"alpha\", [Apply(Strength, N(1), Me)])", 0, 0, "Матёрый", "Alpha", upcost=1)
c("wolf_slaughter", "A", "R", 3, "-", "Hits(N({0}), 3, AllFoes)", 4, 5, "Резня", "Slaughter")
c("wolf_heart_eater", "A", "R", 2, "E", "Damage(N({0}), Target), If(Fatal, [MaxHp(N({1}), Me), Heal(N(6), Me)])", (14, 2), (18, 3), "Сердцеед", "Heart-Eater")
c("wolf_berserk", "P", "R", 2, "-", "Hurt|OncePerTurn(\"berserk\", [Apply(Strength, N(1), Me)])", 0, 0, "Бешенство", "Berserk", upcost=1)
c("wolf_true_form", "S", "R", 1, "-", "Shift, Energy(N({0}), Me), Draw(N(2))", 1, 2, "Истинный облик", "True Form", kw="x")
c("wolf_rain_of_blood", "A", "R", 2, "-", "Damage(N({0}), AllFoes), Apply(Bleed, N({1}), AllFoes)", (7, 2), (9, 3), "Кровавый дождь", "Rain of Blood")
c("wolf_predator", "P", "R", 3, "-", "AttackPlayed|Damage(N({0}), RandomFoe)", 2, 3, "Хищник", "Predator")
c("wolf_lone_wolf", "S", "R", 1, "-", "Block(N({0}), Me), If(Coop, [Apply(TempStrength, N(2), Me)], [Apply(Strength, N(1), Me)])", 8, 11, "Одинокий волк", "Lone Wolf")
c("wolf_tear_out", "A", "R", 1, "E", "Damage(N(4), Target), Damage(Stat(Bleed, Target, {0}), Target)", 3, 4, "Вырвать", "Tear Out", kw="x")
c("wolf_moon_madness", "S", "R", 0, "-", "Shift, Apply(TempStrength, N({0}), Me), LoseHp(N(3), Me)", 4, 6, "Лунное безумие", "Moon Madness", kw="x")
c("wolf_endless_hunt", "A", "R", 0, "E", "Repeat(X(1), [Damage(N({0}), Target), Apply(Bleed, N(1), Target)])", 6, 8, "Бесконечная охота", "The Endless Hunt", kw="X")
c("wolf_blood_brothers", "S", "R", 1, "A", "Apply(Strength, N({0}), Target), Apply(TempStrength, N(2), Me)", 1, 2, "Кровные братья", "Blood Brothers", kw="c")
c("wolf_the_drive", "S", "R", 1, "E", "Apply(HuntMark, N({0}), Target), Apply(Vulnerable, N(1), Target)", 5, 7, "Гон", "The Drive", kw="c")
c("wolf_chained_hound", "S", "R", 2, "A", "Block(N({0}), Target), Apply(Thorns, N({1}), Target)", (12, 3), (16, 4), "Цепной пёс", "Chained Hound", kw="c")

# ======================= Знахарка =======================
hero("znaharka")
c("zn_salve", "S", "C", 1, "H", "Heal(N({0}), Target)", 5, 8, "Мазь", "Salve")
c("zn_bandage", "S", "C", 1, "H", "Block(N({0}), Target), Heal(N(2), Target)", 3, 5, "Перевязка", "Bandage")
c("zn_notched_sickle", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Bleed, N(1), Target)", 5, 7, "Серп с зазубриной", "Notched Sickle")
c("herb_nettle", "H", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N(1), Target), Herb(\"wormwood\")", 2, 4, "Крапива", "Nettle")
c("herb_mint", "H", "C", 1, "-", "Draw(N(1)), Herb(\"sleep\")", 0, 0, "Мята", "Mint", upcost=0)
c("herb_plantain", "H", "C", 1, "H", "Heal(N({0}), Target), Herb(\"weep\")", 3, 5, "Подорожник", "Plantain")
c("herb_thistle", "H", "C", 1, "E", "Hits(N({0}), 2, Target), Herb(\"wormwood\")", 2, 3, "Чертополох", "Thistle")
c("herb_chamomile", "H", "C", 1, "-", "Block(N({0}), AllFriends), Herb(\"sleep\")", 2, 3, "Ромашка", "Chamomile")
c("herb_yarrow", "H", "C", 0, "-", "Block(N({0}), Me), Herb(\"weep\")", 2, 4, "Тысячелистник", "Yarrow")
c("zn_pestle_blow", "A", "C", 1, "E", "Damage(N({0}), Target), Block(N({1}), Me)", (4, 3), (6, 5), "Удар пестом", "Pestle Blow")
c("zn_smelling_salts", "S", "C", 0, "H", "Cleanse(Target), Block(N({0}), Target)", 2, 5, "Нюхательная соль", "Smelling Salts")
c("zn_apron", "S", "C", 1, "-", "Block(N({0}), Me), Draw(N(1))", 3, 6, "Передник с карманами", "Pocketed Apron")
c("zn_bitter_draught", "S", "C", 1, "H", "Apply(TempStrength, N({0}), Target), Heal(N(2), Target)", 2, 3, "Горькое питьё", "Bitter Draught")
c("zn_reaping", "A", "C", 2, "-", "Damage(N({0}), AllFoes)", 8, 11, "Жатва", "Reaping")
c("zn_stir", "S", "C", 1, "E", "Brew, Block(N({0}), Me)", 4, 7, "Помешать варево", "Stir the Pot")
c("zn_thorn_hedge", "S", "C", 1, "-", "Block(N({0}), Me), Apply(Thorns, N({1}), Me)", (4, 1), (5, 2), "Колючая изгородь", "Thorn Hedge")
c("zn_sleeping_dust", "A", "C", 1, "-", "Damage(N({0}), AllFoes), Apply(Weak, N(1), AllFoes)", 2, 4, "Сонная пыль", "Sleeping Dust")
c("zn_forage", "S", "C", 1, "-", "Hook(\"random_herb\"), Block(N({0}), Me)", 3, 6, "По росе", "Foraging at Dew")
c("zn_lancet", "A", "C", 1, "E", "Damage(N({0}), Target), If(Has(Bleed, Target, 1), [Heal(N({1}), Me)])", (6, 3), (8, 5), "Ланцет", "Lancet")
c("zn_warm_hands", "S", "C", 1, "-", "Heal(N({0}), AllFriends)", 2, 3, "Тёплые руки", "Warm Hands")
c("zn_switch_lash", "A", "C", 1, "E", "Hits(N({0}), 2, Target)", 3, 5, "Хлестнуть прутом", "Switch Lash")
c("zn_soothe", "S", "C", 1, "H", "Block(N({0}), Target), If(Link(Skill), [Heal(N(3), Target)])", 5, 8, "Утешить", "Soothe")
c("zn_acrid_smell", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target)", 4, 7, "Едкий запах", "Acrid Smell")
c("zn_night_vigil", "S", "C", 2, "-", "Block(N({0}), Me), Heal(N({1}), Me)", (8, 3), (11, 4), "Ночное бдение", "Night Vigil")
c("zn_sting", "A", "C", 0, "E", "Damage(N({0}), Target)", 3, 5, "Жало", "Sting")
c("zn_granny_recipe", "S", "C", 1, "-", "Draw(N({0}))", 2, 3, "Тетрадка с рецептами", "Recipe Notebook")

c("herb_henbane", "H", "U", 1, "E", "Apply(Rot, N({0}), Target), Herb(\"wormwood\")", 3, 5, "Белена", "Henbane")
c("herb_valerian", "H", "U", 1, "-", "Apply(Weak, N({0}), AllFoes), Herb(\"sleep\")", 1, 2, "Валериана", "Valerian")
c("herb_st_john", "H", "U", 1, "H", "Heal(N({0}), Target), Cleanse(Target), Herb(\"weep\")", 2, 4, "Зверобой", "St John's Wort")
c("herb_wolfsbane", "H", "U", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target), Herb(\"burst\")", 3, 5, "Борец-трава", "Wolfsbane")
c("zn_double_decoction", "S", "U", 2, "E", "Brew, Hook(\"random_herb\"), Hook(\"random_herb\")", 0, 0, "Двойной отвар", "Double Decoction", upcost=1)
c("zn_fumigate", "S", "U", 1, "-", "Apply(Rot, N({0}), AllFoes), Block(N(3), AllFriends)", 2, 3, "Окурить", "Fumigate")
c("zn_healer_oath", "P", "U", 1, "-", "TurnEnd|Heal(N({0}), AllFriends)", 1, 2, "Клятва лекаря", "Healer's Oath")
c("zn_bloodletting", "S", "U", 1, "H", "Cleanse(Target), Apply(Strength, N({0}), Target)", 1, 2, "Кровопускание", "Bloodletting", kw="x")
c("zn_bloodsucker", "A", "U", 1, "E", "Damage(N({0}), Target), Heal(N({1}), Me)", (5, 3), (7, 4), "Кровосос", "Bloodsucker")
c("zn_strong_brew", "S", "U", 1, "H", "Energy(N({0}), Target), Heal(N(2), Target)", 1, 2, "Крепкий отвар", "Strong Brew", kw="x")
c("zn_poisoned_sickle", "A", "U", 1, "E", "Damage(N({0}), Target), Apply(Rot, N({1}), Target)", (4, 3), (6, 4), "Отравленный серп", "Poisoned Sickle")
c("zn_light_hand", "P", "U", 1, "-", "SkillPlayed|OncePerTurn(\"light_hand\", [Heal(N({0}), Me)])", 2, 3, "Лёгкая рука", "A Light Hand")
c("zn_draw_the_curse", "A", "U", 1, "E", "ConsumeCurses(Target), Damage(N({0}), Target), Heal(Consumed(1), AllFriends)", 5, 8, "Вытянуть порчу", "Draw the Curse")
c("zn_bathhouse", "S", "U", 2, "-", "Heal(N({0}), AllFriends), Cleanse(AllFriends)", 4, 6, "Баня", "The Bathhouse")
c("zn_smoke_out", "A", "U", 2, "-", "Damage(N({0}), AllFoes), Apply(Vulnerable, N(1), AllFoes)", 6, 9, "Выкурить", "Smoke Them Out")
c("zn_herb_pouch", "P", "U", 2, "-", "Kill|Hook(\"random_herb\")", 0, 0, "Травяной кисет", "Herb Pouch", upcost=1)
c("zn_tend", "S", "U", 1, "A", "Heal(N({0}), Target), Apply(Fortitude, N(1), Target)", 6, 9, "Выхаживать", "Tend", kw="c")
c("zn_share_brew", "S", "U", 1, "-", "Heal(N({0}), OtherFriends), Block(N({0}), OtherFriends)", 4, 6, "Поделиться отваром", "Share the Brew", kw="c")

c("herb_mandrake", "H", "R", 2, "E", "Damage(N({0}), Target), Apply(Stun, N(1), Target), Herb(\"burst\")", 4, 8, "Мандрагора", "Mandrake", kw="x")
c("herb_adam_head", "H", "R", 1, "-", "Block(N({0}), AllFriends), Herb(\"fern\")", 3, 5, "Адамова голова", "Adam's Head")
c("zn_elixir", "S", "R", 2, "H", "Heal(N({0}), Target), Apply(Strength, N(1), Target), Apply(Fortitude, N(1), Target)", 8, 12, "Эликсир", "Elixir", kw="x")
c("zn_great_cauldron", "P", "R", 2, "-", "TurnEnd|Block(N({0}), AllFriends), Heal(N(1), AllFriends)", 2, 3, "Большой котёл", "The Great Cauldron")
c("zn_poison_mist", "A", "R", 2, "-", "Damage(N({0}), AllFoes), Apply(Rot, N({1}), AllFoes)", (5, 3), (7, 4), "Ядовитый туман", "Poison Mist")
c("zn_panacea", "S", "R", 1, "-", "Cleanse(AllFriends), Heal(N({0}), AllFriends)", 5, 8, "Панацея", "Panacea", kw="x")
c("zn_third_eye", "S", "R", 1, "-", "Draw(N({0})), Hook(\"random_herb\")", 2, 3, "Третий глаз", "The Third Eye")
c("zn_reaper", "A", "R", 2, "E", "Damage(N({0}), Target), If(Fatal, [Heal(N({1}), AllFriends)])", (14, 5), (18, 7), "Жница", "The Reaper")
c("zn_mother_earth", "P", "R", 3, "-", "TurnStart|Heal(N({0}), AllFriends)", 2, 3, "Мать-сыра земля", "Mother Damp Earth")
c("zn_boiling_over", "A", "R", 0, "E", "Repeat(X(1), [Damage(N({0}), Target), Heal(N(2), Me)])", 6, 8, "Через край", "Boiling Over", kw="X")
c("zn_life_for_life", "S", "R", 1, "H", "LoseHp(N(5), Me), Heal(N({0}), Target), Apply(Strength, N(1), Target)", 12, 16, "Жизнь за жизнь", "A Life for a Life", kw="x")
c("zn_guardian_herbs", "S", "R", 1, "-", "Block(N({0}), OtherFriends), Apply(Thorns, N(2), OtherFriends)", 7, 10, "Обережные травы", "Guardian Herbs", kw="c")
c("zn_funeral_table", "S", "R", 2, "-", "Heal(N({0}), AllFriends), Energy(N(1), OtherFriends)", 6, 9, "Поминальный стол", "The Funeral Table", kw="cx")

# ======================= Скоморох =======================
hero("skomoroh")
c("sk_slap", "A", "C", 1, "E", "Damage(N({0}), Target), If(Mask(Bear), [Damage(N({1}), Target)])", (5, 3), (7, 5), "Оплеуха", "Slap")
c("sk_cartwheel", "S", "C", 1, "-", "Block(N({0}), Me), If(Mask(Goat), [Draw(N(1))])", 5, 8, "Колесо", "Cartwheel")
c("sk_boo", "A", "C", 1, "E", "Damage(N({0}), Target), If(Mask(Mara), [Apply(Weak, N(2), Target)])", 5, 8, "Пугало", "Boo")
c("sk_juggle_knives", "A", "C", 1, "-", "Damage(N({0}), RandomFoe), Damage(N({0}), RandomFoe)", 3, 5, "Жонглировать ножами", "Juggle Knives")
c("sk_curtsy", "S", "C", 0, "-", "Block(N({0}), Me)", 3, 5, "Реверанс", "Curtsy")
c("sk_trip", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target)", 4, 7, "Подножка", "Trip")
c("sk_jingle", "S", "C", 1, "-", "Draw(N({0}))", 2, 3, "Бубенцы звенят", "Jingle")
c("sk_patter", "A", "C", 1, "E", "Damage(N({0}), Target), Damage(PlayedThisRound(1), Target)", 4, 6, "Прибаутка", "Patter")
c("sk_leg_up", "S", "C", 1, "H", "Block(N({0}), Target), Draw(N(1))", 3, 6, "Подсадить", "Leg Up")
c("sk_jeer", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N(1), Target)", 4, 6, "Насмешка", "Jeer")
c("sk_squib", "A", "C", 1, "-", "Damage(N({0}), AllFoes)", 4, 6, "Хлопушка", "Squib")
c("sk_quick_change", "S", "C", 1, "-", "Mask(Bear), Block(N({0}), Me)", 4, 7, "Быстрое переодевание", "Quick Change")
c("sk_goat_dance", "S", "C", 1, "-", "Mask(Goat), Block(N({0}), AllFriends)", 2, 4, "Козья пляска", "Goat Dance")
c("sk_mara_whisper", "S", "C", 1, "E", "Mask(Mara), Apply(Fear, N({0}), Target)", 30, 45, "Шёпот Мары", "Mara's Whisper")
c("sk_two_stomps", "A", "C", 1, "E", "Hits(N({0}), 2, Target)", 3, 5, "Два притопа", "Two Stomps")
c("sk_stilt_kick", "A", "C", 2, "E", "Damage(N({0}), Target)", 13, 18, "Удар с ходуль", "Stilt Kick")
c("sk_tongue_twister", "S", "C", 1, "-", "Block(N({0}), Me), Block(PlayedThisRound(1), Me)", 3, 5, "Скороговорка", "Tongue-Twister")
c("sk_coin_trick", "S", "C", 0, "-", "Draw(N({0}))", 1, 2, "Фокус с монетой", "Coin Trick")
c("sk_pie", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Fear, N(15), Target)", 5, 8, "Пирог в лицо", "Pie in the Face")
c("sk_somersault", "S", "C", 2, "-", "Block(N({0}), Me), Draw(N(1))", 8, 11, "Сальто", "Somersault")
c("sk_duet", "A", "C", 1, "E", "Damage(N({0}), Target), If(Link(Attack), [Draw(N(1))])", 6, 9, "Дуэт", "Duet")
c("sk_applause", "S", "C", 1, "-", "Heal(N({0}), Me), Draw(N(1))", 2, 4, "Аплодисменты", "Applause")
c("sk_wink", "S", "C", 1, "E", "Apply(Vulnerable, N({0}), Target), Apply(Weak, N(1), Target)", 1, 2, "Подмигнуть", "Wink")
c("sk_whirligig", "A", "C", 2, "-", "Damage(N({0}), AllFoes), Apply(Weak, N(1), AllFoes)", 6, 9, "Карусель", "Whirligig")
c("sk_rattle_blow", "A", "C", 1, "E", "Damage(N({0}), Target), Block(N({1}), Me)", (4, 3), (6, 5), "Трещоткой по лбу", "Rattle to the Brow")

c("sk_three_masks", "S", "U", 1, "-", "Mask(Bear), Mask(Goat), Mask(Mara), Draw(N({0}))", 1, 2, "Три личины", "Three Masks")
c("sk_knife_act", "A", "U", 1, "E", "Hits(N({0}), 3, Target)", 2, 3, "Номер с ножами", "The Knife Act")
c("sk_once_more", "S", "U", 1, "-", "Echo(Me), Draw(N({0}))", 1, 2, "Ещё разок", "Once More")
c("sk_heckle", "S", "U", 1, "-", "Apply(Weak, N(1), AllFoes), Apply(Fear, N({0}), AllFoes)", 15, 25, "Освистать", "Heckle")
c("sk_showman", "P", "U", 1, "-", "CardPlayed|OncePerTurn(\"showman\", [Block(N({0}), Me)])", 3, 4, "Балаганщик", "Showman")
c("sk_fire_breather", "A", "U", 2, "-", "Damage(N({0}), AllFoes), If(Mask(Bear), [Damage(N(4), AllFoes)])", 7, 10, "Глотатель огня", "Fire-Breather")
c("sk_puppeteer", "A", "U", 1, "E", "Damage(N({0}), Target), If(Mask(Mara), [Apply(Vulnerable, N(2), Target)])", 6, 9, "Петрушечник", "Puppeteer")
c("sk_finale", "A", "U", 2, "E", "Damage(N(4), Target), Damage(PlayedThisRound({0}), Target)", 2, 3, "Финал", "Finale")
c("sk_rope_walk", "S", "U", 1, "-", "Block(N({0}), Me), Apply(Mist, N(1), Me)", 5, 8, "По канату", "Rope Walk")
c("sk_crowd_pleaser", "P", "U", 2, "-", "MaskChanged|Block(N({0}), AllFriends)", 2, 3, "Любимец публики", "Crowd-Pleaser")
c("sk_bear_dance", "A", "U", 2, "E", "Mask(Bear), Damage(N({0}), Target)", 12, 16, "Медвежья пляска", "Bear Dance")
c("sk_sleight", "S", "U", 0, "-", "Draw(N({0}))", 2, 3, "Ловкость рук", "Sleight of Hand", kw="x")
c("sk_mimic_hex", "A", "U", 1, "E", "Consume(Hex, Target), Damage(N({0}), Target), Damage(Consumed(4), Target), Draw(N(1))", 4, 6, "Передразнить сглаз", "Mimic the Hex")
c("sk_dirge", "A", "U", 1, "E", "Consume(Rot, Target), Damage(N({0}), Target), Apply(Fear, Consumed(10), Target)", 6, 9, "Отходная", "Dirge")
c("sk_ovation", "S", "U", 2, "-", "Block(N({0}), AllFriends), Draw(N(1))", 6, 9, "Овация", "Standing Ovation")
c("sk_goat_tricks", "S", "U", 1, "-", "Mask(Goat), Draw(N({0}))", 2, 3, "Козьи проделки", "Goat Tricks")
c("sk_trapdoor", "S", "U", 2, "E", "Apply(Stun, N(1), Target)", 0, 0, "Люк в сцене", "Trapdoor", kw="x", upcost=1)
c("sk_patchwork", "P", "U", 1, "-", "TurnEnd|Block(PlayedThisRound({0}), Me)", 1, 2, "Лоскутное одеяло", "Patchwork")
c("sk_double_act", "S", "U", 1, "A", "Apply(TempStrength, N({0}), Target), Draw(N(1))", 3, 4, "Парный номер", "Double Act", kw="c")
c("sk_pass_the_hat", "S", "U", 0, "-", "Block(N({0}), OtherFriends), Draw(N(1))", 3, 5, "Шапка по кругу", "Pass the Hat", kw="c")

c("sk_showstopper", "A", "R", 3, "-", "Damage(N(6), AllFoes), Damage(PlayedThisRound({0}), AllFoes)", 2, 3, "Гвоздь программы", "Showstopper")
c("sk_puppet_box", "P", "R", 2, "-", "CardPlayed|Damage(N({0}), RandomFoe)", 1, 2, "Вертеп", "The Puppet Box")
c("sk_deadly_jest", "A", "R", 2, "E", "Damage(N({0}), Target), If(Fatal, [Echo(Me), Energy(N(1), Me)])", 13, 17, "Смертельная шутка", "A Deadly Jest")
c("sk_masquerade", "S", "R", 1, "-", "Mask(Bear), Apply(Strength, N({0}), Me), Apply(Fortitude, N({0}), Me)", 1, 2, "Маскарад", "Masquerade", kw="x")
c("sk_hall_of_mirrors", "S", "R", 2, "-", "Echo(Me), Echo(Me)", 0, 0, "Зеркальный зал", "Hall of Mirrors", upcost=1)
c("sk_fear_itself", "S", "R", 2, "-", "Mask(Mara), Apply(Fear, N({0}), AllFoes), Apply(Weak, N(2), AllFoes)", 40, 60, "Сам страх", "Fear Itself")
c("sk_fools_luck", "P", "R", 2, "-", "Hurt|OncePerTurn(\"fools_luck\", [Apply(Mist, N(1), Me), Draw(N({0}))])", 1, 2, "Шутовское счастье", "Fool's Luck")
c("sk_hurly_burly", "A", "R", 0, "-", "Repeat(X(1), [Damage(N({0}), RandomFoe), Block(N(3), Me)])", 6, 8, "Кутерьма", "Hurly-Burly", kw="X")
c("sk_last_laugh", "A", "R", 1, "E", "Damage(N({0}), Target), Apply(Fear, N(50), AllFoes)", 10, 15, "Последний смех", "The Last Laugh", kw="x")
c("sk_whole_show", "S", "R", 1, "-", "Apply(TempStrength, N({0}), AllFriends), Block(N(3), AllFriends)", 2, 3, "Всем балаганом", "The Whole Show", kw="c")
c("sk_swap_roles", "S", "R", 1, "A", "Energy(N({0}), Target), Cleanse(Target)", 1, 2, "Поменяться ролями", "Swap Roles", kw="cx")
c("sk_curtain_call", "S", "R", 2, "-", "Heal(N({0}), AllFriends), Echo(AllFriends)", 4, 6, "Выход на поклон", "Curtain Call", kw="cx")


# ======================= Нейтральные: лавка и знамения, доступны всем =======================
hero("")
c("n_torch", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Vulnerable, N(1), Target)", 3, 5, "Факел", "Torch")
c("n_rye_bread", "S", "C", 1, "-", "Heal(N({0}), Me)", 4, 6, "Ржаной хлеб", "Rye Bread")
c("n_axe", "A", "C", 1, "E", "Damage(N({0}), Target)", 7, 10, "Топор", "Axe")
c("n_cloak", "S", "C", 1, "-", "Block(N({0}), Me)", 6, 9, "Дорожный плащ", "Travelling Cloak")
c("n_sling", "A", "C", 0, "E", "Damage(N({0}), Target)", 3, 5, "Праща", "Sling")
c("n_halt", "S", "C", 1, "-", "Block(N({0}), Me), Draw(N(1))", 3, 6, "Привал", "Halt")
c("n_pinch_of_salt", "S", "C", 1, "E", "Apply(Weak, N({0}), Target), Apply(Vulnerable, N(1), Target)", 1, 2, "Щепоть соли", "Pinch of Salt")
c("n_pitchfork", "A", "C", 2, "E", "Hits(N({0}), 2, Target)", 6, 8, "Вилы", "Pitchfork")
c("n_campfire", "A", "C", 1, "-", "Damage(N({0}), AllFoes)", 4, 6, "Костерок", "Campfire")
c("n_prayer", "S", "C", 1, "-", "Block(N({0}), AllFriends)", 3, 5, "Молитва", "Prayer")
c("n_map_scrap", "S", "C", 0, "-", "Draw(N({0}))", 1, 2, "Обрывок карты", "Scrap of a Map")
c("n_cudgel", "A", "C", 1, "E", "Damage(N({0}), Target), Apply(Weak, N(1), Target)", 4, 6, "Дубина", "Cudgel")

c("n_holy_water", "S", "U", 1, "H", "Cleanse(Target), Heal(N({0}), Target)", 3, 6, "Святая вода", "Holy Water")
c("n_silver_knife", "A", "U", 1, "E", "Damage(N({0}), Target), If(Has(Hex, Target, 1), [Damage(N({1}), Target)])", (7, 5), (9, 7), "Серебряный нож", "Silver Knife")
c("n_horn_call", "S", "U", 1, "-", "Apply(TempStrength, N({0}), AllFriends)", 2, 3, "Зов рога", "Horn Call")
c("n_bear_trap", "S", "U", 1, "E", "Apply(Vulnerable, N(2), Target), Apply(Bleed, N({0}), Target)", 2, 4, "Капкан", "Bear Trap")
c("n_last_coin", "S", "U", 0, "-", "Energy(N({0}), Me)", 1, 2, "Последняя монета", "The Last Coin", kw="x")
c("n_wayside_cross", "S", "U", 2, "-", "Block(N({0}), Me), Apply(Fortitude, N(1), Me)", 9, 13, "Придорожный крест", "Wayside Cross")
c("n_fire_arrow", "A", "U", 2, "E", "Damage(N({0}), Target), Apply(Rot, N(3), Target)", 9, 13, "Горящая стрела", "Fire Arrow")
c("n_shared_flask", "S", "U", 1, "H", "Heal(N({0}), Target), Apply(TempStrength, N(1), Target)", 4, 7, "Общая фляга", "Shared Flask")
c("n_straw_man", "S", "U", 1, "-", "Apply(Fear, N({0}), AllFoes)", 25, 40, "Чучело", "Straw Man")
c("n_whetted_scythe", "A", "U", 2, "-", "Damage(N({0}), AllFoes), Apply(Bleed, N(1), AllFoes)", 7, 10, "Отбитая коса", "Whetted Scythe")
c("n_lookout", "S", "U", 1, "-", "Draw(N({0})), Block(N(3), Me)", 2, 3, "Дозорный", "Lookout")

c("n_pectoral_cross", "P", "R", 2, "-", "TurnStart|Block(N({0}), Me)", 3, 4, "Нательный крест", "Pectoral Cross")
c("n_firebird_ash", "S", "R", 1, "D", "Revive({0}, Target)", 30, 50, "Пепел Жар-птицы", "Firebird Ash", kw="cx")
c("n_sword_from_stone", "A", "R", 3, "E", "Damage(N({0}), Target)", 26, 34, "Меч из камня", "The Sword from the Stone")
c("n_feast", "S", "R", 2, "-", "Heal(N({0}), AllFriends), Draw(N(1))", 5, 8, "Пир", "Feast", kw="x")
c("n_alarm_bell", "S", "R", 2, "-", "Apply(Weak, N({0}), AllFoes), Apply(Vulnerable, N({0}), AllFoes)", 2, 3, "Набат", "Alarm Bell", kw="x")
c("n_wanderer", "P", "R", 1, "-", "TurnStart|Apply(TempStrength, N({0}), Me)", 1, 2, "Странник", "Wanderer")
c("n_old_oath", "S", "R", 1, "-", "Apply(Strength, N({0}), Me)", 2, 3, "Старая клятва", "Old Oath", kw="x")


def build():
    ron = ["// Третий набор карт: до 75 + 5 кооп-карт у каждого героя.\n"
           "// Файл собран скриптом tools/cards3.py — правьте таблицу там.\n["]
    ru, en = [], []
    seen = set()
    for hero_id, cards in CARDS.items():
        ron.append(f"    // ---------------- {hero_id or 'neutral'}: {len(cards)} ----------------")
        for cid, kind, rarity, cost, target, fx, base, up, nru, nen, kw, upcost, beast in cards:
            assert cid not in seen, cid
            seen.add(cid)
            head = [f'id: "{cid}"'] + ([f'hero: "{hero_id}"'] if hero_id else []) + [f"kind: {KIND[kind]}", f"rarity: {RARITY[rarity]}", f"cost: {cost}"]
            if "X" in kw:
                head.append("x_cost: true")
            if TARGET[target]:
                head.append(f"target: {TARGET[target]}")
            words = [KW[k] for k in kw if k in KW]
            if words:
                head.append(f"keywords: [{', '.join(words)}]")
            # a brew is aimed too: its recipe may strike the chosen enemy
            assert ("Target" in fx or "Brew" in fx) == (target != "-"), cid

            def body(vals):
                text = fx.format(*tup(vals))
                if kind == "P":
                    on, effects = text.split("|", 1)
                    return f"passives: [(on: {on}, effects: [{effects}])]"
                return f"effects: [{text}]"

            patch = []
            if upcost is not None:
                patch.append(f"cost: {upcost}")
            if up != base:
                patch.append(body(up))
            assert patch, cid
            sides = ""
            if beast:
                bk, bt, bfx, bbase, bup = beast
                assert ("Target" in bfx) == (bt != "-"), cid
                shape = ""
                if bk != kind or bt != target:
                    shape = f"kind: {KIND[bk]}, target: {TARGET[bt] or 'None'}, "
                sides = "".join(f"\n        {name}: ({shape}effects: [{bfx.format(*tup(v))}])," for name, v in (("beast", bbase), ("beast_up", bup)))
            ron.append(f"    ({', '.join(head)},\n        {body(base)},\n        up: ({', '.join(patch)}),{sides}\n    ),")
            ru.append(f"card-{cid} = {nru}")
            en.append(f"card-{cid} = {nen}")
    ron.append("]\n")
    open(os.path.join(ROOT, "data", "cards", "set3.ron"), "w", encoding="utf-8", newline="\n").write("\n".join(ron))
    for lang, lines in (("ru", ru), ("en", en)):
        open(os.path.join(ROOT, "i18n", lang, "cards3.ftl"), "w", encoding="utf-8", newline="\n").write("\n".join(lines) + "\n")
    # names must not clash with cards written by hand
    for path in ("bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh", "set2", "curses", "special"):
        text = open(os.path.join(ROOT, "data", "cards", path + ".ron"), encoding="utf-8").read()
        clash = seen & set(re.findall(r'id: "([a-z0-9_]+)"', text))
        assert not clash, clash
    for hero_id, cards in CARDS.items():
        print(hero_id, len(cards), {r: sum(1 for x in cards if x[2] == r) for r in "CUR"}, "coop", sum(1 for x in cards if "c" in x[10]), "free", sum(1 for x in cards if x[3] == 0 and "X" not in x[10]))


if __name__ == "__main__":
    build()
