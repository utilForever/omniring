//! cargo run --example practice_battle

use omniring::{AttackOutcome, TurnOrder, determine_turn_order, simulate_turn};
use omniring::{HeldItem, MegaStone, Nature, Pokemon, StatPoints};
use omniring::build_pokemon_from_pokedex_with_item;

fn main() {
    let stat_points = StatPoints {
        hp: 16,
        attack: 10,
        defense: 10,
        special_attack: 10,
        special_defense: 10,
        speed: 10,
    };

    let mut dragonite = build_pokemon_from_pokedex_with_item(
        "Dragonite",
        50,
        stat_points,
        Nature::neutral(),
        None,
        ["Extreme Speed", "Fire Punch", "Thunder Punch", "Hurricane"],
    )
    .expect("Dragonite sample should be valid");

    let mut lucario = build_pokemon_from_pokedex_with_item(
        "Lucario",
        50,
        stat_points,
        Nature::neutral(),
        Some(HeldItem::MegaStone(MegaStone::Lucarionite)),
        ["Aura Sphere", "Quick Attack", "Metal Claw", "Close Combat"],
    )
    .expect("Lucario sample should be valid");

    println!("Practice single battle");
    print_pokemon(&dragonite);
    print_pokemon(&lucario);

    let planned_turns = [(2, 0), (0, 1), (1, 3), (2, 3), (1, 0), (1, 2)];
    for (turn_index, (dragonite_move, lucario_move)) in planned_turns.iter().copied().enumerate() {
        if dragonite.is_fainted() || lucario.is_fainted() {
            break;
        }

        println!();
        println!("Turn {}", turn_index + 1);
        println!(
            "HP before turn: {} {}/{} | {} {}/{}",
            dragonite.name,
            dragonite.current_hp,
            dragonite.stats.hp,
            lucario.name,
            lucario.current_hp,
            lucario.stats.hp
        );
        println!(
            "{} chooses {} / {} chooses {}",
            dragonite.name,
            dragonite.moves[dragonite_move].name,
            lucario.name,
            lucario.moves[lucario_move].name
        );

        let order = determine_turn_order(&dragonite, dragonite_move, &lucario, lucario_move)
            .expect("planned moves should be valid");
        print_turn_order(order, &dragonite, &lucario);

        let outcome = simulate_turn(&mut dragonite, dragonite_move, &mut lucario, lucario_move)
            .expect("turn should resolve");

        print_attack(outcome.first.as_ref());
        print_attack(outcome.second.as_ref());
        println!(
            "HP after turn: {} {}/{} | {} {}/{}",
            dragonite.name,
            dragonite.current_hp,
            dragonite.stats.hp,
            lucario.name,
            lucario.current_hp,
            lucario.stats.hp
        );
    }
}

fn print_pokemon(pokemon: &Pokemon) {
    println!(
        "{} | Lv.{} | HP {}/{} | Mega-ready: {}",
        pokemon.name, pokemon.level, pokemon.current_hp, pokemon.stats.hp, pokemon.can_mega_evolve
    );
    for (index, move_slot) in pokemon.moves.iter().enumerate() {
        println!(
            "  Slot {}: {} | priority {} | power {} | accuracy {}",
            index, move_slot.name, move_slot.priority, move_slot.power, move_slot.accuracy
        );
    }
}

fn print_turn_order(order: TurnOrder, first: &Pokemon, second: &Pokemon) {
    let first_actor = match order {
        TurnOrder::FirstPokemon => &first.name,
        TurnOrder::SecondPokemon => &second.name,
    };
    println!("First action: {first_actor}");
}

fn print_attack(outcome: Option<&AttackOutcome>) {
    match outcome {
        Some(outcome) => println!(
            "{} used {} on {}: {} damage, effectiveness x{}, defender HP {}",
            outcome.attacker,
            outcome.move_name,
            outcome.defender,
            outcome.damage,
            outcome.effectiveness,
            outcome.defender_hp_after
        ),
        None => println!("The second Pokemon could not move."),
    }
}
