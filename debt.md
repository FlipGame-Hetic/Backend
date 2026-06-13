# Technical Debt — Flipper Backend

> Analysé le 2026-06-08 sur la branche `dev/fix-debt`.  
> Classement : **Critique** → **Architecture** → **Qualité de code**.

---

## Critique — bugs potentiels en production

### TD-01 · Deux systèmes de multiplicateur déconnectés
**Fichiers :** `crates/game-logic/src/engine/states.rs`, `crates/game-logic/src/player/skills/player_bonus.rs`, `crates/game-logic/src/combo/multiplier.rs`, `crates/game-logic/src/engine/core.rs`

`GameState` expose `active_multiplier: f32` et `multiplier_expires_at: Option<Instant>`, qui sont renseignés par les skills (`DamageBoost`, `ComboMultiplier`).  
Mais le scoring des bumpers appelle `self.multiplier.current(now)` sur une instance de `MultiplierState` interne à `GameEngine` — instance qui n'est **jamais** synchronisée avec `GameState.active_multiplier`.

Conséquence : activer l'ultime d'un personnage ne change pas le multiplicateur effectivement appliqué aux bumpers. Le boost est affiché dans la réponse HTTP (`GameStateResponse.active_multiplier`) mais n'a aucun effet réel sur le score.

---

### TD-02 · `shield_active` activé mais jamais expiré
**Fichiers :** `crates/game-logic/src/player/skills/player_bonus.rs:50-53`, `crates/game-logic/src/engine/states.rs:52-53`, `crates/game-logic/src/engine/core.rs`

`BonusSkill::Shield` positionne `state.shield_active = true` et `state.shield_expires_at = Some(...)` mais aucun endroit dans l'engine ne vérifie si `shield_expires_at` est dépassé pour remettre `shield_active` à `false`.  
Le bouclier est donc permanent pour toute la durée de la partie dès qu'il est activé.

---

### TD-03 · `damage_multiplier` jamais appliqué
**Fichiers :** `crates/game-logic/src/player/skills/player_bonus.rs:58`, `crates/game-logic/src/engine/states.rs:54`

`BonusSkill::DamageBoost` écrit `state.damage_multiplier = 2.0` mais ce champ n'est **lu nulle part** dans le calcul des dégâts au boss (`boss.take_hit(damage)` dans `pve/engine.rs`).  
L'effet "boost de dégâts" de JudgeDredd est silencieusement inactif.

---

### TD-04 · `COALESCE(created_at, '')` masque des NULLs
**Fichier :** `crates/api/src/modules/scores/service.rs:24`

```sql
COALESCE(created_at, '') as created_at
```

`ScoreEntry.created_at` est typé `String` (non-optionnel) côté Rust. Si la colonne est `NULL` en base, on retourne une chaîne vide au lieu d'une erreur ou d'un `Option<String>`. Ce contournement masque un problème d'intégrité des données et confond le consommateur de l'API.

---

### TD-05 · Serde `flatten` + `tag` sur `WsMessage`
**Fichier :** `crates/shared/src/events.rs`

```rust
#[serde(tag = "dir", rename_all = "snake_case")]
pub enum WsMessage {
    Inbound { device_id: String, #[serde(flatten)] payload: InboundMessage },
    ...
}
```

`InboundMessage` utilise `#[serde(tag = "_type")]`. Combiner `flatten` avec un enum taggé est un pattern connu pour produire des erreurs de désérialisation silencieuses dans serde (issue serde#1183). Le bridge et l'API échangent tous leurs messages via ce type — une régression de désérialisation ne serait pas détectée par les tests actuels.

---

### TD-06 · `active_device_id` jamais vidé à la déconnexion du bridge
**Fichier :** `crates/api/src/modules/realtime/ws_handler.rs`

Quand un bridge se connecte, `state.active_device_id` est mis à jour (ligne ~`*id_guard = Some(device_id.clone())`). Mais quand le bridge se déconnecte (fin de `read_loop`), ce champ n'est pas remis à `None`.  
`sync_game_state_to_bridge` continue d'être appelée avec un `device_id` obsolète — les messages outbound arrivent à un bridge fantôme.

---

## Architecture — design à retravailler

### TD-07 · Logique d'orchestration de partie dupliquée en 3 endroits
**Fichiers :**
- `crates/api/src/modules/game/routes.rs` — `start_game`, `end_game`
- `crates/api/src/modules/realtime/ws_handler.rs` — `process_inbound`
- `crates/api/src/modules/screen/ws_handler.rs` — `handle_start_game`, `handle_game_event`

Les trois chemins font la même séquence : acquérir les locks engine + session dans le bon ordre, appeler `engine.process()`, dispatcher les `ScreenEnvelope`, appeler `sync_game_state_to_bridge`, sauvegarder le score en cas de game over.

Toute correction (ex : la PR #22 sur le scoring multiball) doit être appliquée dans les trois endroits. Il manque un service centralisé `GameService` qui encapsule cette logique.

---

### TD-08 · Event types en magic strings non typés
**Fichiers :** `ws_handler.rs`, `screen/ws_handler.rs`, `game-logic/engine/core.rs`

`ScreenEnvelope.event_type` est une `String`. Tout le routing repose sur des comparaisons de littéraux :

```rust
if env.event_type == "BossDefeated" { ... }
if envelope.event_type == "StartGame" { ... }
"BumperTriangle" => GameEvent::BumperTriangleHit { ... }
```

Une faute de frappe ou un renommage partiel passe silencieusement à la compilation. Une enum `ScreenEventType` éliminerait cette classe de bugs.

---

### TD-09 · Modules stub vides enregistrés dans le routeur
**Fichiers :**
- `crates/api/src/modules/auth/handler.rs` (1 octet)
- `crates/api/src/modules/auth/service.rs` (1 octet)
- `crates/api/src/modules/room/handler.rs` (1 octet)
- `crates/api/src/modules/room/model.rs` (1 octet)
- `crates/api/src/modules/room/routes.rs` (1 octet)
- `crates/api/src/modules/room/service.rs` (1 octet)

Ces fichiers sont déclarés dans `mod.rs` mais ne contiennent rien. Le module `room` en particulier ne contribue aucune route. Ce scaffolding inachevé gonfle l'arbre des modules sans valeur ajoutée.

---

### TD-10 · Config Postgres dans une appli SQLite-only
**Fichier :** `crates/api/src/config.rs:42-52`

```rust
} else if let Ok(host) = std::env::var("DATABASE_HOST") {
    let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "5432".to_owned());
    ...
    format!("postgresql://{user_enc}:{password_enc}@{host}:{port}/{name}")
}
```

La pool est `SqlitePool`. Si un opérateur définit `DATABASE_HOST`, il obtient une URL `postgresql://` que SQLite ne peut pas ouvrir — l'erreur est opaque. La logique Postgres devrait être supprimée ou la migration vers `sqlx::AnyPool` planifiée.

---

### TD-11 · `boss_hp_percent` toujours `None` dans la réponse HTTP
**Fichier :** `crates/api/src/modules/game/dto.rs:17`, `routes.rs`

`GameStateResponse.boss_hp_percent: Option<f32>` est exposé dans l'API mais son `From<game_logic::GameState>` le fixe toujours à `None`. La PVE engine n'est pas consultée. Le client reçoit un champ inutile qui signale un état incomplet.

---

### TD-12 · Endpoints de jeu sans authentification ni rate limiting
**Fichiers :** `crates/api/src/modules/game/routes.rs`, `crates/api/src/app.rs`

`POST /api/v1/game/start`, `GET /api/v1/game/state`, `POST /api/v1/game/end` n'ont aucune authentification. N'importe quel client peut démarrer ou terminer une partie. Aucune couche de rate limiting (tower-governor ou équivalent) n'est présente dans `app.rs`.

---

## Qualité de code — dette accumulée

### TD-13 · `apply_multiplier()` est du code mort
**Fichier :** `crates/game-logic/src/engine/scoring.rs:34`

```rust
pub fn apply_multiplier(base: u64, multiplier: f32) -> u64 { ... }
```

Cette fonction est définie, testée, mais jamais appelée dans le code de production. Elle sera probablement utile une fois TD-01 corrigé mais pollue le module en attendant.

---

### TD-14 · `TiltState::reset()` jamais appelé
**Fichier :** `crates/game-logic/src/engine/states.rs`

`TiltState` expose `reset()` mais cette méthode n'est appellée nulle part. Le compteur de tilt ne se réinitialise pas entre les balles : si un joueur tilte à la balle 1 et atteint le seuil "cheating detected", il reste bloqué pour toute la partie. Un reset au changement de balle (`BallLost`) est probablement le comportement attendu.

---

### TD-15 · `extra_balls` incrémenté mais jamais consommé
**Fichier :** `crates/game-logic/src/engine/states.rs:50`, `player_bonus.rs:81`

`BonusSkill::ExtraBall` incrémente `state.extra_balls` mais aucun code ne décrémente ce compteur ni ne l'utilise pour accorder une balle supplémentaire lors d'un `BallLost`. La fonctionnalité est à moitié implémentée.

---

### TD-16 · `VictoireFinale` event en français
**Fichiers :** `crates/game-logic/src/engine/pve/engine.rs:87`, `pve/events.rs:5`

```rust
envelopes.push(make_event_envelope("VictoireFinale", serde_json::Value::Null));
```

Tous les autres event types sont en anglais (`GameOver`, `BossDefeated`, `ScoreUpdate`, etc.). Cette incohérence oblige le frontend à gérer un cas spécial. À renommer en `"FinalVictory"`.

---

### TD-17 · `PveEngine::new()` hardcode `BossKind::GLaDOS`
**Fichier :** `crates/game-logic/src/engine/pve/engine.rs:20`

```rust
let kind = BossKind::GLaDOS; // hardcodé
```

`reset_to_boss(0, ...)` appelé juste après utilise `BossKind::from_index(index)` pour la cohérence. La ligne hardcodée devrait utiliser la même helper.

---

### TD-18 · `combo_table()` réalloue à chaque démarrage de partie
**Fichier :** `crates/game-logic/src/combo/detector.rs`

`ComboDetector::new()` appelle `combo_table()` qui construit un `Vec<ComboDefinition>` à partir de données entièrement statiques. Ce Vec devrait être une `static LazyLock<Vec<ComboDefinition>>` pour éviter l'allocation à chaque partie.

---

### TD-19 · `ComboResult::BadgeUnlocked` — variant sans implémentation
**Fichier :** `crates/game-logic/src/combo/model.rs`, `engine/core.rs`

Le variant existe et l'engine l'émet (`BadgeUnlocked { badge_id }` dans `process()`), mais il n'y a aucun système de badges : pas de persistance, pas de tracking, pas d'API. C'est du code mort qui crée une fausse impression de fonctionnalité.

---

### TD-20 · `player_id` sans validation
**Fichier :** `crates/api/src/modules/game/dto.rs:6`

`StartGameRequest.player_id: String` est accepté tel quel — chaîne vide, chaîne de 10 Ko, caractères spéciaux SQL. Aucune validation de longueur minimum/maximum ni de format n'est appliquée avant insertion en base.

---

### TD-21 · `character_id` inconnu silencieusement converti en RoboCop
**Fichier :** `crates/game-logic/src/player/personnages/character.rs:119`

```rust
unknown => { tracing::warn!(...); Box::new(RoboCop) }
```

L'API accepte `character_id: 255` sans retourner HTTP 400. Le client n'est pas informé de l'erreur. Devrait être validé dans le handler avant d'instancier l'engine.

---

### TD-22 · Helper `test_state()` copié-collé
**Fichiers :** `crates/api/src/modules/game/routes.rs:168`, `crates/api/tests/game_integration.rs:9`, `crates/api/src/modules/health/routes.rs:40`

La même séquence SQLite in-memory + migrate est répétée dans trois fichiers. Un helper de test partagé dans un module `tests::common` éviterait la dérive.

---

### TD-23 · `PvePhase::Transition` jamais utilisé
**Fichier :** `crates/game-logic/src/engine/pve/states.rs`

Le variant `PvePhase::Transition` est déclaré dans l'enum mais aucun code ne positionne la phase à cette valeur. C'est probablement un placeholder pour une animation de transition jamais implémentée.

---

## Récapitulatif par priorité

| ID | Sévérité | Effort | Zone |
|----|----------|--------|------|
| TD-01 | Critique | Moyen | game-logic |
| TD-02 | Critique | Faible | game-logic |
| TD-03 | Critique | Faible | game-logic |
| TD-04 | Critique | Faible | api/scores |
| TD-05 | Critique | Élevé | shared |
| TD-06 | Critique | Faible | api/realtime |
| TD-07 | Architecture | Élevé | api |
| TD-08 | Architecture | Élevé | shared + api + game-logic |
| TD-09 | Architecture | Faible | api/modules |
| TD-10 | Architecture | Faible | api/config |
| TD-11 | Architecture | Faible | api/game |
| TD-12 | Architecture | Moyen | api |
| TD-13 | Qualité | Faible | game-logic |
| TD-14 | Qualité | Faible | game-logic |
| TD-15 | Qualité | Moyen | game-logic |
| TD-16 | Qualité | Trivial | game-logic |
| TD-17 | Qualité | Trivial | game-logic |
| TD-18 | Qualité | Faible | game-logic |
| TD-19 | Qualité | Faible | game-logic |
| TD-20 | Qualité | Faible | api |
| TD-21 | Qualité | Faible | api |
| TD-22 | Qualité | Faible | api/tests |
| TD-23 | Qualité | Trivial | game-logic |
