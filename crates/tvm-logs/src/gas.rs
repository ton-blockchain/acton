use crate::parser::VmLine;

pub const DEFAULT_INITIAL_GAS: usize = 1_000_000;

#[derive(Debug, Clone)]
pub struct GasTracker {
    gas_base: usize,
    gas_consumed: usize,
    gas_remaining: usize,
}

impl GasTracker {
    #[must_use]
    pub const fn new(initial_gas: usize) -> Self {
        Self {
            gas_base: initial_gas,
            gas_consumed: 0,
            gas_remaining: initial_gas,
        }
    }

    #[must_use]
    pub fn update(&mut self, line: &VmLine<'_>) -> Option<usize> {
        match line {
            VmLine::VmGasRemaining { gas } => {
                let new_gas = gas.parse::<usize>().unwrap_or(self.gas_remaining);
                let new_gas_consumed = self.gas_base.saturating_sub(new_gas);
                let gas_cost = new_gas_consumed.saturating_sub(self.gas_consumed);
                self.gas_consumed = new_gas_consumed;
                self.gas_remaining = new_gas;
                Some(gas_cost)
            }
            VmLine::VmLimitChanged { limit } => {
                if let Ok(new_limit) = limit.parse::<usize>() {
                    self.gas_base = new_limit;
                    self.gas_remaining = self.gas_base.saturating_sub(self.gas_consumed);
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_lines;

    #[test]
    fn tracks_instruction_costs_across_limit_changes() {
        let lines =
            parse_lines("gas remaining: 995\nchanging gas limit to 2000\ngas remaining: 1988\n");
        let mut tracker = GasTracker::new(1000);
        let costs = lines
            .filter_map(Result::ok)
            .filter_map(|line| tracker.update(&line))
            .collect::<Vec<_>>();

        assert_eq!(costs, vec![5, 7]);
    }
}
