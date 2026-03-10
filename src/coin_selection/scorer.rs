use bitcoin::Amount;

use crate::rpc::WalletUtxo;

use super::decorrelation;
use super::script_types::ScriptType;
use super::strategies::StrategyWeights;

/// A UTXO with its computed selection score.
#[derive(Debug, Clone)]
pub struct ScoredUtxo {
    pub utxo: WalletUtxo,
    pub score: f64,
    pub script_type: ScriptType,
    pub breakdown: ScoreBreakdown,
}

/// Detailed breakdown of how the score was computed.
#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    pub script_type_score: f64,
    pub decorrelation_score: f64,
    pub fee_efficiency_score: f64,
    pub consolidation_score: f64,
    pub age_score: f64,
}

/// Dust consolidation threshold in satoshis.
/// UTXOs below this value are considered candidates for consolidation.
const DUST_CONSOLIDATION_THRESHOLD: u64 = 50_000;

/// Fee rate threshold (sat/vB) below which consolidation is encouraged.
const LOW_FEE_THRESHOLD: f64 = 5.0;

/// Score a single UTXO across all dimensions using the given strategy weights.
pub fn score_utxo(
    utxo: &WalletUtxo,
    payment_amount: Amount,
    feerate_sat_per_vb: f64,
    weights: &StrategyWeights,
) -> ScoredUtxo {
    let script_type = ScriptType::detect(&utxo.script_pubkey);

    let script_type_score = score_script_type(script_type);
    let decorrelation_score = score_decorrelation(utxo.amount, payment_amount);
    let fee_efficiency_score = score_fee_efficiency(utxo.amount, script_type, feerate_sat_per_vb);
    let consolidation_score = score_consolidation(utxo.amount, feerate_sat_per_vb);
    let age_score = score_age(utxo.confirmations);

    // Apply strategy weights
    let total = (script_type_score * weights.script_type)
        + (decorrelation_score * weights.decorrelation)
        + (fee_efficiency_score * weights.fee_efficiency)
        + (consolidation_score * weights.consolidation)
        + (age_score * weights.age);

    ScoredUtxo {
        utxo: utxo.clone(),
        score: total,
        script_type,
        breakdown: ScoreBreakdown {
            script_type_score,
            decorrelation_score,
            fee_efficiency_score,
            consolidation_score,
            age_score,
        },
    }
}

/// Score script type preference: P2TR > P2WPKH > P2SH > P2PKH.
/// Returns 0.0 to 1.0.
fn score_script_type(script_type: ScriptType) -> f64 {
    match script_type {
        ScriptType::P2TR => 1.0,
        ScriptType::P2WPKH => 0.67,
        ScriptType::P2SH => 0.33,
        ScriptType::P2PKH => 0.1,
        ScriptType::Unknown => 0.0,
    }
}

/// Score amount decorrelation — how well this UTXO obscures the payment.
/// Returns 0.0 to 1.0.
fn score_decorrelation(utxo_amount: Amount, payment_amount: Amount) -> f64 {
    decorrelation::measure_output_ambiguity(utxo_amount, payment_amount)
}

/// Score fee efficiency — how cheap this input is to spend relative to its value.
/// Returns 0.0 to 1.0.
fn score_fee_efficiency(
    utxo_amount: Amount,
    script_type: ScriptType,
    feerate_sat_per_vb: f64,
) -> f64 {
    let spending_cost = script_type.spending_cost_sats(feerate_sat_per_vb);
    let value = utxo_amount.to_sat();

    if value == 0 {
        return 0.0;
    }

    let cost_ratio = spending_cost as f64 / value as f64;
    (1.0 - cost_ratio).clamp(0.0, 1.0)
}

/// Score consolidation opportunity.
/// Small UTXOs during low-fee periods get high scores.
/// Returns 0.0 to 1.0.
fn score_consolidation(utxo_amount: Amount, feerate_sat_per_vb: f64) -> f64 {
    let value = utxo_amount.to_sat();

    if value < DUST_CONSOLIDATION_THRESHOLD && feerate_sat_per_vb < LOW_FEE_THRESHOLD {
        // Great consolidation candidate: small UTXO + low fees
        1.0
    } else if value < DUST_CONSOLIDATION_THRESHOLD {
        // Small UTXO but fees are high — moderate candidate
        0.5
    } else if feerate_sat_per_vb < LOW_FEE_THRESHOLD {
        // Low fees but UTXO isn't small — slight benefit
        0.3
    } else {
        // Normal UTXO, normal fees — no consolidation benefit
        0.1
    }
}

/// Score UTXO age using a Gaussian distribution centered around ~1 day (144 blocks).
///
/// Very fresh UTXOs may link to identifiable recent transactions.
/// Very old UTXOs may be recognizable as long-dormant holdings.
/// Moderate-age UTXOs are ideal.
///
/// Returns 0.0 to 1.0.
fn score_age(confirmations: u32) -> f64 {
    let confs = confirmations as f64;
    let mean = 144.0; // ~1 day in blocks
    let stddev = 72.0; // ~12 hours

    // Gaussian: exp(-0.5 * ((x - mean) / stddev)^2)
    let z = (confs - mean) / stddev;
    (-0.5 * z * z).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{OutPoint, ScriptBuf, Txid};
    use std::str::FromStr;

    fn make_utxo(amount_sats: u64, confirmations: u32, script_hex: &str) -> WalletUtxo {
        WalletUtxo {
            outpoint: OutPoint::new(
                Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .unwrap(),
                0,
            ),
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            vout: 0,
            amount: Amount::from_sat(amount_sats),
            confirmations,
            script_pubkey: ScriptBuf::from_hex(script_hex).unwrap_or_default(),
            address: String::new(),
            spendable: true,
            solvable: true,
        }
    }

    #[test]
    fn test_p2tr_scores_higher_than_p2wpkh() {
        assert!(score_script_type(ScriptType::P2TR) > score_script_type(ScriptType::P2WPKH));
    }

    #[test]
    fn test_fee_efficiency_high_value_utxo() {
        // 1 BTC UTXO at 1 sat/vB — very efficient
        let score = score_fee_efficiency(
            Amount::from_sat(100_000_000),
            ScriptType::P2TR,
            1.0,
        );
        assert!(score > 0.99, "High-value UTXO should be very efficient: {}", score);
    }

    #[test]
    fn test_fee_efficiency_dust_utxo() {
        // 600 sat UTXO at 10 sat/vB — expensive to spend
        let score = score_fee_efficiency(
            Amount::from_sat(600),
            ScriptType::P2TR,
            10.0,
        );
        // 57.5 * 10 = 575 cost for 600 value => 1 - 575/600 ≈ 0.04
        assert!(score < 0.1, "Dust UTXO should have low efficiency: {}", score);
    }

    #[test]
    fn test_consolidation_small_low_fee() {
        let score = score_consolidation(Amount::from_sat(10_000), 1.0);
        assert_eq!(score, 1.0, "Small UTXO + low fee = best consolidation");
    }

    #[test]
    fn test_consolidation_large_high_fee() {
        let score = score_consolidation(Amount::from_sat(1_000_000), 20.0);
        assert_eq!(score, 0.1, "Large UTXO + high fee = no consolidation benefit");
    }

    #[test]
    fn test_age_optimal() {
        // Exactly at the mean (144 blocks) should score highest
        let optimal = score_age(144);
        let too_fresh = score_age(1);
        let too_old = score_age(10_000);

        assert!(optimal > too_fresh, "Optimal age should beat fresh");
        assert!(optimal > too_old, "Optimal age should beat old");
        assert!((optimal - 1.0).abs() < f64::EPSILON, "At mean should score ~1.0");
    }

    #[test]
    fn test_full_scoring() {
        let weights = super::super::strategies::StrategyWeights::for_strategy(
            super::super::Strategy::Balanced,
        );

        // P2TR UTXO, 0.5 BTC, 144 confirmations
        let p2tr_hex = "5120".to_string() + &"aa".repeat(32);
        let utxo = make_utxo(50_000_000, 144, &p2tr_hex);

        let scored = score_utxo(
            &utxo,
            Amount::from_sat(100_000),
            1.0,
            &weights,
        );

        assert!(scored.score > 0.0, "Score should be positive");
        assert_eq!(scored.script_type, ScriptType::P2TR);
    }
}
