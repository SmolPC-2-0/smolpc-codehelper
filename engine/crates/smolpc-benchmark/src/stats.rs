use serde::Serialize;

/// Descriptive statistics for a series of measurements.
#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub p95: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

/// Compute descriptive statistics for the given values.
/// Returns `None` if the slice is empty.
pub fn compute_stats(values: &[f64]) -> Option<Stats> {
    if values.is_empty() {
        return None;
    }

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;

    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let median = percentile(&sorted, 50.0);
    let p90 = percentile(&sorted, 90.0);
    let p95 = percentile(&sorted, 95.0);

    Some(Stats {
        mean,
        median,
        p90,
        p95,
        std_dev,
        min,
        max,
    })
}

/// Linear interpolation percentile on a sorted slice.
fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (pct / 100.0) * (sorted.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let frac = rank - lower as f64;
        sorted[lower] * (1.0 - frac) + sorted[upper] * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_returns_none() {
        assert!(compute_stats(&[]).is_none());
    }

    #[test]
    fn single_value() {
        let s = compute_stats(&[42.0]).unwrap();
        assert_eq!(s.mean, 42.0);
        assert_eq!(s.median, 42.0);
        assert_eq!(s.min, 42.0);
        assert_eq!(s.max, 42.0);
        assert_eq!(s.std_dev, 0.0);
    }

    #[test]
    fn known_values() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let s = compute_stats(&values).unwrap();
        assert!((s.mean - 30.0).abs() < 1e-9);
        assert!((s.median - 30.0).abs() < 1e-9);
        assert_eq!(s.min, 10.0);
        assert_eq!(s.max, 50.0);
        // std_dev of [10,20,30,40,50] population = sqrt(200) ≈ 14.142
        assert!((s.std_dev - 14.142135).abs() < 0.001);
    }

    #[test]
    fn percentiles_interpolate() {
        // 10 values: 1..=10
        let values: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        let s = compute_stats(&values).unwrap();
        // P50 of 1..10 with linear interp: rank=4.5 → 5.5
        assert!((s.median - 5.5).abs() < 1e-9);
        // P90: rank=8.1 → 9*0.9 + 10*0.1 = 9.1
        assert!((s.p90 - 9.1).abs() < 1e-9);
    }

    #[test]
    fn two_values() {
        let s = compute_stats(&[100.0, 200.0]).unwrap();
        assert!((s.mean - 150.0).abs() < 1e-9);
        assert!((s.median - 150.0).abs() < 1e-9);
        assert_eq!(s.min, 100.0);
        assert_eq!(s.max, 200.0);
    }
}
