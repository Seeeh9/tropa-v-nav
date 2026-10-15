# Status icons

Small pictures shown in a row under each unit in battle (`crates/game/src/battle/status_row.rs`),
with the number on the chip's corner. Painted in Google Flow in the style of the intent icons
(`assets/intents`): one carved, painted emblem of wood, iron and bone, glowing a little, on flat
pure black, bold readable silhouette that still reads at 34 px, no text, no frame, no border.
Taken in with `python tools/intentart.py --into status <key> ...` (black keyed to alpha, 128 px).

Each Flow message asks for six, numbered; the request opens with:

> Paint six separate square emblem icons for a dark Slavic folk-horror card game. Each is one
> carved and painted emblem of old wood, iron, bone and cloth, lit with a faint inner glow, centred
> on a flat pure black background, with a bold silhouette that stays readable when tiny. No text,
> no letters, no frame, no border, no background scenery. Each image is a separate picture:

| key | what it shows |
|---|---|
| Strength | a mighty bare forearm with a hammered iron armband, fist raised, red-orange glow |
| TempStrength | a fist made of glowing embers and sparks, already breaking into smoke |
| Fortitude | a round oak shield with an iron boss and a carved oak leaf, steady golden glow |
| Vulnerable | a cracked iron breastplate, the crack glowing raw red |
| Weak | a mouth sewn shut with coarse red thread, pale lips (the curse of Muteness) |
| Rot | a rotting skull overgrown with sickly green mushrooms and mould |
| Hex | an evil eye: one bloodshot staring eye in a ring of iron nails |
| Bleed | a fresh cut dripping three heavy drops of dark blood |
| Fear | a pale ghostly face with huge black hollow eyes and an open scream |
| Thorns | a wreath of black bramble with long sharp thorns |
| HuntMark | a hunter's crimson mark: crossed antlers pierced by an arrow |
| Valor | a pointed old Rus warrior helmet radiating golden rays |
| Stun | a cracked helmet with small birds and sparks circling it |
| Mist | swirling wisps of cold grey-blue fog curling into a spiral |
| Chained | heavy rusted chain links wrapped around a wolf's paw |
| marked | the single huge yellow eye of one-eyed Likho, staring |
| blind | an eye bound over with a black rag blindfold |
| coiled | a dark snake coiled tight around a human wrist |
| confused | forest paths knotted into a tangle that crosses itself |
| mired | a leather boot sinking into black swamp mud with bubbles |
| asleep | a closed eye under a crescent moon with a red poppy |
| charmed | a glowing heart of rowan berries bound with a ribbon of pale river-maiden hair |
| frozen | three playing cards locked inside a block of blue ice |
| winter | a large snowflake carved of ice, frost breathing off it |
| immortal | a silver needle inside a cracked egg, cold light |
| last_breath | a guttering candle flame bending, a curl of smoke |
| rune_Thunder | a stone carved with a jagged lightning rune, blue-white light |
| rune_Frost | a stone carved with an icy rune, frost crystals growing on it |
| rune_Mist | a stone carved with a rune half hidden in fog |
| rune_Flame | a stone carved with a fire rune, flames licking up |
| rune_Zhiva | a stone carved with a life rune, a green sprout growing from it |
| moon_Human | a pale crescent moon over a man's calm face in profile |
| moon_Beast | a blood-red full moon behind a snarling wolf's head |
| downed | a fallen sword lying on dark earth, its light gone out |
| last | (spare) |
| last | (spare) |
