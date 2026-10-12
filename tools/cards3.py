"""The third card set: fills every hero up to 75 cards + 5 co-op ones.

The tables below are the source; this script writes
assets/data/cards/set3.ron and assets/i18n/{ru,en}/cards3.ftl from them.
Card text is not written here: the game composes it from the effects.

    python tools/cards3.py

A row is
    c(id, kind, rarity, cost, target, effects, base, up, ru, en, kw="", upcost=None)
kind:    A attack, S skill, P power (effects are then "Trigger|effects")
rarity:  C common, U uncommon, R rare
target:  E enemy, H any hero, A ally, D downed, - none
effects: RON with {0} {1}.. for the numbers; `base` and `up` fill them in
kw:      letters: x exhaust, r retain, e ethereal, c co-op, X x-cost
upcost:  the upgrade changes the cost instead of (or as well as) the numbers

Balance (design doc, section 4): 1 energy = 6 damage to one, 5 block, 4 damage to all,
2 stacks of a debuff; a card drawn = 0.5; uncommon +15%, rare +30% or a unique effect;
upgrade = +30-50% or -1 cost.
"""
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
KIND = {"A": "Attack", "S": "Skill", "P": "Power"}
RARITY = {"C": "Common", "U": "Uncommon", "R": "Rare"}
TARGET = {"E": "Enemy", "H": "AnyHero", "A": "Ally", "D": "Downed", "-": None}
KW = {"x": "Exhaust", "r": "Retain", "e": "Ethereal", "c": "Coop"}

CARDS = {}
_hero = None


def hero(name):
    global _hero
    _hero = name
    CARDS[name] = []


def c(cid, kind, rarity, cost, target, fx, base, up, ru, en, kw="", upcost=None):
    CARDS[_hero].append((cid, kind, rarity, cost, target, fx, base, up, ru, en, kw, upcost))


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


def build():
    ron = ["// Третий набор карт: до 75 + 5 кооп-карт у каждого героя.\n"
           "// Файл собран скриптом tools/cards3.py — правьте таблицу там.\n["]
    ru, en = [], []
    seen = set()
    for hero_id, cards in CARDS.items():
        ron.append(f"    // ---------------- {hero_id}: {len(cards)} ----------------")
        for cid, kind, rarity, cost, target, fx, base, up, nru, nen, kw, upcost in cards:
            assert cid not in seen, cid
            seen.add(cid)
            head = [f'id: "{cid}"', f'hero: "{hero_id}"', f"kind: {KIND[kind]}", f"rarity: {RARITY[rarity]}", f"cost: {cost}"]
            if "X" in kw:
                head.append("x_cost: true")
            if TARGET[target]:
                head.append(f"target: {TARGET[target]}")
            words = [KW[k] for k in kw if k in KW]
            if words:
                head.append(f"keywords: [{', '.join(words)}]")
            assert ("Target" in fx) == (target != "-"), cid

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
            ron.append(f"    ({', '.join(head)},\n        {body(base)},\n        up: ({', '.join(patch)})),")
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
