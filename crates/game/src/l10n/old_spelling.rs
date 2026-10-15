//! Modern Russian spelling turned into the pre-1918 one, for the old speech: a hard sign after
//! every word ending in a consonant, і before a vowel, no ё, and ѣ wherever the old books had it.
//! The yat can't be ruled from sound, so it comes from lists of the roots and endings that carry
//! it; a word the lists miss keeps its е, which an old reader forgives sooner than a ѣ put where
//! none belongs.

/// Whole words, lowercase, ё already read as е.
const WORDS: &[(&str, &str)] = &[
    ("где", "гдѣ"),
    ("бе", "бѣ"),
    ("зде", "здѣ"),
    ("те", "тѣ"),
    ("тех", "тѣх"),
    ("тем", "тѣм"),
    ("теми", "тѣми"),
    ("здесь", "здѣсь"),
    ("нет", "нѣт"),
    ("две", "двѣ"),
    ("обе", "обѣ"),
    ("мне", "мнѣ"),
    ("тебе", "тебѣ"),
    ("себе", "себѣ"),
    ("после", "послѣ"),
    ("более", "болѣе"),
    ("менее", "менѣе"),
    ("вне", "внѣ"),
    ("ее", "ея"),
    ("лет", "лѣт"),
    ("ел", "ѣл"),
    ("ела", "ѣла"),
    ("ели", "ѣли"),
    ("петь", "пѣть"),
    ("пел", "пѣл"),
    ("пела", "пѣла"),
    ("пели", "пѣли"),
    ("бес", "бѣс"),
    ("бесы", "бѣсы"),
    ("бесов", "бѣсов"),
    ("лень", "лѣнь"),
    ("мир", "мір"),
    ("мира", "міра"),
    ("миру", "міру"),
    ("мире", "мірѣ"),
    ("миром", "міром"),
];

/// Roots carrying ѣ, found at the start of a word or after one of `PREFIXES`.
const ROOTS: &[(&str, &str)] = &[
    ("бел", "бѣл"),
    ("бед", "бѣд"),
    ("бег", "бѣг"),
    ("беж", "бѣж"),
    ("вер", "вѣр"),
    ("вес", "вѣс"),
    ("вет", "вѣт"),
    ("вед", "вѣд"),
    ("веж", "вѣж"),
    ("век", "вѣк"),
    ("веч", "вѣч"),
    ("вещи", "вѣщі"),
    ("вещ", "вѣщ"),
    ("дел", "дѣл"),
    ("дев", "дѣв"),
    ("дет", "дѣт"),
    ("дед", "дѣд"),
    ("дейст", "дѣйст"),
    ("ед", "ѣд"),
    ("езд", "ѣзд"),
    ("еха", "ѣха"),
    ("желез", "желѣз"),
    ("звезд", "звѣзд"),
    ("звер", "звѣр"),
    ("зрел", "зрѣл"),
    ("лес", "лѣс"),
    ("лев", "лѣв"),
    ("леп", "лѣп"),
    ("лето", "лѣто"),
    ("летн", "лѣтн"),
    ("леч", "лѣч"),
    ("лез", "лѣз"),
    ("лени", "лѣни"),
    ("ленив", "лѣнив"),
    ("мест", "мѣст"),
    ("меш", "мѣш"),
    ("мен", "мѣн"),
    ("мер", "мѣр"),
    ("мет", "мѣт"),
    ("мех", "мѣх"),
    ("немо", "нѣмо"),
    ("немот", "нѣмот"),
    ("немая", "нѣмая"),
    ("немую", "нѣмую"),
    ("немые", "нѣмые"),
    ("немых", "нѣмых"),
    ("немее", "нѣмѣе"),
    ("немец", "нѣмец"),
    ("немц", "нѣмц"),
    ("неж", "нѣж"),
    ("нед", "нѣд"),
    ("нек", "нѣк"),
    ("пен", "пѣн"),
    ("песн", "пѣсн"),
    ("песен", "пѣсен"),
    ("пев", "пѣв"),
    ("петух", "пѣтух"),
    ("плен", "плѣн"),
    ("плес", "плѣс"),
    ("рез", "рѣз"),
    ("рек", "рѣк"),
    ("реч", "рѣч"),
    ("ред", "рѣд"),
    ("реш", "рѣш"),
    ("сед", "сѣд"),
    ("сев", "сѣв"),
    ("сер", "сѣр"),
    ("сеч", "сѣч"),
    ("сек", "сѣк"),
    ("сет", "сѣт"),
    ("сен", "сѣн"),
    ("сем", "сѣм"),
    ("сме", "смѣ"),
    ("снег", "снѣг"),
    ("слеп", "слѣп"),
    ("след", "слѣд"),
    ("свет", "свѣт"),
    ("свеч", "свѣч"),
    ("свеж", "свѣж"),
    ("тел", "тѣл"),
    ("тен", "тѣн"),
    ("тес", "тѣс"),
    ("хлеб", "хлѣб"),
    ("хмел", "хмѣл"),
    ("цел", "цѣл"),
    ("цеп", "цѣп"),
    ("цвет", "цвѣт"),
    ("человек", "человѣк"),
    ("грех", "грѣх"),
    ("греш", "грѣш"),
    ("гнев", "гнѣв"),
    ("гнезд", "гнѣзд"),
    ("орех", "орѣх"),
    ("успех", "успѣх"),
    ("обет", "обѣт"),
    ("север", "сѣвер"),
];

/// Prefixes a yat root may stand after; the bare root is tried first.
const PREFIXES: &[&str] = &[
    "", "без", "бес", "в", "вз", "воз", "вы", "до", "за", "из", "ис", "на", "над", "не", "о", "об", "обо", "от", "пере",
    "по", "под", "пре", "при", "про", "раз", "рас", "с", "со", "у",
];

/// Words that begin like a yat root but keep their е, checked against the word with or without
/// its prefix.
const KEEP_E: &[&str] = &[
    "верх", "верт", "верев", "весел", "весн", "весен", "весл", "весь", "вечер", "ведр", "ветх", "ветеран", "девят",
    "един", "едва", "мерт", "мерз", "мерк", "меня", "мелк", "мета", "метал", "метел", "метл", "метод", "метр", "мести",
    "месть", "механ", "песок", "песк", "резв", "сестр", "серд", "серп", "серьг", "серьез", "серебр", "сетов", "седьм",
    "секун", "секр", "секта", "сем", "телег", "телен", "телят", "телеф", "лест", "лент", "олен", "умер", "смерт", "смел",
    "сер", "сен", "менее", "немного", "немал", "немедл", "реки", "ред", "рекрут", "лезв", "верз", "верб", "влеч", "увлеч", "привлеч", "вести", "ведет", "ведут", "веду",
];

/// After these a noun ending in -е stands in a case that ends in -ѣ: «въ полѣ», «къ трапезѣ».
const LOCATIVE: &[&str] = &["в", "во", "на", "о", "об", "обо", "при", "к", "ко"];

fn is_vowel(c: char) -> bool {
    "аеиоуыэюяѣ".contains(c)
}

fn is_consonant(c: char) -> bool {
    "бвгджзклмнпрстфхцчшщ".contains(c)
}

fn keeps_e(w: &str) -> bool {
    KEEP_E.iter().any(|k| w.starts_with(k))
}

/// The ѣ of one lowercase word: whole words, then roots, then the commonest endings.
fn yat(w: &str, after: &str) -> String {
    if let Some((_, old)) = WORDS.iter().find(|(m, _)| *m == w) {
        return old.to_string();
    }
    let kept = keeps_e(w);
    let mut w = w.to_string();
    'roots: for prefix in PREFIXES {
        let Some(rest) = w.strip_prefix(prefix) else { continue };
        if keeps_e(rest) || keeps_e(&w) {
            continue;
        }
        for (m, old) in ROOTS {
            if let Some(tail) = rest.strip_prefix(m) {
                w = format!("{prefix}{old}{tail}");
                break 'roots;
            }
        }
    }
    let n = w.chars().count();
    // verbs in -еть and their past (смотрѣть, сидѣлъ), comparatives (сильнѣе)
    let endings: &[(&str, &str, usize)] = &[("еть", "ѣть", 5), ("ела", "ѣла", 6), ("ело", "ѣло", 6), ("ели", "ѣли", 6), ("ел", "ѣл", 5), ("ее", "ѣе", 6)];
    for (end, old, min) in endings {
        if let Some(stem) = w.strip_suffix(end) {
            let wrong = ["тер", "пер", "мер", "ш", "ж", "ч", "щ", "в", "пеп", "кот", "дят", "ор", "ос", "коз", "уз", "ут", "ангел", "ст"].iter().any(|s| stem.ends_with(s));
            if n >= *min && !wrong && !kept && !stem.ends_with('ѣ') {
                w = format!("{stem}{old}");
            }
            break;
        }
    }
    if LOCATIVE.contains(&after) && n >= 3 && w.ends_with('е') && !["ие", "ое", "ее", "ые", "ѣе"].iter().any(|e| w.ends_with(e)) {
        w.pop();
        w.push('ѣ');
    }
    w
}

/// One word in modern spelling, any case, as the old books would print it.
fn word(original: &str, after: &str) -> (String, String) {
    let lower: String = original.to_lowercase().replace('ё', "е");
    let w = yat(&lower, after);
    let chars: Vec<char> = w.chars().collect();
    let mut out: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| if c == 'и' && chars.get(i + 1).is_some_and(|&n| is_vowel(n) || n == 'й') { 'і' } else { c })
        .collect();
    if out.chars().last().is_some_and(is_consonant) {
        out.push('ъ');
    }
    // the case of the original, letter by letter; letters grown past its end follow its last
    let orig: Vec<char> = original.chars().collect();
    let caps = orig.len() > 1 && orig.iter().all(|c| !c.is_lowercase());
    let cased = out
        .chars()
        .enumerate()
        .map(|(i, c)| if caps || orig.get(i).is_some_and(|o| o.is_uppercase()) { c.to_uppercase().collect::<String>() } else { c.to_string() })
        .collect();
    (cased, lower)
}

fn cyrillic(c: char) -> bool {
    matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё' | 'ѣ' | 'Ѣ' | 'і' | 'І')
}

/// Rewrites every Cyrillic word of `text`, leaving numbers, Latin and markup alone.
pub fn convert(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut cur = String::new();
    // the word before, so a preposition can put the next one in its case
    let mut last = String::new();
    let flush = |cur: &mut String, out: &mut String, last: &mut String| {
        if !cur.is_empty() {
            let (w, lower) = word(cur, last);
            out.push_str(&w);
            *last = lower;
            cur.clear();
        }
    };
    for c in text.chars() {
        if cyrillic(c) {
            cur.push(c);
        } else {
            flush(&mut cur, &mut out, &mut last);
            if !c.is_whitespace() {
                last.clear();
            }
            out.push(c);
        }
    }
    flush(&mut cur, &mut out, &mut last);
    out
}

#[cfg(test)]
mod tests {
    use super::convert;

    #[test]
    fn old_books() {
        assert_eq!(convert("Белый лес, где нет света."), "Бѣлый лѣсъ, гдѣ нѣтъ свѣта.");
        assert_eq!(convert("Наложите немоту 2"), "Наложите нѣмоту 2");
        assert_eq!(convert("синий ветер"), "синій вѣтеръ");
        assert_eq!(convert("ЛЕС"), "ЛѢСЪ");
        assert_eq!(convert("её"), "ея");
        assert_eq!(convert("весна весела"), "весна весела");
        assert_eq!(convert("смерть"), "смерть");
        assert_eq!(convert("в поле на земле"), "въ полѣ на землѣ");
        assert_eq!(convert("он смотрел и сидел"), "онъ смотрѣлъ и сидѣлъ");
        assert_eq!(convert("победа соседа"), "побѣда сосѣда");
        assert_eq!(convert("Бѣлый лѣсъ"), "Бѣлый лѣсъ");
        assert_eq!(convert("немного"), "немного");
    }
}

#[cfg(test)]
/// `DUMP=<file> cargo test dump_story -- --ignored` writes the old tale as it would be printed, to read through.
mod dump {
    #[test]
    #[ignore]
    fn dump_story() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/i18n/ru-old/story.ftl")).unwrap();
        let out: String = src.lines().map(|l| super::convert(l) + "\n").collect();
        std::fs::write(std::env::var("DUMP").unwrap(), out).unwrap();
    }
}
