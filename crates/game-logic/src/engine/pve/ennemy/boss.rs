use crate::engine::components::health::HealthComponent;
use crate::engine::pve::difficulty::scale_hp;
use crate::engine::pve::ennemy::kind::BossKind;

pub struct Boss {
    pub kind: BossKind,
    pub health: HealthComponent,
    pub malus_active: bool,
}

impl Boss {
    pub fn new(kind: BossKind, difficulty_index: u8) -> Self {
        let hp = scale_hp(kind.base_hp(), difficulty_index, 0);
        Self {
            kind,
            health: HealthComponent::new(hp),
            malus_active: false,
        }
    }

    pub fn new_endless(kind: BossKind, endless_level: u32) -> Self {
        let hp = scale_hp(kind.base_hp(), 3, endless_level);
        Self {
            kind,
            health: HealthComponent::new(hp),
            malus_active: false,
        }
    }

    /// Returns true if the boss died from this hit.
    pub fn take_hit(&mut self, damage: u32) -> bool {
        self.health.take_damage(damage);
        self.health.is_dead()
    }
}
