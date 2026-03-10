use bitcoin::ScriptBuf;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Bitcoin script types relevant to coin selection and fee calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptType {
    /// Pay-to-Taproot (SegWit v1) — most efficient and private
    P2TR,
    /// Pay-to-Witness-Public-Key-Hash (SegWit v0)
    P2WPKH,
    /// Pay-to-Script-Hash wrapping P2WPKH (nested SegWit)
    P2SH,
    /// Pay-to-Public-Key-Hash (legacy)
    P2PKH,
    /// Unknown or unsupported script type
    Unknown,
}

impl fmt::Display for ScriptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptType::P2TR => write!(f, "P2TR"),
            ScriptType::P2WPKH => write!(f, "P2WPKH"),
            ScriptType::P2SH => write!(f, "P2SH"),
            ScriptType::P2PKH => write!(f, "P2PKH"),
            ScriptType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl ScriptType {
    /// Detect script type from a scriptPubKey.
    pub fn detect(script: &ScriptBuf) -> Self {
        if script.is_p2tr() {
            ScriptType::P2TR
        } else if script.is_p2wpkh() {
            ScriptType::P2WPKH
        } else if script.is_p2sh() {
            ScriptType::P2SH
        } else if script.is_p2pkh() {
            ScriptType::P2PKH
        } else {
            ScriptType::Unknown
        }
    }

    /// Estimated input weight in virtual bytes for spending this script type.
    ///
    /// These values include the outpoint (36 bytes), sequence (4 bytes),
    /// and the typical witness/scriptSig data for each type.
    pub fn input_weight_vbytes(&self) -> f64 {
        match self {
            // Taproot key-path spend: ~57.5 vBytes
            // outpoint(36) + sequence(4) + scriptSigLen(1) + witness(1+1+64)/4
            ScriptType::P2TR => 57.5,
            // P2WPKH: ~68 vBytes
            // outpoint(36) + sequence(4) + scriptSigLen(1) + witness(1+1+72+1+33)/4
            ScriptType::P2WPKH => 68.0,
            // P2SH-P2WPKH: ~91 vBytes
            // outpoint(36) + sequence(4) + scriptSig(23) + witness(107)/4
            ScriptType::P2SH => 91.0,
            // P2PKH: ~148 vBytes
            // outpoint(36) + sequence(4) + scriptSig(107)
            ScriptType::P2PKH => 148.0,
            // Conservative estimate for unknown types
            ScriptType::Unknown => 148.0,
        }
    }

    /// Fee cost in satoshis to spend this input at the given feerate.
    pub fn spending_cost_sats(&self, feerate_sat_per_vb: f64) -> u64 {
        (self.input_weight_vbytes() * feerate_sat_per_vb).ceil() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script_from_hex(hex: &str) -> ScriptBuf {
        ScriptBuf::from_hex(hex).unwrap()
    }

    #[test]
    fn test_detect_p2tr() {
        // OP_1 <32-byte-key>
        let hex = "5120".to_string() + &"aa".repeat(32);
        assert_eq!(ScriptType::detect(&script_from_hex(&hex)), ScriptType::P2TR);
    }

    #[test]
    fn test_detect_p2wpkh() {
        // OP_0 <20-byte-hash>
        let hex = "0014".to_string() + &"bb".repeat(20);
        assert_eq!(
            ScriptType::detect(&script_from_hex(&hex)),
            ScriptType::P2WPKH
        );
    }

    #[test]
    fn test_detect_p2sh() {
        // OP_HASH160 <20-byte-hash> OP_EQUAL
        let hex = "a914".to_string() + &"cc".repeat(20) + "87";
        assert_eq!(ScriptType::detect(&script_from_hex(&hex)), ScriptType::P2SH);
    }

    #[test]
    fn test_detect_p2pkh() {
        // OP_DUP OP_HASH160 <20-byte-hash> OP_EQUALVERIFY OP_CHECKSIG
        let hex = "76a914".to_string() + &"dd".repeat(20) + "88ac";
        assert_eq!(
            ScriptType::detect(&script_from_hex(&hex)),
            ScriptType::P2PKH
        );
    }

    #[test]
    fn test_input_weights_ordering() {
        // Taproot should be cheapest, P2PKH most expensive
        assert!(ScriptType::P2TR.input_weight_vbytes() < ScriptType::P2WPKH.input_weight_vbytes());
        assert!(
            ScriptType::P2WPKH.input_weight_vbytes() < ScriptType::P2SH.input_weight_vbytes()
        );
        assert!(
            ScriptType::P2SH.input_weight_vbytes() < ScriptType::P2PKH.input_weight_vbytes()
        );
    }

    #[test]
    fn test_spending_cost() {
        let cost = ScriptType::P2TR.spending_cost_sats(10.0);
        // 57.5 * 10 = 575
        assert_eq!(cost, 575);
    }
}
