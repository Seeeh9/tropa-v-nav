"""The fourth card set: ten cards a hero, opened by hero levels 3 to 10.

They bring new ways to build a deck: throwing cards away on purpose (and cards that
pay off when thrown), burning them, calling them back from the discard pile, and
payoffs for each hero's own mechanic.

    python -X utf8 tools/cards4.py

writes assets/data/cards/set4.ron and assets/i18n/{ru,en}/cards4.ftl. A row is
    c(id, kind, rarity, cost, target, effects, base, up, ru, en, level,
      kw="", upcost=None, disc=None, upfx=None)
as in tools/cards3.py, plus
level:  hero level that opens the card in rewards
disc:   what the card does when a card or a power discards it ({0}.. as in effects)
upfx:   the upgrade's effects when they differ in shape, not only in numbers
"""
import os
import re

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets")
KIND = {"A": "Attack", "S": "Skill", "P": "Power", "H": "Herb"}
RARITY = {"C": "Common", "U": "Uncommon", "R": "Rare"}
TARGET = {"E": "Enemy", "H": "AnyHero", "A": "Ally", "-": None}
KW = {"x": "Exhaust", "r": "Retain", "e": "Ethereal", "c": "Coop"}

CARDS = {}
_hero = None


def hero(name):
    global _hero
    _hero = name
    CARDS[name] = []


def c(cid, kind, rarity, cost, target, fx, base, up, ru, en, level, kw="", upcost=None, disc=None, upfx=None):
    CARDS[_hero].append(dict(cid=cid, kind=kind, rarity=rarity, cost=cost, target=target, fx=fx, base=base, up=up,
                             ru=ru, en=en, level=level, kw=kw, upcost=upcost, disc=disc, upfx=upfx))


def tup(v):
    return v if isinstance(v, tuple) else (v,)


# ======================= Богатырь: удаль от сброса, щит за всех =======================
hero("bogatyr")
c("bog_spare_cry", "S", "C", 1, "-", "Block(N({0}), Me)", 6, 9, "Запасной клич", "A Spare War Cry", 3,
  disc="Apply(Valor, N(2), Me)")
c("bog_drop_guard", "A", "C", 1, "E", "Discard(N(1)), Damage(N({0}), Target)", 9, 12, "Бросить щит", "Drop the Guard", 3)
c("bog_reckless", "A", "U", 0, "E", "DiscardHand, Damage(Discarded({0}), Target)", 4, 5, "Очертя голову", "Headlong", 5)
c("bog_shield_wall", "P", "U", 1, "-", "Discarded|Block(N({0}), AllFriends)", 2, 3, "Щит на щит", "Shield upon Shield", 5)
c("bog_unbending", "P", "U", 2, "-", "TurnStart|Consume(Valor, Me), Block(Consumed({0}), Me)", 2, 3, "Кремень", "Flint", 6)
c("bog_valor_burst", "A", "U", 2, "E", "Consume(Valor, Me), Damage(Consumed({0}), Target)", 3, 4, "Выплеснуть удаль", "Pour Out the Valour", 7)
c("bog_stand_for_all", "S", "R", 1, "-", "Block(HandSize({0}), AllFriends)", 2, 3, "Стоять за всех", "Stand for All", 8)
c("bog_hold_to_the_end", "A", "R", 3, "E", "Damage(MissingHp({0}), Target)", 60, 80, "Стоять насмерть", "To the Death", 9)
c("bog_battle_fury", "P", "R", 2, "-", "Discarded|Apply(Valor, N(1), Me)", 0, 0, "Ратная злость", "Battle Fury", 10, upcost=1)
c("bog_toss_aside", "S", "R", 1, "-", "DiscardHand, Block(Discarded({0}), AllFriends)", 3, 4, "Всё в сторону", "Toss It All Aside", 10)

# ======================= Ворожея: лечение порчей, порча от сброса =======================
hero("vorozheya")
c("vor_drain_hex", "S", "C", 1, "E", "Consume(Hex, Target), Heal(Consumed({0}), AllFriends)", 1, 1, "Испить порчу", "Drink the Hex", 3, upcost=0)
c("vor_healing_word", "S", "C", 1, "H", "Heal(N({0}), Target), Apply(Hex, N({1}), RandomFoe)", (5, 2), (7, 3), "Заговор от хвори", "Charm Against Sickness", 3)
c("vor_mending_doll", "P", "U", 1, "-", "TurnEnd|Heal(N({0}), AllFriends)", 1, 2, "Кукла-лекарка", "The Healing Doll", 5)
c("vor_ill_omen", "S", "U", 1, "-", "Apply(Hex, N({0}), AllFoes)", 2, 3, "Дурная примета", "An Ill Omen", 5,
  disc="Apply(Hex, N({0}), AllFoes)")
c("vor_cast_lots", "S", "U", 0, "-", "Discard(N(1)), Apply(Hex, N({0}), AllFoes)", 1, 2, "Метать кости", "Cast the Bones", 6)
c("vor_blood_for_blood", "A", "U", 1, "E", "Damage(N({0}), Target), Heal(Curses(Target, 1), Me)", 6, 9, "Кровь за кровь", "Blood for Blood", 7)
c("vor_scatter_curse", "P", "R", 1, "-", "Discarded|Apply(Hex, N({0}), RandomFoe)", 2, 3, "Рассыпать порчу", "Scatter the Curse", 8)
c("vor_life_thread", "S", "R", 2, "H", "Heal(N({0}), Target), Cleanse(Target)", 12, 16, "Нить жизни", "The Thread of Life", 9, kw="x")
c("vor_black_mass", "S", "R", 1, "-", "DiscardHand, Apply(Hex, Discarded({0}), AllFoes)", 2, 3, "Чёрная треба", "The Black Rite", 10)
c("vor_little_mirror", "S", "R", 1, "H", "Block(N({0}), Target), Heal(N({1}), Target)", (6, 3), (8, 5), "Зеркальце", "The Little Mirror", 10,
  disc="Heal(N({1}), AllFriends)")

# ======================= Волхв: резы из сброса, огонь сожжённого =======================
hero("volhv")
c("vol_birch_bark", "S", "C", 0, "-", "Draw(N({0})), Discard(N({0}))", 1, 2, "Береста", "Birch Bark", 3,
  disc="Rune(Thunder, Me)")
c("vol_scratched_rune", "S", "C", 1, "-", "Rune(Frost, Me)", 0, 0, "Нацарапанная реза", "A Scratched Rune", 3, upcost=0,
  disc="Rune(Frost, Me)")
c("vol_leaf_storm", "A", "U", 1, "-", "Discard(N(1)), Damage(DiscardedThisTurn({0}), AllFoes)", 4, 6, "Листобой", "Leaf Storm", 5)
c("vol_reread", "S", "U", 1, "-", "Recall(N({0})), Draw(N(1))", 1, 2, "Перечесть", "Read It Again", 5)
c("vol_rune_ash", "P", "U", 1, "-", "Discarded|OncePerTurn(\"rune_ash\", [Rune(Flame, Me)])", 0, 0, "Пепел рез", "Ash of Runes", 6, upcost=0)
c("vol_burn_scroll", "S", "U", 0, "-", "ExhaustCards(N(1)), Invoke", 0, 0, "Сжечь свиток", "Burn the Scroll", 7,
  upfx="ExhaustCards(N(1)), Invoke, Draw(N(1))")
c("vol_ember_wisdom", "P", "R", 2, "-", "Exhausted|Rune(Thunder, Me)", 0, 0, "Мудрость углей", "Wisdom of Embers", 8, upcost=1)
c("vol_dove_book", "S", "R", 2, "-", "DiscardHand, Draw(Discarded(1)), Energy(N(1), Me)", 0, 0, "Голубиная книга", "The Dove Book", 9, kw="x", upcost=1)
c("vol_seven_runes", "S", "R", 3, "-", "Rune(Thunder, Me), Rune(Frost, Me), Rune(Flame, Me)", 0, 0, "Три резы разом", "Three Runes at Once", 10, kw="x", upcost=2)
c("vol_wise_fire", "A", "R", 1, "-", "Damage(ExhaustPile({0}), AllFoes)", 3, 4, "Мудрый огонь", "The Wise Fire", 10)

# ======================= Волколак: кровь и шерсть =======================
hero("volkolak")
c("vk_shed_fur", "S", "C", 0, "-", "Discard(N(1)), Apply(TempStrength, N({0}), Me)", 2, 3, "Сбросить шерсть", "Shed the Fur", 3)
c("vk_moon_call", "S", "C", 1, "-", "Apply(Weak, N({0}), AllFoes)", 1, 2, "Лунный зов", "Call of the Moon", 3,
  disc="Shift")
c("vk_frenzy", "A", "U", 1, "E", "Discard(N(1)), Hits(N({0}), 2, Target)", 5, 7, "Остервенение", "Savagery", 5)
c("vk_tufts", "P", "U", 1, "-", "Discarded|Damage(N({0}), RandomFoe)", 3, 4, "Клочья шерсти", "Tufts of Fur", 5)
c("vk_blood_scent", "A", "U", 1, "E", "Damage(N({0}), Target), If(Has(Bleed, Target, 1), [Draw(N(1)), Energy(N(1), Me)])", 6, 9, "Чуять кровь", "Smell the Blood", 6)
c("vk_feral_toss", "A", "U", 2, "-", "DiscardHand, Repeat(Discarded(1), [Damage(N({0}), RandomFoe)])", 5, 7, "Звериный разгул", "Feral Rampage", 7)
c("vk_maul", "A", "R", 2, "E", "Damage(N({0}), Target), Apply(Bleed, N({1}), Target)", (10, 4), (14, 5), "Рвать в клочья", "Tear to Shreds", 8)
c("vk_hunting_moon", "P", "R", 2, "-", "TurnStart|Apply(Bleed, N(1), AllFoes)", 0, 0, "Охотничья луна", "The Hunting Moon", 9, upcost=1)
c("vk_bloodbath", "A", "R", 1, "-", "Consume(Bleed, AllFoes), Damage(Consumed({0}), AllFoes)", 2, 3, "Кровавый пир", "Bloodbath", 10, kw="x")
c("vk_two_natures", "S", "R", 1, "-", "If(Form(Beast), [Draw(N({0}))], [Block(N({1}), Me)]), Shift", (3, 10), (4, 14), "Две природы", "Two Natures", 10)

# ======================= Знахарка: новые травы, котёл из сброса =======================
hero("znaharka")
c("zn_spilled_pot", "S", "C", 0, "-", "Discard(N(1)), Draw(N(1)), Herb(\"wormwood\")", 0, 0, "Опрокинутый горшок", "The Spilled Pot", 3,
  upfx="Discard(N(1)), Draw(N(1)), Herb(\"fern\")")
c("herb_dried_bundle", "H", "C", 1, "-", "Block(N({0}), Me), Herb(\"nettle\")", 4, 6, "Сухой пучок", "A Dried Bundle", 3,
  disc="Herb(\"nettle\")")
c("herb_poultice", "H", "U", 1, "H", "Heal(N({0}), Target), Herb(\"st_john\")", 5, 8, "Примочка", "A Compress", 5)
c("zn_slow_simmer", "P", "U", 1, "-", "Discarded|Herb(\"weep\")", 0, 0, "Томление", "Slow Simmer", 5, upcost=0)
c("zn_bitter_brew", "S", "U", 1, "E", "Apply(Weak, N({0}), Target), Brew", 1, 2, "Полынный настой", "Wormwood Tincture", 6)
c("herb_henbane_smoke", "H", "U", 1, "-", "Apply(Fear, N({0}), AllFoes), Herb(\"henbane\")", 10, 15, "Беленный дым", "Henbane Smoke", 7)
c("zn_wise_woman", "P", "R", 2, "-", "TurnStart|Herb(\"valerian\")", 0, 0, "Ведунья", "The Wise Woman", 8, upcost=1)
c("zn_boiling_cauldron", "S", "R", 2, "E", "Herb(\"fern\"), Brew", 0, 0, "Кипящий котёл", "The Boiling Cauldron", 9, kw="x", upcost=1)
c("zn_herb_garden", "S", "R", 1, "-", "Recall(N({0})), Herb(\"st_john\")", 2, 3, "Огород", "The Herb Garden", 10)
c("zn_nettle_lash", "A", "R", 1, "E", "Damage(N({0}), Target), Apply(Bleed, N({1}), Target), Herb(\"nettle\")", (6, 3), (9, 4), "Крапивный хлыст", "Nettle Lash", 10)

# ======================= Скоморох: фокусы с рукой, личины =======================
hero("skomoroh")
c("sk_toss_and_catch", "S", "C", 1, "-", "Discard(N(1)), Draw(N({0}))", 2, 3, "Подбросить и поймать", "Toss and Catch", 3)
c("sk_ace_up_sleeve", "S", "C", 1, "-", "Block(N({0}), Me)", 5, 8, "Козырь в рукаве", "An Ace up the Sleeve", 3,
  disc="Energy(N(1), Me)")
c("sk_jest_patter", "A", "U", 1, "E", "Damage(N({0}), Target), Damage(DiscardedThisTurn({1}), Target)", (4, 3), (6, 4), "Балагурство", "Banter", 5)
c("sk_quick_hands", "P", "U", 1, "-", "Discarded|Block(N({0}), Me)", 3, 4, "Проворные пальцы", "Nimble Fingers", 5)
c("sk_reshuffle", "S", "U", 1, "-", "DiscardHand, Draw(Discarded(1))", 0, 0, "Перетасовка", "Reshuffle", 6, upcost=0)
c("sk_bear_romp", "A", "U", 2, "E", "If(Mask(Bear), [Damage(N({0}), Target)], [Damage(N({1}), Target)])", (14, 8), (18, 11), "Медвежья возня", "Bear Romp", 7)
c("sk_laugh_and_fear", "P", "R", 2, "-", "Discarded|Apply(Fear, N({0}), RandomFoe)", 5, 8, "Смех сквозь страх", "Laughter Through Fear", 8)
c("sk_mara_grin", "A", "R", 1, "E", "Damage(N({0}), Target), If(Mask(Mara), [Apply(Fear, N({1}), Target)])", (6, 20), (9, 30), "Оскал Мары", "Mara's Grin", 9)
c("sk_last_bow", "A", "R", 2, "-", "DiscardHand, Damage(Discarded({0}), AllFoes)", 5, 7, "Последний выход", "The Grand Finale", 10, kw="x")
c("sk_trick_box", "S", "R", 1, "-", "Recall(N({0}))", 2, 3, "Короб с фокусами", "A Box of Tricks", 10,
  disc="Recall(N(1))")

# ======================= Емеля: дрёма из сброса =======================
hero("emelya")
c("em_kick_blanket", "S", "C", 0, "-", "Discard(N(1)), Apply(Drowse, N({0}), Me)", 1, 2, "Скинуть одеяло", "Kick off the Blanket", 3)
c("em_pillow", "S", "C", 1, "-", "Block(N({0}), Me)", 6, 9, "Подушка", "The Pillow", 3,
  disc="Apply(Drowse, N(2), Me)")
c("em_lazy_toss", "A", "U", 1, "-", "Discard(N(2)), Damage(Discarded({0}), RandomFoe)", 6, 8, "Лень вперёд родилась", "Sloth Was Born First", 5)
c("em_stove_warmth", "P", "U", 1, "-", "Discarded|Apply(Drowse, N(1), Me)", 0, 0, "Тепло печи", "The Stove's Warmth", 5, upcost=0)
c("em_wish_list", "S", "U", 1, "-", "Recall(N({0})), Draw(N(1))", 1, 2, "Хотелки", "A List of Wishes", 6)
c("em_dream_wave", "A", "U", 2, "-", "Consume(Drowse, Me), Damage(Consumed({0}), AllFoes)", 2, 3, "Сонная волна", "A Wave of Sleep", 7)
c("em_pike_favour", "P", "R", 2, "-", "TurnStart|Draw(N({0})), Discard(N(1))", 1, 2, "Щучий подарок", "The Pike's Gift", 8)
c("em_feather_bed", "S", "R", 1, "-", "Block(HandSize({0}), Me)", 3, 4, "Перина", "The Feather Bed", 9)
c("em_on_my_side", "S", "R", 2, "-", "DiscardHand, Heal(Discarded({0}), Me), Draw(N(3))", 1, 2, "Лёжа на боку", "Lying on My Side", 10, kw="x")
c("em_golden_dream", "A", "R", 1, "-", "Damage(Stat(Drowse, Me, {0}), RandomFoe)", 3, 4, "Сон о золоте", "A Golden Dream", 10)


def build():
    ron = ["// Четвёртый набор: по десять карт на героя, открываются уровнями героя 3..10.\n"
           "// Файл собран скриптом tools/cards4.py — правьте таблицу там.\n["]
    ru, en = [], []
    seen = set()
    for hero_id, cards in CARDS.items():
        ron.append(f"    // ---------------- {hero_id}: {len(cards)} ----------------")
        for k in cards:
            cid, kind, target, kw = k["cid"], k["kind"], k["target"], k["kw"]
            assert cid not in seen, cid
            seen.add(cid)
            head = [f'id: "{cid}"', f'hero: "{hero_id}"', f"kind: {KIND[kind]}", f"rarity: {RARITY[k['rarity']]}",
                    f"cost: {k['cost']}", f"level: {k['level']}"]
            if TARGET[target]:
                head.append(f"target: {TARGET[target]}")
            words = [KW[x] for x in kw if x in KW]
            if words:
                head.append(f"keywords: [{', '.join(words)}]")
            fx = k["fx"]
            assert ("Target" in fx or "Brew" in fx) == (target != "-"), cid
            if k["disc"]:
                assert "Target" not in k["disc"], cid

            def body(shape, vals):
                text = shape.format(*tup(vals))
                if kind == "P":
                    on, effects = text.split("|", 1)
                    return f"passives: [(on: {on}, effects: [{effects}])]"
                return f"effects: [{text}]"

            def disc(vals):
                return f", on_discard: [{k['disc'].format(*tup(vals))}]" if k["disc"] else ""

            patch = []
            if k["upcost"] is not None:
                patch.append(f"cost: {k['upcost']}")
            if k["upfx"]:
                patch.append(body(k["upfx"], k["up"]))
            elif k["up"] != k["base"]:
                patch.append(body(fx, k["up"]))
                if k["disc"] and k["disc"].format(*tup(k["up"])) != k["disc"].format(*tup(k["base"])):
                    patch.append(f"on_discard: [{k['disc'].format(*tup(k['up']))}]")
            assert patch, cid
            ron.append(f"    ({', '.join(head)},\n        {body(fx, k['base'])}{disc(k['base'])},\n        up: ({', '.join(patch)}),\n    ),")
            ru.append(f"card-{cid} = {k['ru']}")
            en.append(f"card-{cid} = {k['en']}")
    ron.append("]\n")
    open(os.path.join(ROOT, "data", "cards", "set4.ron"), "w", encoding="utf-8", newline="\n").write("\n".join(ron))
    for lang, lines in (("ru", ru), ("en", en)):
        open(os.path.join(ROOT, "i18n", lang, "cards4.ftl"), "w", encoding="utf-8", newline="\n").write("\n".join(lines) + "\n")
    for path in os.listdir(os.path.join(ROOT, "data", "cards")):
        if path == "set4.ron":
            continue
        text = open(os.path.join(ROOT, "data", "cards", path), encoding="utf-8").read()
        clash = seen & set(re.findall(r'id: "([a-z0-9_]+)"', text))
        assert not clash, clash
    for hero_id, cards in CARDS.items():
        for k in cards:
            free = k["cost"] == 0 or k["upcost"] == 0
            assert not (free and re.search(r"Draw\(N\(([2-9])\)\), Discard\(N\(1\)\)|Discard\(N\(1\)\), Draw\(N\(([2-9])", k["fx"].format(*tup(k["up"])))), k["cid"]
        print(hero_id, len(cards), sorted(k["level"] for k in cards))


if __name__ == "__main__":
    build()
