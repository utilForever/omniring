use crate::{BattleObservation, Observation, PokemonState, TeamPreviewObservation};

/// Number of numeric fields in an encoded team preview.
pub const TEAM_PREVIEW_ENCODING_LEN: usize = 72;
/// Number of numeric fields in an encoded battle, including terminal observations.
pub const BATTLE_ENCODING_LEN: usize = 87;

impl TeamPreviewObservation {
    /// Encodes the player's roster, then the opponent's, in roster-slot order (0..6).
    /// Each Pokemon contributes `[hp_curr, hp_max, move_0, move_1, move_2, move_3]`.
    /// HP is raw: `0 <= hp_curr <= hp_max <= u32::MAX`, with `hp_max >= 1`.
    /// Move availability is 0 (unavailable) or 1 (available), in move-slot order.
    /// All fields use `f64` so every valid HP value is represented exactly.
    /// No team selections or phase tag are included.
    pub fn encode(&self) -> [f64; TEAM_PREVIEW_ENCODING_LEN] {
        let mut encoded = [0.0; TEAM_PREVIEW_ENCODING_LEN];

        encode_roster(&self.player, &mut encoded[..36]);
        encode_roster(&self.opponent, &mut encoded[36..72]);

        encoded
    }
}

impl BattleObservation {
    /// Encodes a battle from this observation's Trainer perspective, without reading hidden state.
    /// The first 72 fields follow [`TeamPreviewObservation::encode`], using current roster values.
    /// The remaining zero-based offsets are:
    ///
    /// | Offsets | Fields | Values |
    /// | --- | --- | --- |
    /// | 72..78 | Player selection, slots 0..6 | 0 = unselected, 1 = selected |
    /// | 78..84 | Opponent selection, slots 0..6 | -1 = unknown, 1 = revealed selected |
    /// | 84 | Player active slot | 0..=5, or -1 if no active Pokemon |
    /// | 85 | Opponent active slot | 0..=5, or -1 if no active Pokemon |
    /// | 86 | Terminated | 0 = false, 1 = true |
    ///
    /// Unknown opposing slots always use -1, whether secretly selected or unselected;
    /// they are never inferred from other slots. Revealed selections stay 1 after fainting.
    /// Forced replacements and terminal battles retain this length. Truncation, rewards,
    /// events, and a phase tag are not part of the observation encoding.
    pub fn encode(&self) -> [f64; BATTLE_ENCODING_LEN] {
        let mut encoded = [0.0; BATTLE_ENCODING_LEN];

        encode_roster(self.player.roster(), &mut encoded[..36]);
        encode_roster(self.opponent.roster(), &mut encoded[36..72]);

        encoded[72..78].copy_from_slice(&self.player.selected().map(f64::from));
        encoded[78..84].copy_from_slice(
            &self
                .opponent
                .selection_revealed()
                .map(|revealed| if revealed { 1.0 } else { -1.0 }),
        );
        encoded[84] = self.player.slot_active().map_or(-1.0, |slot| slot as f64);
        encoded[85] = self.opponent.slot_active().map_or(-1.0, |slot| slot as f64);
        encoded[86] = f64::from(self.terminated);

        encoded
    }
}

impl Observation {
    /// Encodes the current phase into a vector of raw, exact numeric fields.
    /// Returns [`TEAM_PREVIEW_ENCODING_LEN`] values for preview or [`BATTLE_ENCODING_LEN`]
    /// for battle. See [`TeamPreviewObservation::encode`] and [`BattleObservation::encode`]
    /// for field order, ranges, and the -1 sentinel for unknown/absent information.
    /// Use the concrete observation's `encode` method when a fixed array is preferred.
    pub fn encode(&self) -> Vec<f64> {
        match self {
            Self::TeamPreview(preview) => preview.encode().to_vec(),
            Self::Battle(battle) => battle.encode().to_vec(),
        }
    }
}

fn encode_roster(roster: &[PokemonState; 6], encoded: &mut [f64]) {
    for (pokemon, fields) in roster.iter().zip(encoded.as_chunks_mut::<6>().0) {
        fields[0] = f64::from(pokemon.hp_curr());
        fields[1] = f64::from(pokemon.hp_max());
        fields[2..].copy_from_slice(&pokemon.move_availability.map(f64::from));
    }
}
