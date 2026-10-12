//! Text-mode fight for checking the rules without graphics.
//!
//!   cargo run -p nav_core --bin console -- [--auto] [--seed N] [--players N] [encounter]
//!
//! Commands: `<n> [target]` play card n, `r` ready, `q` quit.

use nav_core::*;
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn show(c: &Content, s: &Combat) {
    println!("\n== round {} ==", s.round);
    for e in s.living_enemies() {
        let d = &c.enemies[&e.def];
        let intent = e.intent.as_ref().map(|i| {
            let mv = &d.moves[i.mv];
            let dmg = bot::intent_damage(c, s, e).map(|(d, k)| if k > 1 { format!(" {d}x{k}") } else { format!(" {d}") }).unwrap_or_default();
            let at = i.target.map(|t| format!(" -> hero {t}")).unwrap_or_default();
            format!("{}{dmg}{at}", mv.id)
        });
        println!("  [{}] {:<12} hp {:>3}/{:<3} block {:<2} {:?}  intent: {}", e.uid, e.def, e.hp, e.max_hp, e.block, e.statuses, intent.unwrap_or_else(|| "-".into()));
    }
    for h in &s.heroes {
        let state = if h.downed { " DOWNED" } else if h.ready { " ready" } else { "" };
        println!("  hero {} {:<10} hp {:>3}/{:<3} block {:<2} energy {} {:?}{state}", h.slot, h.def, h.hp, h.max_hp, h.block, h.energy, h.statuses);
    }
}

fn show_hand(c: &Content, h: &Hero) {
    for (i, card) in h.hand.iter().enumerate() {
        let v = c.cards[&card.def].view(card.upgraded, h.form);
        println!("    {i}: {}{} ({}) {:?}", card.def, if card.upgraded { "+" } else { "" }, v.cost, v.target);
    }
    println!("    draw {} / discard {} / exhaust {}", h.draw.len(), h.discard.len(), h.exhaust.len());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
    let auto = args.iter().any(|a| a == "--auto");
    let seed = flag("--seed").unwrap_or(1);
    let players = flag("--players").unwrap_or(1).clamp(1, 4) as usize;
    let encounter = args.iter().rev().find(|a| a.starts_with("a")).cloned().unwrap_or_else(|| "a1_wolves".into());

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let c = Content::load_dir(&dir).unwrap_or_else(|e| panic!("content: {e}"));
    let enc = c.encounters.get(&encounter).unwrap_or_else(|| panic!("no encounter `{encounter}`"));
    let mut enemies = enc.enemies.clone();
    if players >= 3 {
        enemies.extend(enc.extra3.iter().cloned());
    }
    if players >= 4 {
        enemies.extend(enc.extra4.iter().cloned());
    }
    let party: Vec<HeroSetup> = (0..players).map(|_| HeroSetup::fresh(&c, "bogatyr").unwrap()).collect();
    let (mut s, _) = start(&c, &party, &enemies, seed);

    let stdin = std::io::stdin();
    while s.phase == Phase::Heroes {
        show(&c, &s);
        for p in 0..players as u8 {
            while s.phase == Phase::Heroes && !s.heroes[p as usize].downed && !s.heroes[p as usize].ready {
                let action = if auto || p > 0 {
                    bot::choose(&c, &s, p)
                } else {
                    show_hand(&c, &s.heroes[0]);
                    print!("> ");
                    std::io::stdout().flush().ok();
                    let mut line = String::new();
                    if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut it = line.split_whitespace();
                    match it.next() {
                        Some("q") => return,
                        Some("r") | None => CombatAction::Ready(true),
                        Some(n) => {
                            let Some(card) = n.parse::<usize>().ok().and_then(|i| s.heroes[0].hand.get(i)) else {
                                println!("no such card");
                                continue;
                            };
                            let v = c.cards[&card.def].view(card.upgraded, s.heroes[0].form);
                            let arg = it.next().and_then(|t| t.parse::<u32>().ok());
                            let target = match v.target {
                                CardTarget::None => None,
                                CardTarget::Enemy => Some(UnitRef::Enemy(arg.or_else(|| s.living_enemies().next().map(|e| e.uid)).unwrap_or(0))),
                                _ => Some(UnitRef::Hero(arg.unwrap_or(0) as u8)),
                            };
                            CombatAction::Play { card: card.uid, target }
                        }
                    }
                };
                let round = s.round;
                match apply(&c, &mut s, p, &action) {
                    Ok(ev) => {
                        if auto || p > 0 {
                            println!("  hero {p}: {action:?}");
                        }
                        for e in ev {
                            println!("      {e:?}");
                        }
                    }
                    Err(e) => println!("  refused: {e:?}"),
                }
                if s.round != round {
                    break;
                }
            }
        }
    }
    show(&c, &s);
    println!("\n{}  (state hash {:016x})", if s.phase == Phase::Won { "VICTORY" } else { "DEFEAT" }, s.hash64());
}
