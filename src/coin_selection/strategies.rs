use serde::{Deserialize, Serialize};

use super::Strategy;

/// Weight multipliers for each scoring dimension.
///
/// Each strategy profile adjusts these weights to emphasize
/// different aspects of coin selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyWeights {
    /// Weight for script-type preference (P2TR > P2WPKH > ...)
    pub script_type: f64,
    /// Weight for amount decorrelation (privacy)
    pub decorrelation: f64,
    /// Weight for fee efficiency
    pub fee_efficiency: f64,
    /// Weight for dust consolidation opportunity
    pub consolidation: f64,
    /// Weight for UTXO age scoring
    pub age: f64,
}

impl StrategyWeights {
    /// Get the weight configuration for a given strategy.
    pub fn for_strategy(strategy: Strategy) -> Self {
        match strategy {
            Strategy::Balanced => Self::balanced(),
            Strategy::PrivacyMax => Self::privacy_max(),
            Strategy::FeeMin => Self::fee_min(),
            Strategy::Consolidate => Self::consolidate(),
        }
    }

    /// Balanced: equal emphasis across all dimensions.
    /// Total max score: 30 + 25 + 20 + 15 + 10 = 100
    fn balanced() -> Self {
        StrategyWeights {
            script_type: 30.0,
            decorrelation: 25.0,
            fee_efficiency: 20.0,
            consolidation: 15.0,
            age: 10.0,
        }
    }

    /// Privacy-max: heavily favor privacy-enhancing properties.
    fn privacy_max() -> Self {
        StrategyWeights {
            script_type: 35.0,
            decorrelation: 40.0,
            fee_efficiency: 5.0,
            consolidation: 5.0,
            age: 15.0,
        }
    }

    /// Fee-min: minimize the fee impact of receiver inputs.
    fn fee_min() -> Self {
        StrategyWeights {
            script_type: 25.0,
            decorrelation: 10.0,
            fee_efficiency: 45.0,
            consolidation: 10.0,
            age: 10.0,
        }
    }

    /// Consolidate: maximize UTXO cleanup during low-fee periods.
    fn consolidate() -> Self {
        StrategyWeights {
            script_type: 10.0,
            decorrelation: 10.0,
            fee_efficiency: 15.0,
            consolidation: 55.0,
            age: 10.0,
        }
    }

    /// Sum of all weights (for normalization if needed).
    pub fn total(&self) -> f64 {
        self.script_type
            + self.decorrelation
            + self.fee_efficiency
            + self.consolidation
            + self.age
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_strategies_sum_to_100() {
        for strategy in [
            Strategy::Balanced,
            Strategy::PrivacyMax,
            Strategy::FeeMin,
            Strategy::Consolidate,
        ] {
            let weights = StrategyWeights::for_strategy(strategy);
            assert!(
                (weights.total() - 100.0).abs() < f64::EPSILON,
                "{:?} weights sum to {} instead of 100",
                strategy,
                weights.total()
            );
        }
    }

    #[test]
    fn test_privacy_max_emphasizes_decorrelation() {
        let balanced = StrategyWeights::for_strategy(Strategy::Balanced);
        let privacy = StrategyWeights::for_strategy(Strategy::PrivacyMax);
        assert!(privacy.decorrelation > balanced.decorrelation);
    }

    #[test]
    fn test_fee_min_emphasizes_efficiency() {
        let balanced = StrategyWeights::for_strategy(Strategy::Balanced);
        let fee_min = StrategyWeights::for_strategy(Strategy::FeeMin);
        assert!(fee_min.fee_efficiency > balanced.fee_efficiency);
    }

    #[test]
    fn test_consolidate_emphasizes_consolidation() {
        let balanced = StrategyWeights::for_strategy(Strategy::Balanced);
        let consolidate = StrategyWeights::for_strategy(Strategy::Consolidate);
        assert!(consolidate.consolidation > balanced.consolidation);
    }
}
