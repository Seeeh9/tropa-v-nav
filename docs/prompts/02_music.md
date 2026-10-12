# Музыка и звуки — промпты

Игра берёт файлы из `assets/music/` и `assets/sfx/`. Сгенерированный файл кладётся как `<имя>-flow.ogg` и вытесняет синтезированную заглушку. Нет файла — тишина, игра не ломается.

Заглушки пишет `python tools/gen_music.py`: зацикленные `.wav` на лейттеме (гусли, гудок, бубен, хор, колокол).
Вне боя играют одиночные петли: `music/menu`, `village`, `map`, `rest`, `shop`, `omen`, `end` —
генерировать их тем же хвостом, 2:00–3:00, без ударных кроме `map` и `shop`.

## Общий хвост для музыки

```text
Dark folk instrumental for a Slavic folk horror game. Minor modal key, live acoustic recording feel, raw and ritualistic, no synth pads, no EDM, no trailer-style epic percussion, no vocals with lyrics (wordless choir and lament allowed). Seamless loop. Shares a slow five-note leitmotif: A – C – D – E – C.
```

Лейттема (5 нот) вписана в хвост, чтобы все треки звучали как одна игра.

## Бой: три слоя

Игра накладывает три файла одного трека и меняет только их громкость:

- `_l1` — ритм и бас, играет всегда;
- `_l2` — мелодия, входит, когда враги собираются атаковать;
- `_l3` — хор и тяжёлые барабаны, входит на боссе, при падении союзника или HP отряда ниже 30%.

Слои должны быть одной длины, одного темпа и тональности. Генератор музыки не выдаёт согласованные стемы, поэтому порядок такой: сгенерировать **один** полный трек, а слои получить из него разделением на стемы (ударные+бас / мелодия / хор). Если стемы не сводятся — положить полный трек как `_l1`, слои 2 и 3 не класть: будет просто музыка без адаптивности.

| файл (без `_lN-flow.ogg`) | длина | BPM | промпт |
| --- | --- | --- | --- |
| `music/battle_act1` | 2:30 | 100 | Battle in a black spruce forest: ritual frame drums, bowed gudok fiddle, growling jaw harp (vargan). |
| `music/battle_act2` | 2:30 | 96 | Battle in a dead village and swamp: nasal wooden horn (rozhok), dull heavy thuds, whispering voices. |
| `music/battle_act3` | 2:30 | 108 | Battle in the land of the dead: low throat singing, icy plucked gusli, distant bell. |
| `music/elite` | 2:30 | 112 | Elite enemy: heavy ritual drums and sharp wordless choir shouts. |
| `music/boss_act1` | 3:00 | 104 | The forest comes alive: percussion of wood and bone, creaking, a deep horn. |
| `music/boss_act2` | 3:00 | 100 | The bog: a low swamp choir, bubbling, slow brass-like horns. |
| `music/boss_act3` | 3:00 | 110 | Winter and death: an icy female choir, enormous drums. |
| `music/koschei` | 3:00 | 84 → 120 | Koschei: regal, slow, a low male choir on one note; it speeds up and all instruments join; ends on a heartbeat pulse and a choir cry. |

## Остальные треки (один файл, без слоёв)

Менеджер музыки для этих экранов ещё не написан — сейчас играет только бой.

| файл | длина | промпт |
| --- | --- | --- |
| `music/menu-flow.ogg` | 3:00 | A lone hurdy-gurdy, wind, a distant woman's lament. |
| `music/village-flow.ogg` | 3:30 | Warm gusli, a crackling fire, hope with unease. |
| `music/lobby-flow.ogg` | 2:00 | Tambourine and jaw harp, a rising rhythm. |
| `music/map_act1-flow.ogg` | 2:30 | A reed pipe (zhaleika), creaking trees, owls. |
| `music/map_act2-flow.ogg` | 2:30 | A detuned hurdy-gurdy, dripping water, flies. |
| `music/map_act3-flow.ogg` | 2:30 | A choir on one note, a bell under water. |
| `music/omen-flow.ogg` | 1:30 | Near silence, a single plucked string, rustling. |
| `music/pedlar-flow.ogg` | 1:30 | A crooked cheerful dance tune on a bowed gudok. |
| `music/fire-flow.ogg` | 2:00 | A quiet lullaby on gusli. |
| `music/victory-flow.ogg` | 0:20 | A solemn wordless choir. |
| `music/defeat-flow.ogg` | 0:25 | The music breaks off; a lament and one bell. |
| `music/credits-flow.ogg` | 3:30 | Dawn: the game's theme in a bright major key. |

## Звуки

Шаблон:

```text
Game sound effect, dry and close, <описание>, <длина> seconds, no music, no reverb tail, 48 kHz.
```

| файл | длина | описание |
| --- | --- | --- |
| `sfx/card_play-flow.ogg` | 0.2 | a stiff birch-bark card slapped onto a wooden table |
| `sfx/card_draw-flow.ogg` | 0.15 | a birch-bark card slid off a stack |
| `sfx/hit-flow.ogg` | 0.3 | an iron mace hitting a body through padded cloth |
| `sfx/hit_heavy-flow.ogg` | 0.5 | a heavy iron mace crushing wood and bone, with a low thud |
| `sfx/block-flow.ogg` | 0.25 | a blow stopped by a thick oak shield |
| `sfx/heal-flow.ogg` | 0.5 | water poured from a clay jug, a soft exhale |
| `sfx/status-flow.ogg` | 0.4 | a wet squelch and a short hoarse whisper |
| `sfx/death-flow.ogg` | 0.8 | a creature collapsing into dry leaves and ash |
| `sfx/downed-flow.ogg` | 0.9 | a warrior dropping to his knees in mail, a pained breath |
| `sfx/raised-flow.ogg` | 0.8 | a sharp gasp of someone pulled back to life, mail shifting |
| `sfx/click-flow.ogg` | 0.08 | a small wooden peg tapped on wood |
| `sfx/ready-flow.ogg` | 0.3 | a seal pressed into warm wax |
| `sfx/round-flow.ogg` | 0.6 | one low ritual drum stroke |
| `sfx/rune-flow.ogg` | 0.4 | a knife scoring wood, a faint ringing hum |
| `sfx/potion-flow.ogg` | 0.4 | a cork pulled and a quick gulp |
| `sfx/gold-flow.ogg` | 0.25 | a few old coins dropped into a leather purse |
| `sfx/victory-flow.ogg` | 1.5 | three slow strokes on a large frame drum, rising |
| `sfx/defeat-flow.ogg` | 1.5 | a single cracked church bell, struck once |
