# Иллюстрации карт — как собирается промпт

Файл-цель: `assets/cards/<id>-gemini.png`, 1000×750. После скачивания:
`python tools/cardart.py <id> [<id> ...]` (берёт свежие загрузки по порядку, режет до 4:3).

Запрос = **начало** + [**облик героя**] + **сюжет** + **хвост**.

## Начало

```text
Generate an image, landscape 4:3 aspect ratio. Card illustration:
```

## Облик героя (если герой в кадре)

| герой | текст |
| --- | --- |
| Богатырь | a huge grey-bearded warrior in hacked mail and an iron face mask, with a tall black oak shield carved with a stern face and a six-flanged mace |
| Ворожея | a gaunt woman in a damp linen shift, braids threaded with small bird bones, milky white eyes, rag dolls hanging on her belt, soot-stained fingers |
| Волхв | a tall old man in animal hides and a birch-bark mask, a long beard with woven charms, dark red rune marks painted on his arms, a staff of lightning-split oak |
| Волколак | a wiry hunter with a wolf pelt on his shoulders and a silver chain around his neck |
| Знахарка | a stooped middle-aged woman in a dark headscarf with old burn scars on her face, bundles of dried herbs on her back, a sickle and two clay jugs at her belt |
| Скоморох | a thin jester in a patchwork coat sewn with small bells, a white grinning mask on his face and three wooden masks on his belt |

## Сюжет

Одно предложение: кто что делает. Одно ясное действие, без свечения и частиц
(«кровь» — `dark red paint`). Для предметов и трав — `still life:` или `close-up of ...`.

Примеры:

- `bogatyr_guard` — … steps in front of a smaller hooded companion and plants his great shield in the mud to cover them both.
- `vol_thunder` — close-up of an old scarred hand carving a jagged lightning-shaped rune into a dark wooden post with a flint knife, fresh pale wood showing in the cut, shavings falling.
- `herb_wormwood` — still life: a bundle of silvery grey-green wormwood tied with twine lying on a dark scarred wooden table beside a small sickle, lit by one candle.

Сюжеты уже нарисованных карт по одной не записаны: список готовых — в `docs/manifest.json` (`done_in_bulk.cards`).

## Хвост

```text
Focus on one clear action, dramatic composition, dark background fading to the edges so a card frame reads well. No magic glow, no sparks or particles. Style: dark painterly digital oil painting, visible thick brushstrokes, heavy chiaroscuro, single key light from upper left, cold rim light, limited palette of soot black, bone white, rust red, swamp green, candle amber, cold blue. Slavic folk horror, inspired by 19th-century Russian painting (Vasnetsov, Vrubel). Grim, grounded, realistic anatomy, readable silhouette. No text, no letters, no watermark, no frame, no modern objects.
```

## Лимит

У Gemini есть суточный лимит на изображения: 12.10.2026 он кончился после ~110 картинок
(44 спрайта врагов, 62 карты, пробы). Сообщение о лимите называет время сброса.
