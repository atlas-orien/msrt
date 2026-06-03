#[derive(Clone, Debug)]
pub(crate) struct NoiseLcg {
    state: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NoiseConfig {
    pub(crate) corrupt_per_mille: u16,
    pub(crate) drop_byte_per_mille: u16,
    pub(crate) insert_byte_per_mille: u16,
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

    fn next_u16(&mut self) -> u16 {
        u16::from(self.next_byte()) << 8 | u16::from(self.next_byte())
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
        let roll = self.next_u16() % 1000;
        let corrupt_end = config.corrupt_per_mille;
        let drop_end = corrupt_end.saturating_add(config.drop_byte_per_mille);
        let insert_end = drop_end.saturating_add(config.insert_byte_per_mille);

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

pub(crate) fn parse_percent_per_mille(value: String, name: &str) -> Result<u16, String> {
    let percent = value
        .parse::<f64>()
        .map_err(|error| format!("invalid {name}: {error}"))?;

    if !(0.0..=100.0).contains(&percent) {
        return Err(format!("{name} must be between 0 and 100"));
    }

    Ok((percent * 10.0).round() as u16)
}

pub(crate) fn format_percent(per_mille: u16) -> String {
    if per_mille.is_multiple_of(10) {
        (per_mille / 10).to_string()
    } else {
        format!("{}.{:01}", per_mille / 10, per_mille % 10)
    }
}

pub(crate) fn add_stats(total: &mut NoiseStats, delta: NoiseStats) {
    total.corrupted += delta.corrupted;
    total.dropped += delta.dropped;
    total.inserted += delta.inserted;
}

pub(crate) fn has_noise(config: NoiseConfig) -> bool {
    config.corrupt_per_mille != 0
        || config.drop_byte_per_mille != 0
        || config.insert_byte_per_mille != 0
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
