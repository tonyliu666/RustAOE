use sim::Rng;

fn take(rng: &mut Rng, n: usize) -> Vec<u64> {
    (0..n).map(|_| rng.next_u64()).collect()
}

#[test]
fn the_same_seed_produces_the_same_sequence() {
    let a = take(&mut Rng::from_seed(12345), 32);
    let b = take(&mut Rng::from_seed(12345), 32);
    assert_eq!(a, b);
}

#[test]
fn different_seeds_produce_different_sequences() {
    let a = take(&mut Rng::from_seed(1), 32);
    let b = take(&mut Rng::from_seed(2), 32);
    assert_ne!(a, b);
}

#[test]
fn a_zero_seed_still_produces_varied_output() {
    // A raw xoshiro state of all zeros is a fixed point that emits zeros
    // forever. Seeding must scramble before use.
    let out = take(&mut Rng::from_seed(0), 16);
    assert!(
        out.iter().any(|&x| x != 0),
        "zero seed degenerated: {out:?}"
    );
    assert!(out.windows(2).any(|w| w[0] != w[1]), "output is constant");
}

#[test]
fn below_stays_within_the_requested_bound() {
    let mut rng = Rng::from_seed(7);
    for _ in 0..1000 {
        assert!(rng.below(10) < 10);
    }
    for _ in 0..1000 {
        assert!(rng.below(1) < 1);
    }
}

#[test]
fn below_eventually_reaches_every_value_in_range() {
    let mut rng = Rng::from_seed(99);
    let mut seen = [false; 6];
    for _ in 0..1000 {
        seen[rng.below(6) as usize] = true;
    }
    assert!(
        seen.iter().all(|&s| s),
        "some values never appeared: {seen:?}"
    );
}

#[test]
#[should_panic]
fn below_zero_panics_because_the_range_is_empty() {
    let _ = Rng::from_seed(1).below(0);
}

#[test]
fn cloning_the_rng_forks_the_sequence_without_advancing_the_original() {
    let mut rng = Rng::from_seed(42);
    let _ = rng.next_u64();
    let mut forked = rng.clone();
    assert_eq!(take(&mut rng, 8), take(&mut forked, 8));
}
