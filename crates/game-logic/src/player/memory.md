# memory.md — module `player`

> Contexte pour l'IA : ce module définit les **4 personnages jouables** et leurs **compétences** (1 bonus + 1 malus par perso). Il est consommé par `engine` pour appliquer les effets des skills sur le `GameState`. En PvE le malus est désactivé ; en PvP le malus cible l'adversaire.

**Utiliser les principes DRY & SOLID, faire du clean code.**

---

## Responsabilités

- Définir le trait `Character` et les 4 implémentations
- Modéliser les compétences bonus (défensives/utilitaires) et malus (offensives)
- Gérer les **cooldowns** de compétences
- Gérer la **barre d'énergie ultime** (remplie au score, vidée à l'activation)
- Décrire les effets à appliquer sur `GameState` (retournés comme `SkillEffect`)

---

## Fichiers du module

| Fichier | Rôle |
|---------|------|
| `mod.rs` | Re-exports publics |
| `personnages/character.rs` | Trait `Character` + sélection de personnage |
| `personnages/character_stats.rs` | `CharacterStats` struct (cooldowns, HP perso, etc.) |
| `skills/mod.rs` | Re-exports skills |
| `skills/player_bonus.rs` | Enum `BonusSkill` + implémentation des effets |
| `skills/player_malus.rs` | Enum `MalusSkill` + implémentation des effets |

---

## Les 4 personnages (à implémenter dans `personnages/`)

| ID | Nom | Archétype | Bonus | Malus |
|----|-----|-----------|-------|-------|
| 0 | RoboCop | Défensif / Protection | `Shield` (10-30s) | `InkBlot` |
| 1 | Judge Dredd | Justice implacable | `DamageBoost` | `BumperReduction` |
| 2 | Hacker | Manipulation terrain | `ComboMultiplier` (x2-x4) | `Invisible` (bille invisible) |
| 3 | Cyborg agressif | Offensif | `ExtraFlippers` (4→6) | `ModifyBounce` |

> Note: La sélection définitive des personnages peut encore évoluer. La structure bonus/malus est figée.

---

## Trait `Character` (`character.rs`)

```rust
pub trait Character: Send + Sync {
    fn id(&self) -> u8;
    fn name(&self) -> &'static str;
    fn stats(&self) -> &CharacterStats;
    fn bonus(&self) -> BonusSkill;
    fn malus(&self) -> MalusSkill;
}

pub struct CharacterStats {
    pub ultimate_charge_max: u32,     // energie max avant activation possible
    pub bonus_cooldown_ms: u64,       // temps de recharge après utilisation
    pub malus_cooldown_ms: u64,
}

// Sélecteur de personnage
pub fn select_character(id: u8) -> Box<dyn Character> { ... }
```

---

## Skills Bonus (`player_bonus.rs`)

```rust
pub enum BonusSkill {
    Shield,            // Empêche 1 perte de bille (10-30s). Bloque charge ultime pendant actif.
    TimeSlowdown,      // Ralentit bille ~5-10s
    ComboMultiplier,   // x2 à x4 sur les points marqués
    DamageBoost,       // Augmente dégâts infligés au boss
    ExtraFlippers,     // 4 → 6 flippers temporaire
    Portal,            // 2 charges : entrée + sortie. Conserve les points.
    Freeze,            // Gèle adversaire (PvP) ou bille (PvE) brièvement
    ExtraBall,         // Ajoute 1 bille temporaire (usage unique, limite d'abus)
}

impl BonusSkill {
    // Retourne l'effet à appliquer sur GameState
    pub fn activate(&self, state: &mut GameState) -> SkillEffect { ... }
}
```

### Effets par bonus

| Bonus | Effet sur `GameState` |
|-------|-----------------------|
| `Shield` | `state.shield_active = true`, durée 10-30s |
| `TimeSlowdown` | Émet `ScreenEnvelope` → front modifie vitesse bille |
| `ComboMultiplier` | `state.active_multiplier *= 2.0..=4.0` |
| `DamageBoost` | `state.damage_multiplier *= 1.5` |
| `ExtraFlippers` | Émet event → ESP32 active 2 flippers supplémentaires |
| `Portal` | Émet event `PortalCheckpoint` → front gère la téléportation |
| `Freeze` | Émet event `FreezeOpponent` (PvP) ou `FreezeBall` (PvE) |
| `ExtraBall` | `state.extra_balls += 1` |

---

## Skills Malus (`player_malus.rs`)

```rust
pub enum MalusSkill {
    Invisible,         // Bille adversaire invisible (très puissant)
    InkBlot,           // Tache sur l'écran adversaire (style Mario Kart)
    BumperReduction,   // Réduit taille bumpers adverses
    BlackHole,         // Trou à position aléatoire (à équilibrer)
    ModifyBounce,      // Rebond aléatoire/augmenté
    StickyBumpers,     // Bille colle aux bumpers adverses
}

impl MalusSkill {
    // En PvE → retourne SkillEffect::NoEffect
    // En PvP → retourne l'effet à appliquer sur l'adversaire
    pub fn activate(&self, game_mode: GameMode) -> SkillEffect { ... }
}
```

---

## Type `SkillEffect` (commun aux deux)

```rust
pub enum SkillEffect {
    ModifyMultiplier { factor: f32, duration_ms: u64 },
    AddBalls { count: u8 },
    ShieldActivated { duration_ms: u64 },
    AddScore { pts: u32 },
    EmitScreenEvent { event_type: String, payload: serde_json::Value },
    NoEffect,
}
```

---

## Activation de l'ultime (intégration avec `engine`)

La barre d'ultime se charge via les points marqués :
```
charge_gained = pts_gagnes / ULTIME_CHARGE_RATIO
```

Activation : joueur maintient les 2 flippers levés → relâche → `UltimateActivated` event.

```rust
// Dans engine/core.rs, à la réception de UltimateActivated :
if state.ultimate_charge >= character.stats().ultimate_charge_max {
    let effect = character.bonus().activate(&mut state);
    apply_skill_effect(effect, &mut envelopes);
    state.ultimate_charge = 0;
    // Démarrer le cooldown du bonus
}
```

---

## Mode PvP — logique malus

En PvP, `MalusSkill::activate(GameMode::Pvp)` émet un `ScreenEnvelope` ciblant l'**écran adverse** :
- `ScreenTarget::Specific(opponent_screen_id)`
- Le frontend adverse reçoit l'event et applique l'effet visuel

En PvE, les malus sont ignorés (`SkillEffect::NoEffect`).

---

## État d'implémentation

| Fichier | Status | Prochaine action |
|---------|--------|-----------------|
| `mod.rs` | ❌ Vide | Re-exporter `Character`, `select_character` |
| `personnages/character.rs` | ❌ Vide | Trait `Character` + 4 structs (RoboCop, JudgeDredd, Hacker, Cyborg) |
| `personnages/character_stats.rs` | ❌ Vide | `CharacterStats` struct |
| `skills/mod.rs` | ❌ Vide | Re-exporter skills |
| `skills/player_bonus.rs` | ❌ Vide | `BonusSkill` enum + `activate()` |
| `skills/player_malus.rs` | ❌ Vide | `MalusSkill` enum + `activate()` |

---

## Tests à écrire

```rust
// character.rs #[cfg(test)]
- test_select_character_robocp()   → id=0, bonus=Shield, malus=InkBlot
- test_select_invalid_id()         → panique ou default

// player_bonus.rs #[cfg(test)]
- test_shield_activates()          → state.shield_active == true
- test_extra_ball_increments()     → state.extra_balls += 1
- test_combo_multiplier_applies()  → state.active_multiplier >= 2.0

// player_malus.rs #[cfg(test)]
- test_malus_pve_no_effect()       → SkillEffect::NoEffect en mode PvE
- test_malus_pvp_has_effect()      → effet non-NoEffect en mode PvP
```

---

## Décisions de design déjà prises

- **V1** : différenciation uniquement par couleur (pas de modèles 3D distincts)
- **Bonus** actif en PvE et PvP
- **Malus** actif uniquement en PvP (disparaît en PvE)
- **`Shield`** bloque l'accumulation de charge d'ultime pendant qu'il est actif
- **`ExtraBall`** est une limite d'abus : limiter à 1 par game ou 1 par bille perdue
- **Personnage 11** (`Combo 11`) est à définir — ne pas implémenter pour l'instant
