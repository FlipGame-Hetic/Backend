# memory.md — module `engine`

> Contexte pour l'IA : ce module est le **coordinateur central** du jeu. Il orchestre la machine à états, le scoring, les événements IoT et délègue aux sous-modules `pve`/`pvp`. Il reçoit des `GameEvent` et retourne des `Vec<ScreenEnvelope>` à dispatcher par `api`.

**Utiliser les principes DRY & SOLID, faire du clean code.**

---

## Responsabilités

- Tenir à jour le `GameState` global (phase, score, vies, multiplicateur actif)
- Recevoir et dispatcher les `GameEvent` vers les bons sous-systèmes
- Calculer le score (bumpers, bonus timer, malus tilt...)
- Déclencher le `ComboDetector` du module `combo` sur chaque `ButtonPressed`
- Gérer la progression PvE (3 boss séquentiels + Endless)
- Produire les `ScreenEnvelope` à envoyer aux 3 écrans

---

## Fichiers du module

| Fichier | Rôle |
|---------|------|
| `events.rs` | `enum GameEvent` — tous les events possibles (IoT + internes) |
| `states.rs` | `GamePhase`, `GameState` struct, `TiltState` |
| `scoring.rs` | Fonctions de calcul de score (bumper, timer bonus, apply multiplier) |
| `core.rs` | `GameEngine` principal — reçoit events, orchestre tout |
| `config.rs` | `GameConfig` — constantes (nb vies, HP boss, seuils) |
| `mod.rs` | Re-exports publics |
| `components/health.rs` | `HealthComponent` réutilisable (joueur + boss) |
| `pve/engine.rs` | `PveEngine` — gère les 3 boss + Endless |
| `pve/states.rs` | `PveState` — boss actuel, HP, phase de transition |
| `pve/events.rs` | Events spécifiques PvE (BossDefeated, PhaseTransition) |
| `pve/difficulty.rs` | Scaling de difficulté entre boss |
| `pve/ennemy/boss.rs` | `Boss` struct (nom, HP, malus actifs) |
| `pve/ennemy/kind.rs` | `BossKind` enum (GLaDOS, HAL9000, AUTO, etc.) |
| `pvp/` | *(Non implémenté — Phase 5)* |

---

## Types attendus (`events.rs`)

```rust
pub enum GameEvent {
    // Depuis IoT (via shared::events → InboundMessage)
    StartGame { player_id: String },
    EndGame,
    BallLaunched,
    BallLost,
    BallSaved,
    ButtonPressed { side: ButtonSide },   // Left ou Right
    BumperHit { pts: u32 },               // +100 pts
    BumperTriangleHit { pts: u32 },       // +200 pts
    BumperCombo { count: u32 },           // 10+ touches → MultiballRingBounces
    PortalUsed,
    TiltDetected,
    LifeUp,
    MultiballWin,
    ScoreMultiplierActivated,
    UltimateActivated { player_id: String },

    // Internes (générés par game-logic lui-même)
    ComboActivated(crate::combo::ComboEffect),
    BossDefeated { boss_id: u8 },
    GameOverTriggered { reason: GameOverReason },
    TimerBonusCheck,                      // tick pour BonusGameTimerMultiplier
}

pub enum GameOverReason { NoLivesLeft, PlayerQuit }
pub enum ButtonSide { Left, Right }
```

---

## Types attendus (`states.rs`)

```rust
pub enum GamePhase {
    Idle,
    InGame,
    GameOver,
}

pub struct GameState {
    pub phase: GamePhase,
    pub score: u64,
    pub lives: u8,                    // défaut : 3
    pub active_multiplier: f32,       // défaut : 1.0
    pub multiplier_expires_at: Option<Instant>,
    pub tilt_state: TiltState,
    pub balls_lost_since_start: u32,  // pour BonusGameTimerMultiplier
    pub session_start: Option<Instant>,
    pub cheating_detected: bool,      // 3ème tilt → ne plus compter les points
    pub extra_balls: u8,              // balles supplémentaires actives
}

pub struct TiltState {
    pub count: u8,   // 0, 1, 2, 3
}

impl TiltState {
    // Retourne le malus à appliquer et si cheating détecté
    pub fn on_tilt(&mut self) -> TiltEffect { ... }
}

pub enum TiltEffect {
    Penalty(i32),           // 1er: -2000, 2ème: -6000
    CheatingDetected,       // 3ème
}
```

---

## Scoring (`scoring.rs`)

### Règles de scoring à implémenter

| Événement | Formule |
|-----------|---------|
| `BumperHit` | `+100 * multiplier` |
| `BumperTriangleHit` | `+200 * multiplier` |
| `BumperCombo` (≥10 touches) | `MultiballWin` déclenché |
| `PortalUsed` | Points bonus (montant à définir) |
| `BonusGameTimerMultiplier` | Si 0 billes perdues depuis début, au bout de 60s : `+500 pts * 1.5` |
| `ComboActivated` | `+bonus_pts + score_actuel * multiplier` pendant `duration_ms` |
| `TiltDetected (1er)` | `-2 000 pts` |
| `TiltDetected (2ème)` | `-6 000 pts` |
| `TiltDetected (3ème)` | `CheatingDetected` : ne plus enregistrer le score |
| `EndGame` stats | `max_multiplier_used`, `nb_multiball`, `missions_reussies`, `boss_vaincus` |

---

## PvE Engine (`pve/engine.rs`)

### Séquence de jeu PvE (UC-04)

```
1. StartGame → initialise : 3 vies, HP boss 1, score 0, jauge ultime 0
2. Le joueur joue → events BumperHit, BumperTriangleHit → DamagesBoss
3. HP boss → 0 → BossDefeated → transition screen → boss 2 (+ HP, + malus)
4. Boss 2 vaincu → boss 3 (encore + HP, + malus)
5. Boss 3 vaincu → VictoireFinale → calcul XP
6. Si plus de vies → GameOver → XP partiel
7. Mode Endless → boucle infinie de boss avec scaling continu
```

### Infos Boss

| Boss | ID | HP de base | Malus infligé |
|------|-----|-----------|--------------|
| GLaDOS | 0 | 500 | Décale bille (ModifyBounce) |
| HAL 9000 | 1 | 800 | Tache d'encre (InkBlot) |
| AUTO | 2 | 1200 | Black Hole aléatoire |

Les HP scalent entre boss : `hp_boss_n = hp_base * difficulty_multiplier(n)`.

---

## Intégration avec `combo`

Dans `core.rs` / `GameEngine` :

```rust
// À chaque ButtonPressed :
let result = self.combo_detector.push(side.into(), Instant::now());
match result {
    ComboResult::Activated(effect) => {
        self.state.active_multiplier = effect.multiplier;
        self.state.multiplier_expires_at = Some(now + Duration::from_millis(effect.duration_ms));
        self.state.score += effect.bonus_pts as u64;
        envelopes.push(screen_envelope_combo_activated(&effect));
    }
    ComboResult::Penalty { pts } => { self.state.score = self.state.score.saturating_sub(pts as u64); }
    ComboResult::BadgeUnlocked { badge_id } => { envelopes.push(screen_envelope_badge(&badge_id)); }
    ComboResult::None => {}
}
```

---

## État d'implémentation

| Fichier | Status | Prochaine action |
|---------|--------|-----------------|
| `events.rs` | ❌ Vide | Implémenter `GameEvent` complet |
| `states.rs` | Commentaire uniquement | Implémenter `GamePhase`, `GameState`, `TiltState` |
| `scoring.rs` | ❌ Vide | Implémenter les fonctions de scoring |
| `core.rs` | ❌ Vide | Implémenter `GameEngine` |
| `config.rs` | ❌ Vide | Implémenter `GameConfig` (constantes) |
| `components/health.rs` | ❌ Vide | Implémenter `HealthComponent` |
| `pve/engine.rs` | ❌ Vide | Implémenter `PveEngine` |
| `pve/states.rs` | ❌ Vide | Implémenter `PveState` |
| `pve/events.rs` | ❌ Vide | Implémenter events PvE |
| `pve/difficulty.rs` | ❌ Vide | Scaling difficulté |
| `pve/ennemy/boss.rs` | ❌ Vide | Struct `Boss` |
| `pve/ennemy/kind.rs` | ❌ Vide | Enum `BossKind` |
| `pvp/` | ❌ Non créé | validation par l'user |

---

## Conventions dans ce module

- `GameEngine::process(event: GameEvent) -> Vec<ScreenEnvelope>` est l'interface publique principale
- Les envelopes écrans ciblent : `ScreenTarget::Front`, `ScreenTarget::Back`, `ScreenTarget::Dmd`
- Jamais de side-effect réseau ici — retourner des envelopes, c'est `api` qui dispatch
- `config.rs` contient toutes les constantes magiques (pas de magic numbers dans le code)
```rust
// config.rs
pub const DEFAULT_LIVES: u8 = 3;
pub const BUMPER_SCORE: u32 = 100;
pub const BUMPER_TRIANGLE_SCORE: u32 = 200;
pub const MULTIBALL_RING_THRESHOLD: u32 = 10;
pub const TIMER_BONUS_SECONDS: u64 = 60;
pub const TIMER_BONUS_SCORE: u32 = 500;
pub const TIMER_BONUS_MULTIPLIER: f32 = 1.5;
pub const TILT_PENALTY_1: i32 = -2_000;
pub const TILT_PENALTY_2: i32 = -6_000;
```

---

## Tests à écrire

```rust
// states.rs #[cfg(test)]
- test_tilt_first()    → Penalty(-2000)
- test_tilt_second()   → Penalty(-6000)
- test_tilt_third()    → CheatingDetected

// scoring.rs #[cfg(test)]
- test_bumper_with_multiplier()       → 100 * 1.5 = 150
- test_timer_bonus_no_lives_lost()    → +500 + *1.5
- test_timer_bonus_with_lives_lost()  → aucun bonus

// pve/engine.rs #[cfg(test)]
- test_boss_defeated_transitions_to_next()
- test_all_bosses_defeated_triggers_victory()
- test_no_lives_triggers_game_over()
```
