# memory.md — module `combo`

> Contexte pour l'IA : ce module gère **exclusivement** la détection des séquences de boutons flipper et le calcul des effets multiplicateurs associés. Il ne touche pas au score directement  il retourne un `ComboResult` que `engine/scoring.rs` applique.

**Utiliser les principes DRY & SOLID, faire du clean code.**

---

## Responsabilités

- Maintenir un **buffer circulaire** des dernières pressions de bouton (G = gauche, D = droite)
- Détecter si le buffer correspond à une **séquence de combo** définie
- Calculer le **multiplicateur et les points bonus** associés
- Gérer le **timer de validité** (une séquence trop lente ne compte pas)
- Détecter les **pénalités** (7G ou 7D = -2000 pts)
- Gérer le **combo badge unique** (séquence secrète → badge permanent)
- Détecter le **Tilt** (via `TiltDetected` event depuis `engine`)

---

## Fichiers du module

| Fichier | Rôle |
|---------|------|
| `model.rs` | Types : `ButtonPress`, `ComboDefinition`, `ComboResult`, `ComboEffect` |
| `detector.rs` | `ComboDetector` : buffer + matching + timer |
| `multiplier.rs` | Calcul et expiration du multiplicateur actif |
| `error.rs` | `ComboError` enum |
| `mod.rs` | Re-exports publics |

---

## Types attendus (`model.rs`)

```rust
// Entrée IoT
pub enum ButtonPress { Left, Right }  // G = Left, D = Right

// Définition d'un combo (table statique)
pub struct ComboDefinition {
    pub id: u8,
    pub sequence: Vec<ButtonPress>,
    pub max_duration_ms: u64,    // fenêtre de temps max pour valider
    pub bonus_pts: u32,
    pub multiplier: f32,
    pub duration_ms: u64,        // durée d'effet du multiplicateur
}

// Résultat retourné par le détecteur
pub enum ComboResult {
    Activated(ComboEffect),
    Penalty { pts: i32 },        // séquences 9 et 10 : -2000 pts
    BadgeUnlocked { badge_id: String },
    None,
}

pub struct ComboEffect {
    pub combo_id: u8,
    pub bonus_pts: u32,
    pub multiplier: f32,
    pub duration_ms: u64,
}
```

---

## Table complète des combos (à implémenter dans `detector.rs`)

> G = Left flipper, D = Right flipper

| ID | Séquence | Bonus pts | Multiplicateur | Durée effet |
|----|----------|-----------|---------------|-------------|
| 1  | G G D D | 0 | x1.2 | 1 000 ms |
| 2  | G G D D G | 0 | x1.5 | 1 500 ms |
| 3  | G G D D D D | 0 | x2.0 | 2 000 ms |
| 4  | G G D D D G | +2 000 | x1.5 | 2 000 ms |
| 5  | G G D G G D | +2 000 | x1.5 | 2 000 ms |
| 6  | G G D D G G D | +5 000 | x2.0 | 1 000 ms |
| 7  | G G D G D G G | +5 000 | x3.0 | 500 ms |
| 8  | D D D G G D G | +1 500 | x1.5 | 3 000 ms |
| 9  | D D D D D D D | **-2 000** | aucun | — |
| 10 | G G G G G G G | **-2 000** | aucun | — |
| 11 | G G D D G D D | *(à définir)* | *(à définir)* | — |
| 12 | D D D G | 0 | x2.0 | 500 ms |
| 13 | D D G | 0 | x1.5 | 500 ms |


### Combo badge unique (secrète)
**Je te dirais quand le coder**
```
Plunger → D1 D2 G1 G2 D1 G1 D1 G1
Conditions: ultis supprimés (malus+ bonus) + score >= 100 000 pts
Effet: déblocage badge permanent (attaché au joueur à vie)
```

---

## Logique du `ComboDetector` (`detector.rs`)

```
État interne :
  - buffer: VecDeque<(ButtonPress, Instant)>  ← max 10 entrées
  - combos: &'static [ComboDefinition]        ← table statique

À chaque push(button, now):
  1. Ajouter (button, now) au buffer
  2. Purger les entrées > max_duration_ms par rapport au premier élément du candidat
  3. Pour chaque combo (du plus long au plus court) :
     a. Vérifier si les N derniers éléments matchent la séquence
     b. Vérifier que le span temporel ≤ max_duration_ms du combo
     c. Si match → retourner ComboResult::Activated(...)
  4. Vérifier pénalités (7 mêmes boutons de suite)
  5. Vérifier combo badge (séquence secrète + préconditions)
  6. Retourner ComboResult::None
```

**Règle de priorité** : toujours tester les combos du plus long au plus court pour éviter les faux positifs (ex: combo 12 `DDG` ne doit pas masquer combo 2 `GGDDG`).

---

## Logique du `MultiplierState` (`multiplier.rs`)

```
État interne :
  - active: Option<(f32, Instant, Duration)>  ← (valeur, début, durée)

Méthodes :
  - apply(effect: &ComboEffect, now: Instant)  → active le multiplicateur
  - current(now: Instant) → f32               → 1.0 si expiré
  - is_expired(now: Instant) → bool
```

---

## Gestion du Tilt (`error.rs` + intégration engine)

Le tilt est géré par `engine` qui l'envoie ici. Les effets sont :
- **1er tilt** : -2 000 pts
- **2ème tilt** : -6 000 pts
- **3ème tilt** : `CheatingDetected` → plus de score enregistré pour la session

```rust
pub enum ComboError {
    #[error("buffer overflow")]
    BufferOverflow,
}
```

---

## État d'implémentation

| Fichier | Status | Prochaine action |
|---------|--------|-----------------|
| `model.rs` | ❌ Vide | Implémenter les types ci-dessus |
| `detector.rs` | ❌ Vide | Implémenter `ComboDetector` avec la table des 13 combos |
| `multiplier.rs` | ❌ Vide | Implémenter `MultiplierState` |
| `error.rs` | ❌ Vide | Ajouter `ComboError` |
| `mod.rs` | ❌ Vide | Re-exporter `ComboDetector`, `ComboResult`, `ComboEffect` |

---

## Tests à écrire

```rust
// detector.rs #[cfg(test)]
- test_combo_1_ggdd()           → ComboResult::Activated(id=1, x1.2, 1000ms)
- test_combo_9_penalty()        → ComboResult::Penalty(-2000)
- test_combo_too_slow()         → ComboResult::None (hors fenêtre de temps)
- test_longest_match_priority() → combo 2 (GGDDG) prioritaire sur combo 1 (GGDD)

// multiplier.rs #[cfg(test)]
- test_multiplier_expires()     → current() == 1.0 après expiration
- test_multiplier_active()      → current() == valeur pendant la durée
```
