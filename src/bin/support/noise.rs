#[derive(Clone, Debug)]
pub(crate) struct NoiseLcg {
    state: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NoiseConfig {
    pub(crate) corrupt_percent: u8,
    pub(crate) drop_byte_percent: u8,
    pub(crate) insert_byte_percent: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NoiseStats {
    pub(crate) corrupted: usize,
    pub(crate) dropped: usize,
    pub(crate) inserted: usize,
}

impl NoiseLcg {
    pub(crate) const fn new() -> Self {
        Self { state: 0x4d535254 }
    }

    pub(crate) fn next_byte(&mut self) -> u8 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.state >> 24) as u8
    }

    pub(crate) fn mutate_bytes(
        &mut self,
        bytes: &[u8],
        config: NoiseConfig,
    ) -> (Vec<u8>, NoiseStats) {
        let mut out = bytes.to_vec();
        let mut stats = NoiseStats::default();

        match self.choose_action(config) {
            NoiseAction::Corrupt if !out.is_empty() => {
                let pos = self.next_byte() as usize % out.len();
                out[pos] ^= self.next_byte() | 1;
                stats.corrupted += 1;
            }
            NoiseAction::Drop if !out.is_empty() => {
                let pos = self.next_byte() as usize % out.len();
                out.remove(pos);
                stats.dropped += 1;
            }
            NoiseAction::Insert => {
                let pos = if out.is_empty() {
                    0
                } else {
                    self.next_byte() as usize % (out.len() + 1)
                };
                out.insert(pos, self.next_byte());
                stats.inserted += 1;
            }
            NoiseAction::None | NoiseAction::Corrupt | NoiseAction::Drop => {}
        }

        (out, stats)
    }

    fn choose_action(&mut self, config: NoiseConfig) -> NoiseAction {
        let roll = self.next_byte() % 100;
        let corrupt_end = config.corrupt_percent;
        let drop_end = corrupt_end.saturating_add(config.drop_byte_percent);
        let insert_end = drop_end.saturating_add(config.insert_byte_percent);

        if roll < corrupt_end {
            NoiseAction::Corrupt
        } else if roll < drop_end {
            NoiseAction::Drop
        } else if roll < insert_end {
            NoiseAction::Insert
        } else {
            NoiseAction::None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoiseAction {
    None,
    Corrupt,
    Drop,
    Insert,
}

pub(crate) fn validate_percent(percent: u8, name: &str) -> Result<(), String> {
    if percent > 100 {
        return Err(format!("{name} must be <= 100"));
    }

    Ok(())
}

pub(crate) fn add_stats(total: &mut NoiseStats, delta: NoiseStats) {
    total.corrupted += delta.corrupted;
    total.dropped += delta.dropped;
    total.inserted += delta.inserted;
}

pub(crate) fn has_noise(config: NoiseConfig) -> bool {
    config.corrupt_percent != 0 || config.drop_byte_percent != 0 || config.insert_byte_percent != 0
}

#[allow(dead_code)]
pub(crate) fn make_noise_bytes(len: usize) -> Vec<u8> {
    let mut lcg = NoiseLcg::new();
    (0..len).map(|_| lcg.next_byte()).collect()
}

pub(crate) fn mutate_or_copy(
    noise_state: &mut NoiseLcg,
    bytes: &[u8],
    config: NoiseConfig,
) -> (Vec<u8>, NoiseStats) {
    if !has_noise(config) {
        return (bytes.to_vec(), NoiseStats::default());
    }

    noise_state.mutate_bytes(bytes, config)
}
