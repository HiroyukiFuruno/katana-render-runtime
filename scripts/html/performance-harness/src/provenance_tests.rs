#![cfg(test)]

use super::arm_name;

#[cfg(all(feature = "baseline", not(feature = "candidate")))]
#[test]
fn arm_name_matches_the_baseline_feature() {
    assert_eq!(arm_name(), "baseline-0.4.23");
}

#[cfg(all(feature = "candidate", not(feature = "baseline")))]
#[test]
fn arm_name_matches_the_candidate_feature() {
    assert_eq!(arm_name(), "candidate");
}
