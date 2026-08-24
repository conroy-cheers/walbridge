//! K-means clustering in OKLab, with deterministic k-means++ init.

use crate::color::Oklab;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone)]
pub struct Cluster {
    pub centroid: Oklab,
    /// Number of source pixels assigned to this cluster.
    pub count: usize,
}

pub fn kmeans(samples: &[Oklab], k: usize, iterations: usize, seed: u64) -> Vec<Cluster> {
    assert!(!samples.is_empty(), "kmeans: no samples");
    let k = k.min(samples.len());
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut centroids = kmeanspp_init(samples, k, &mut rng);
    let mut assignments = vec![0usize; samples.len()];

    for _ in 0..iterations {
        // Assignment step.
        let mut changed = false;
        for (idx, &s) in samples.iter().enumerate() {
            let mut best = 0;
            let mut best_d = f32::INFINITY;
            for (ci, &c) in centroids.iter().enumerate() {
                let d = s.dist_sq(c);
                if d < best_d {
                    best_d = d;
                    best = ci;
                }
            }
            if assignments[idx] != best {
                assignments[idx] = best;
                changed = true;
            }
        }

        // Update step.
        let mut sums = vec![(0f32, 0f32, 0f32, 0usize); k];
        for (idx, &s) in samples.iter().enumerate() {
            let slot = &mut sums[assignments[idx]];
            slot.0 += s.l;
            slot.1 += s.a;
            slot.2 += s.b;
            slot.3 += 1;
        }
        for (ci, (l, a, b, n)) in sums.into_iter().enumerate() {
            if n > 0 {
                let inv = 1.0 / n as f32;
                centroids[ci] = Oklab::new(l * inv, a * inv, b * inv);
            }
            // Empty clusters: leave centroid in place — they'll get weight 0
            // and be filtered out downstream.
        }

        if !changed {
            break;
        }
    }

    let mut counts = vec![0usize; k];
    for &a in &assignments {
        counts[a] += 1;
    }
    centroids
        .into_iter()
        .zip(counts)
        .map(|(centroid, count)| Cluster { centroid, count })
        .filter(|c| c.count > 0)
        .collect()
}

fn kmeanspp_init(samples: &[Oklab], k: usize, rng: &mut ChaCha8Rng) -> Vec<Oklab> {
    let first = samples[rng.gen_range(0..samples.len())];
    let mut centroids = vec![first];

    while centroids.len() < k {
        // Distance of each sample to the nearest existing centroid.
        let mut dists = vec![0f32; samples.len()];
        let mut total = 0f32;
        for (i, &s) in samples.iter().enumerate() {
            let d = centroids
                .iter()
                .map(|&c| s.dist_sq(c))
                .fold(f32::INFINITY, f32::min);
            dists[i] = d;
            total += d;
        }
        if total <= f32::EPSILON {
            break; // All remaining samples duplicate existing centroids.
        }
        let mut pick = rng.gen::<f32>() * total;
        let mut idx = 0;
        for (i, &d) in dists.iter().enumerate() {
            pick -= d;
            if pick <= 0.0 {
                idx = i;
                break;
            }
        }
        centroids.push(samples[idx]);
    }

    centroids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmeans_recovers_obvious_clusters() {
        let mut samples = Vec::new();
        for _ in 0..100 {
            samples.push(Oklab::new(0.1, 0.0, 0.0));
            samples.push(Oklab::new(0.8, 0.0, 0.0));
            samples.push(Oklab::new(0.5, 0.2, -0.1));
        }
        let clusters = kmeans(&samples, 3, 20, 42);
        assert_eq!(clusters.len(), 3);
        // Each cluster should be roughly balanced.
        for c in &clusters {
            assert!(c.count > 50, "unbalanced: {c:?}");
        }
    }

    #[test]
    fn kmeans_is_deterministic_with_same_seed() {
        let samples: Vec<Oklab> = (0..200)
            .map(|i| Oklab::new(i as f32 / 200.0, 0.0, 0.0))
            .collect();
        let a = kmeans(&samples, 5, 20, 123);
        let b = kmeans(&samples, 5, 20, 123);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.count, y.count);
            assert!((x.centroid.l - y.centroid.l).abs() < 1e-5);
        }
    }
}
