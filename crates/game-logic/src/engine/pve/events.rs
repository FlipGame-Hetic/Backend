#[derive(Debug, Clone)]
pub enum PveEvent {
    BossDefeated { boss_id: u8 },
    PhaseTransition { from: u8, to: u8 },
    VictoireFinale,
    EndlessScaling { level: u32 },
}
