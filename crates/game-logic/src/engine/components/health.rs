#[derive(Debug)]
pub struct HealthComponent {
    pub max: u32,
    pub current: u32,
}

impl HealthComponent {
    pub fn new(max: u32) -> Self {
        Self { max, current: max }
    }

    pub fn take_damage(&mut self, amount: u32) {
        self.current = self.current.saturating_sub(amount);
    }

    pub fn is_dead(&self) -> bool {
        self.current == 0
    }

    pub fn reset(&mut self) {
        self.current = self.max;
    }

    pub fn reset_with_new_max(&mut self, new_max: u32) {
        self.max = new_max;
        self.current = new_max;
    }

    pub fn percentage(&self) -> f32 {
        if self.max == 0 {
            return 0.0;
        }
        self.current as f32 / self.max as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_reduces_hp() {
        let mut hp = HealthComponent::new(100);
        hp.take_damage(30);
        assert_eq!(hp.current, 70);
    }

    #[test]
    fn test_saturating_sub() {
        let mut hp = HealthComponent::new(10);
        hp.take_damage(999);
        assert_eq!(hp.current, 0);
        assert!(hp.is_dead());
    }

    #[test]
    fn test_reset() {
        let mut hp = HealthComponent::new(200);
        hp.take_damage(100);
        hp.reset();
        assert_eq!(hp.current, 200);
    }

    #[test]
    fn test_percentage() {
        let mut hp = HealthComponent::new(100);
        hp.take_damage(25);
        assert!((hp.percentage() - 0.75).abs() < f32::EPSILON);
    }
}
