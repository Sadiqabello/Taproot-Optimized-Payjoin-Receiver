use bitcoin::Amount;

/// Measure how well a receiver UTXO obscures the payment amount.
///
/// The goal is to ensure that no output in the resulting transaction
/// directly reveals the payment amount. A high decorrelation score means
/// the UTXO value makes it harder for an observer to identify which
/// output is the payment and which is change.
///
/// Returns a score between 0.0 (no privacy benefit) and 1.0 (excellent).
pub fn measure_output_ambiguity(
    receiver_utxo_value: Amount,
    payment_amount: Amount,
) -> f64 {
    let utxo_sats = receiver_utxo_value.to_sat() as f64;
    let payment_sats = payment_amount.to_sat() as f64;

    if payment_sats == 0.0 || utxo_sats == 0.0 {
        return 0.0;
    }

    // In a Payjoin, the receiver adds an input and the outputs are restructured.
    // The combined value (payment + receiver_utxo) will be split into outputs.
    //
    // We want to avoid situations where:
    // 1. An output exactly equals the payment amount (ratio_score)
    // 2. The receiver input value is trivially small or a round number (round_score)
    // 3. The output split is obviously asymmetric (balance_score)

    let ratio_score = compute_ratio_score(utxo_sats, payment_sats);
    let round_score = compute_round_number_penalty(utxo_sats);
    let balance_score = compute_balance_score(utxo_sats, payment_sats);

    // Weighted combination
    let score = (ratio_score * 0.5) + (round_score * 0.2) + (balance_score * 0.3);
    score.clamp(0.0, 1.0)
}

/// Score based on how well the UTXO value avoids creating outputs
/// that equal the payment amount.
///
/// If the UTXO is close in value to the payment, an observer could
/// identify the payment output by process of elimination.
fn compute_ratio_score(utxo_sats: f64, payment_sats: f64) -> f64 {
    let ratio = utxo_sats / payment_sats;

    // Best: ratio between 0.3 and 3.0 (not too similar, not too different)
    // Worst: ratio very close to 1.0 (UTXO ≈ payment) or extremely far
    if ratio < 0.01 || ratio > 100.0 {
        // Extreme mismatch — trivially identifiable
        return 0.1;
    }

    // Penalize when UTXO value is very close to payment amount
    let closeness = (ratio - 1.0).abs();
    if closeness < 0.05 {
        // Within 5% — very poor for privacy
        return 0.2;
    }

    // Sweet spot: different enough to create ambiguous outputs
    if (0.3..=3.0).contains(&ratio) {
        // Good range — outputs will be of comparable size
        let distance_from_one = (ratio - 1.0).abs();
        return (0.5 + distance_from_one * 0.5).min(1.0);
    }

    // Outside sweet spot but not extreme
    0.4
}

/// Penalize round-number UTXOs that stand out on-chain.
fn compute_round_number_penalty(utxo_sats: f64) -> f64 {
    let utxo = utxo_sats as u64;

    // Check if the UTXO is a "round" number in BTC terms
    // 1 BTC = 100_000_000 sats
    let round_thresholds: &[(u64, f64)] = &[
        (100_000_000, 0.2), // Exact BTC amounts
        (10_000_000, 0.3),  // 0.1 BTC
        (1_000_000, 0.4),   // 0.01 BTC
        (100_000, 0.6),     // 100k sats
        (10_000, 0.7),      // 10k sats
    ];

    for &(threshold, penalty_score) in round_thresholds {
        if utxo % threshold == 0 {
            return penalty_score;
        }
    }

    // Non-round amounts are better for privacy
    1.0
}

/// Score based on how balanced the output split would be.
/// More balanced outputs create more ambiguity about which is payment vs change.
fn compute_balance_score(utxo_sats: f64, payment_sats: f64) -> f64 {
    let total = utxo_sats + payment_sats;
    if total == 0.0 {
        return 0.0;
    }

    // In a Payjoin, receiver gets (payment + utxo_value) split across outputs.
    // The more balanced the potential output values, the harder to distinguish.
    let smaller = utxo_sats.min(payment_sats);
    let ratio = smaller / total;

    // ratio of 0.5 means perfectly balanced — ideal
    // ratio near 0 means one side dominates — poor
    (ratio * 2.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_values() {
        assert_eq!(
            measure_output_ambiguity(Amount::ZERO, Amount::from_sat(1000)),
            0.0
        );
        assert_eq!(
            measure_output_ambiguity(Amount::from_sat(1000), Amount::ZERO),
            0.0
        );
    }

    #[test]
    fn test_similar_amounts_score_lower() {
        // UTXO very close to payment — poor privacy
        let close = measure_output_ambiguity(
            Amount::from_sat(100_000),
            Amount::from_sat(100_500),
        );
        // UTXO different from payment — better privacy
        let different = measure_output_ambiguity(
            Amount::from_sat(250_000),
            Amount::from_sat(100_000),
        );

        assert!(
            different > close,
            "Different amounts ({}) should score higher than similar ({})",
            different,
            close,
        );
    }

    #[test]
    fn test_round_number_penalty() {
        // Round BTC amount
        let round = compute_round_number_penalty(100_000_000.0); // 1 BTC
        // Irregular amount
        let irregular = compute_round_number_penalty(73_421_589.0);

        assert!(
            irregular > round,
            "Irregular ({}) should score higher than round ({})",
            irregular,
            round,
        );
    }

    #[test]
    fn test_balance_score() {
        // Perfectly balanced
        let balanced = compute_balance_score(100_000.0, 100_000.0);
        // Very unbalanced
        let unbalanced = compute_balance_score(1_000.0, 100_000.0);

        assert!(
            balanced > unbalanced,
            "Balanced ({}) should score higher than unbalanced ({})",
            balanced,
            unbalanced,
        );
    }

    #[test]
    fn test_score_in_range() {
        let score = measure_output_ambiguity(
            Amount::from_sat(150_000),
            Amount::from_sat(100_000),
        );
        assert!((0.0..=1.0).contains(&score), "Score {} out of range", score);
    }
}
