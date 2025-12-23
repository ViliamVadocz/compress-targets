use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader},
};

use compress_targets::Target;
use takparse::Move;

const USAGE: &str = "Usage:
    check-compression <path/to/original> <path/to/converted>
";

fn main() {
    let mut args = std::env::args();
    let (_, Some(first), Some(second), None) = (args.next(), args.next(), args.next(), args.next())
    else {
        println!("{USAGE}");
        return;
    };

    let original = match OpenOptions::new().read(true).open(first) {
        Ok(input) => BufReader::new(input),
        Err(err) => {
            eprintln!("Could not open original file: {err}");
            return;
        }
    };

    let converted = match OpenOptions::new().read(true).open(second) {
        Ok(input) => BufReader::new(input),
        Err(err) => {
            eprintln!("Could not open converted file: {err}");
            return;
        }
    };

    let original_lines = original.lines().map(|line| line.unwrap());
    let converted_lines = converted.lines().map(|line| line.unwrap());

    let mut mean_value_loss: f64 = 0.0;
    let mut mean_kl_divergence: f64 = 0.0;

    for (i, (og, cv)) in original_lines.zip(converted_lines).enumerate() {
        let og_target: Target = og.parse().unwrap();
        let cv_target: Target = cv.parse().unwrap();

        assert_eq!(
            og_target.tps.board().collect::<Vec<_>>(),
            cv_target.tps.board().collect::<Vec<_>>()
        );
        assert_eq!(og_target.tps.color(), cv_target.tps.color());

        let value_loss = (og_target.value as f64 - cv_target.value as f64).powi(2);
        update_mean(&mut mean_value_loss, value_loss, i as f64);

        let kl_divergence = kl_div(&og_target.policy, &cv_target.policy);
        update_mean(&mut mean_kl_divergence, kl_divergence, i as f64);

        println!("vl: {value_loss}, \tmean vl: {mean_value_loss}, \tkl: {kl_divergence}, \tmean_kl: {mean_kl_divergence}");
    }
}

fn update_mean(mean: &mut f64, new: f64, i: f64) {
    *mean += (new - *mean) / (i + 1.0);
}

fn kl_div(p: &[(Move, f32)], q: &[(Move, f32)]) -> f64 {
    assert_eq!(p.len(), q.len());
    let mut sum = 0.0;
    for (&(p_a, p_x), &(q_a, q_x)) in p.iter().zip(q) {
        assert_eq!(p_a, q_a);
        let p_x = f64::from(p_x).max(1e-16);
        let q_x = f64::from(q_x).max(1e-16);
        sum += p_x * (p_x / q_x).ln();
    }
    sum
}
