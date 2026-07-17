#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleFormat {
    Singles,
    // Doubles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMode {
    Ranked,
    // Casual,
    // Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleMechanic {
    None,
    MegaEvolution,
    // Terastallization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChampionsRules {
    pub format: BattleFormat,
    pub mode: BattleMode,
    pub level_cap: u8,
    pub team_size: u8,
    pub selected_team_size: u8,
    pub mechanic: BattleMechanic,
}

impl ChampionsRules {
    pub fn ranked_doubles() -> Self {
        Self {
            format: BattleFormat::Singles,
            mode: BattleMode::Ranked,
            level_cap: 50,
            team_size: 6,
            selected_team_size: 3,
            mechanic: BattleMechanic::MegaEvolution,
        }
    }

    pub fn casual_singles() -> Self {
        Self {
            format: BattleFormat::Singles,
            mode: BattleMode::Casual,
            level_cap: 50,
            team_size: 3,
            selected_team_size: 3,
            mechanic: BattleMechanic::MegaEvolution,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokemonType {
    Normal,
    Fire,
    Water,
    Electric,
    Grass,
    Ice,
    Fighting,
    Poison,
    Ground,
    Flying,
    Psychic,
    Bug,
    Rock,
    Ghost,
    Dragon,
    Dark,
    Steel,
    Fairy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub special_attack: u16,
    pub special_defense: u16,
    pub speed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatPoints {
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub special_attack: u8,
    pub special_defense: u8,
    pub speed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nature {
    pub increased: Option<StatName>,
    pub decreased: Option<StatName>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatName {
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub name: String,
    pub move_type: PokemonType,
    pub category: MoveCategory,
    pub power: u16,
    pub accuracy: u8,
    pub priority: i8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pokemon {
    pub name: String,
    pub level: u8,
    pub primary_type: PokemonType,
    pub secondary_type: Option<PokemonType>,
    pub stats: Stats,
    pub stat_points: StatPoints,
    pub nature: Nature,
    pub ability: Option<String>,
    pub item: Option<String>,
    pub current_hp: u16,
    pub moves: [Move; 4],
    pub can_mega_evolve: bool,
    pub has_used_battle_mechanic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokemonSpec {
    pub name: String,
    pub level: u8,
    pub primary_type: PokemonType,
    pub secondary_type: Option<PokemonType>,
    pub stats: Stats,
    pub stat_points: StatPoints,
    pub nature: Nature,
    pub moves: [Move; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u16,
    pub effectiveness: f32,
    pub stab: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttackOutcome {
    pub attacker: String,
    pub defender: String,
    pub move_name: String,
    pub damage: u16,
    pub effectiveness: f32,
    pub stab: bool,
    pub defender_hp_after: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    pub first: Option<AttackOutcome>,
    pub second: Option<AttackOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleError {
    InvalidMoveIndex { index: usize },
    FaintedPokemonCannotAttack,
    InvalidStatPoints,
}

impl StatPoints {
    pub const MAX_TOTAL: u8 = 66;
    pub const MAX_PER_STAT: u8 = 32;

    pub fn total(self) -> u8 {
        self.hp
            + self.attack
            + self.defense
            + self.special_attack
            + self.special_defense
            + self.speed
    }

    pub fn is_valid_for_champions(self) -> bool {
        self.total() <= Self::MAX_TOTAL
            && self.hp <= Self::MAX_PER_STAT
            && self.attack <= Self::MAX_PER_STAT
            && self.defense <= Self::MAX_PER_STAT
            && self.special_attack <= Self::MAX_PER_STAT
            && self.special_defense <= Self::MAX_PER_STAT
            && self.speed <= Self::MAX_PER_STAT
    }
}

impl Nature {
    pub fn neutral() -> Self {
        Self {
            increased: None,
            decreased: None,
        }
    }
}

impl Move {
    pub fn new(
        name: impl Into<String>,
        move_type: PokemonType,
        category: MoveCategory,
        power: u16,
        accuracy: u8,
        priority: i8,
    ) -> Self {
        Self {
            name: name.into(),
            move_type,
            category,
            power,
            accuracy,
            priority,
        }
    }
}

impl Pokemon {
    pub fn new(spec: PokemonSpec) -> Result<Self, BattleError> {
        if !spec.stat_points.is_valid_for_champions() {
            return Err(BattleError::InvalidStatPoints);
        }

        Ok(Self {
            name: spec.name,
            level: spec.level,
            primary_type: spec.primary_type,
            secondary_type: spec.secondary_type,
            stats: spec.stats,
            stat_points: spec.stat_points,
            nature: spec.nature,
            ability: None,
            item: None,
            current_hp: spec.stats.hp,
            moves: spec.moves,
            can_mega_evolve: false,
            has_used_battle_mechanic: false,
        })
    }

    pub fn is_fainted(&self) -> bool {
        self.current_hp == 0
    }

    pub fn has_type(&self, pokemon_type: PokemonType) -> bool {
        self.primary_type == pokemon_type || self.secondary_type == Some(pokemon_type)
    }
}

pub fn calculate_damage(
    attacker: &Pokemon,
    defender: &Pokemon,
    selected_move: &Move,
) -> DamageResult {
    if selected_move.category == MoveCategory::Status || selected_move.power == 0 {
        return DamageResult {
            damage: 0,
            effectiveness: 1.0,
            stab: false,
        };
    }

    let (attack, defense) = match selected_move.category {
        MoveCategory::Physical => (attacker.stats.attack, defender.stats.defense),
        MoveCategory::Special => (
            attacker.stats.special_attack,
            defender.stats.special_defense,
        ),
        MoveCategory::Status => unreachable!("status moves return before damage calculation"),
    };

    let level_factor = (2 * u32::from(attacker.level)) / 5 + 2;
    let base_damage = (((level_factor * u32::from(selected_move.power) * u32::from(attack))
        / u32::from(defense))
        / 50)
        + 2;
    let stab = attacker.has_type(selected_move.move_type);
    let effectiveness = type_effectiveness_against(selected_move.move_type, defender);
    let stab_modifier = if stab { 1.5 } else { 1.0 };
    let modified_damage = (base_damage as f32 * stab_modifier * effectiveness).floor() as u16;
    let damage = if effectiveness == 0.0 {
        0
    } else {
        modified_damage.max(1)
    };

    DamageResult {
        damage,
        effectiveness,
        stab,
    }
}

pub fn execute_attack(
    attacker: &Pokemon,
    defender: &mut Pokemon,
    move_index: usize,
) -> Result<AttackOutcome, BattleError> {
    if attacker.is_fainted() {
        return Err(BattleError::FaintedPokemonCannotAttack);
    }

    let selected_move = attacker
        .moves
        .get(move_index)
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })?;
    let damage_result = calculate_damage(attacker, defender, selected_move);
    defender.current_hp = defender.current_hp.saturating_sub(damage_result.damage);

    Ok(AttackOutcome {
        attacker: attacker.name.clone(),
        defender: defender.name.clone(),
        move_name: selected_move.name.clone(),
        damage: damage_result.damage,
        effectiveness: damage_result.effectiveness,
        stab: damage_result.stab,
        defender_hp_after: defender.current_hp,
    })
}

pub fn simulate_turn(
    first_pokemon: &mut Pokemon,
    first_move_index: usize,
    second_pokemon: &mut Pokemon,
    second_move_index: usize,
) -> Result<TurnOutcome, BattleError> {
    validate_move_index(first_pokemon, first_move_index)?;
    validate_move_index(second_pokemon, second_move_index)?;

    let first_move = &first_pokemon.moves[first_move_index];
    let second_move = &second_pokemon.moves[second_move_index];
    let first_goes_first = first_move.priority > second_move.priority
        || (first_move.priority == second_move.priority
            && first_pokemon.stats.speed >= second_pokemon.stats.speed);

    if first_goes_first {
        resolve_turn_order(
            first_pokemon,
            first_move_index,
            second_pokemon,
            second_move_index,
        )
    } else {
        let reversed = resolve_turn_order(
            second_pokemon,
            second_move_index,
            first_pokemon,
            first_move_index,
        )?;
        Ok(TurnOutcome {
            first: reversed.second,
            second: reversed.first,
        })
    }
}

fn resolve_turn_order(
    faster: &mut Pokemon,
    faster_move_index: usize,
    slower: &mut Pokemon,
    slower_move_index: usize,
) -> Result<TurnOutcome, BattleError> {
    let first = execute_attack(faster, slower, faster_move_index)?;
    let second = if slower.is_fainted() {
        None
    } else {
        Some(execute_attack(slower, faster, slower_move_index)?)
    };

    Ok(TurnOutcome {
        first: Some(first),
        second,
    })
}

fn validate_move_index(pokemon: &Pokemon, move_index: usize) -> Result<(), BattleError> {
    pokemon
        .moves
        .get(move_index)
        .map(|_| ())
        .ok_or(BattleError::InvalidMoveIndex { index: move_index })
}

pub fn type_effectiveness_against(attack_type: PokemonType, defender: &Pokemon) -> f32 {
    let primary = type_effectiveness(attack_type, defender.primary_type);
    let secondary = defender.secondary_type.map_or(1.0, |defense_type| {
        type_effectiveness(attack_type, defense_type)
    });

    primary * secondary
}

pub fn type_effectiveness(attack_type: PokemonType, defense_type: PokemonType) -> f32 {
    use PokemonType::{
        Bug, Dark, Dragon, Electric, Fairy, Fighting, Fire, Flying, Ghost, Grass, Ground, Ice,
        Normal, Poison, Psychic, Rock, Steel, Water,
    };

    match (attack_type, defense_type) {
        (Normal, Rock | Steel) => 0.5,
        (Normal, Ghost) => 0.0,
        (Fire, Fire | Water | Rock | Dragon) => 0.5,
        (Fire, Grass | Ice | Bug | Steel) => 2.0,
        (Water, Water | Grass | Dragon) => 0.5,
        (Water, Fire | Ground | Rock) => 2.0,
        (Electric, Electric | Grass | Dragon) => 0.5,
        (Electric, Water | Flying) => 2.0,
        (Electric, Ground) => 0.0,
        (Grass, Fire | Grass | Poison | Flying | Bug | Dragon | Steel) => 0.5,
        (Grass, Water | Ground | Rock) => 2.0,
        (Ice, Fire | Water | Ice | Steel) => 0.5,
        (Ice, Grass | Ground | Flying | Dragon) => 2.0,
        (Fighting, Poison | Flying | Psychic | Bug | Fairy) => 0.5,
        (Fighting, Normal | Ice | Rock | Dark | Steel) => 2.0,
        (Fighting, Ghost) => 0.0,
        (Poison, Poison | Ground | Rock | Ghost) => 0.5,
        (Poison, Grass | Fairy) => 2.0,
        (Poison, Steel) => 0.0,
        (Ground, Grass | Bug) => 0.5,
        (Ground, Fire | Electric | Poison | Rock | Steel) => 2.0,
        (Ground, Flying) => 0.0,
        (Flying, Electric | Rock | Steel) => 0.5,
        (Flying, Grass | Fighting | Bug) => 2.0,
        (Psychic, Psychic | Steel) => 0.5,
        (Psychic, Fighting | Poison) => 2.0,
        (Psychic, Dark) => 0.0,
        (Bug, Fire | Fighting | Poison | Flying | Ghost | Steel | Fairy) => 0.5,
        (Bug, Grass | Psychic | Dark) => 2.0,
        (Rock, Fighting | Ground | Steel) => 0.5,
        (Rock, Fire | Ice | Flying | Bug) => 2.0,
        (Ghost, Dark) => 0.5,
        (Ghost, Psychic | Ghost) => 2.0,
        (Ghost, Normal) => 0.0,
        (Dragon, Steel) => 0.5,
        (Dragon, Dragon) => 2.0,
        (Dragon, Fairy) => 0.0,
        (Dark, Fighting | Dark | Fairy) => 0.5,
        (Dark, Psychic | Ghost) => 2.0,
        (Steel, Fire | Water | Electric | Steel) => 0.5,
        (Steel, Ice | Rock | Fairy) => 2.0,
        (Fairy, Fire | Poison | Steel) => 0.5,
        (Fairy, Fighting | Dragon | Dark) => 2.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tackle() -> Move {
        Move::new(
            "Tackle",
            PokemonType::Normal,
            MoveCategory::Physical,
            40,
            100,
            0,
        )
    }

    fn quick_attack() -> Move {
        Move::new(
            "Quick Attack",
            PokemonType::Normal,
            MoveCategory::Physical,
            40,
            100,
            1,
        )
    }

    fn ember() -> Move {
        Move::new(
            "Ember",
            PokemonType::Fire,
            MoveCategory::Special,
            40,
            100,
            0,
        )
    }

    fn vine_whip() -> Move {
        Move::new(
            "Vine Whip",
            PokemonType::Grass,
            MoveCategory::Physical,
            45,
            100,
            0,
        )
    }

    fn splash() -> Move {
        Move::new(
            "Splash",
            PokemonType::Normal,
            MoveCategory::Status,
            0,
            100,
            0,
        )
    }

    fn valid_stat_points() -> StatPoints {
        StatPoints {
            hp: 16,
            attack: 10,
            defense: 10,
            special_attack: 10,
            special_defense: 10,
            speed: 10,
        }
    }

    fn charmander() -> Pokemon {
        Pokemon::new(PokemonSpec {
            name: "Charmander".to_string(),
            level: 50,
            primary_type: PokemonType::Fire,
            secondary_type: None,
            stats: Stats {
                hp: 120,
                attack: 80,
                defense: 65,
                special_attack: 95,
                special_defense: 75,
                speed: 90,
            },
            stat_points: valid_stat_points(),
            nature: Nature::neutral(),
            moves: [ember(), tackle(), splash(), tackle()],
        })
        .unwrap()
    }

    fn bulbasaur() -> Pokemon {
        Pokemon::new(PokemonSpec {
            name: "Bulbasaur".to_string(),
            level: 50,
            primary_type: PokemonType::Grass,
            secondary_type: Some(PokemonType::Poison),
            stats: Stats {
                hp: 125,
                attack: 82,
                defense: 83,
                special_attack: 85,
                special_defense: 85,
                speed: 70,
            },
            stat_points: valid_stat_points(),
            nature: Nature::neutral(),
            moves: [vine_whip(), tackle(), splash(), tackle()],
        })
        .unwrap()
    }

    #[test]
    fn champions_ranked_doubles_uses_current_public_defaults() {
        let rules = ChampionsRules::ranked_doubles();

        assert_eq!(rules.format, BattleFormat::Doubles);
        assert_eq!(rules.mode, BattleMode::Ranked);
        assert_eq!(rules.team_size, 6);
        assert_eq!(rules.selected_team_size, 4);
        assert_eq!(rules.mechanic, BattleMechanic::MegaEvolution);
    }

    #[test]
    fn rejects_invalid_champions_stat_points() {
        let invalid_points = StatPoints {
            hp: 33,
            attack: 33,
            defense: 0,
            special_attack: 0,
            special_defense: 0,
            speed: 0,
        };

        let result = Pokemon::new(PokemonSpec {
            name: "Invalid".to_string(),
            level: 50,
            primary_type: PokemonType::Normal,
            secondary_type: None,
            stats: Stats {
                hp: 100,
                attack: 100,
                defense: 100,
                special_attack: 100,
                special_defense: 100,
                speed: 100,
            },
            stat_points: invalid_points,
            nature: Nature::neutral(),
            moves: [tackle(), tackle(), tackle(), tackle()],
        });

        assert_eq!(result, Err(BattleError::InvalidStatPoints));
    }

    #[test]
    fn calculates_stab_and_super_effective_damage() {
        let attacker = charmander();
        let defender = bulbasaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[0]);

        assert_eq!(result.effectiveness, 2.0);
        assert!(result.stab);
        assert_eq!(result.damage, 63);
    }

    #[test]
    fn status_moves_deal_no_damage() {
        let attacker = charmander();
        let defender = bulbasaur();

        let result = calculate_damage(&attacker, &defender, &attacker.moves[2]);

        assert_eq!(result.damage, 0);
        assert_eq!(result.effectiveness, 1.0);
        assert!(!result.stab);
    }

    #[test]
    fn priority_move_can_attack_before_faster_pokemon() {
        let mut slower = bulbasaur();
        slower.moves[0] = quick_attack();
        let mut faster = charmander();

        let outcome = simulate_turn(&mut slower, 0, &mut faster, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Bulbasaur");
    }

    #[test]
    fn faster_pokemon_attacks_first_and_fainting_stops_counterattack() {
        let mut attacker = charmander();
        let mut defender = bulbasaur();
        defender.current_hp = 10;

        let outcome = simulate_turn(&mut attacker, 0, &mut defender, 0).unwrap();

        assert_eq!(outcome.first.unwrap().attacker, "Charmander");
        assert!(outcome.second.is_none());
        assert_eq!(defender.current_hp, 0);
    }

    #[test]
    fn invalid_move_index_returns_error() {
        let mut attacker = charmander();
        let mut defender = bulbasaur();

        let result = simulate_turn(&mut attacker, 4, &mut defender, 0);

        assert_eq!(result, Err(BattleError::InvalidMoveIndex { index: 4 }));
    }
}
