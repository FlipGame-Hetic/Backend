use crate::engine::components::health::HealthComponent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvePhase {
    Fighting,
    Transition,
    Victory,
    GameOver,
}

#[derive(Debug)]
pub struct PveState {
    pub current_boss_index: u8,
    pub endless_level: u32,
    pub boss_health: HealthComponent,
    pub phase: PvePhase,
}

impl PveState {
    pub fn new(initial_hp: u32) -> Self {
        Self {
            current_boss_index: 0,
            endless_level: 0,
            boss_health: HealthComponent::new(initial_hp),
            phase: PvePhase::Fighting,
        }
    }
}
