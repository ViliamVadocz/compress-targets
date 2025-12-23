use std::{num::ParseFloatError, str::FromStr};

use takparse::{Move, ParseMoveError, ParseTpsError, Tps};
use thiserror::Error;

pub const MIN_PROBABILITY: f64 = 1e-5;
pub const LOG_MIN: f64 = -11.512925464970229; // MIN_PROBABILITY.ln();

#[derive(Error, Debug)]
pub enum ParseTargetError {
    #[error("missing TPS")]
    MissingTps,
    #[error("missing value")]
    MissingValue,
    #[error("missing policy")]
    MissingPolicy,
    #[error("policy format is wrong")]
    WrongPolicyFormat,
    #[error("{0}")]
    Tps(#[from] ParseTpsError),
    #[error("{0}")]
    Action(#[from] ParseMoveError),
    #[error("{0}")]
    Float(#[from] ParseFloatError),
    #[error("policy is NaN")]
    PolicyNan,
}

pub struct Target {
    pub tps: Tps,
    pub value: f32,
    pub ube: Option<f32>,
    pub policy: Box<[(Move, f32)]>,
}

impl FromStr for Target {
    type Err = ParseTargetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        //{tps};{value};{ube};{policy}
        let mut iter = s.trim().split(';');
        let tps: Tps = iter.next().ok_or(ParseTargetError::MissingTps)?.parse()?;
        let value = iter.next().ok_or(ParseTargetError::MissingValue)?.parse()?;

        let mut maybe_ube = iter.next();
        let mut maybe_policy = iter.next();
        if maybe_policy.is_none() {
            // no UBE
            std::mem::swap(&mut maybe_policy, &mut maybe_ube);
        }
        let ube = maybe_ube.map(|s| s.parse()).transpose()?;

        let policy: Box<_> = maybe_policy
            .ok_or(ParseTargetError::MissingPolicy)?
            .split_terminator(',')
            .map(|s| {
                s.split_once(':')
                    .ok_or(ParseTargetError::WrongPolicyFormat)
                    .and_then(|(a, p)| Ok((a.parse()?, p.parse()?)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Target {
            tps,
            value,
            ube,
            policy,
        })
    }
}

impl Target {
    pub fn actions_match_policy(&self, real_actions: &[Move]) -> bool {
        self.policy.len() == real_actions.len()
            && self
                .policy
                .iter()
                .zip(real_actions)
                .all(|((a, _), b)| a == b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_TARGET: &str = "2,2,1,1,2,1/2,1,221C,2C,1S,2/1,221,x,2,2,1/1,1,12S,2,2,1/x2,22121S,2S,12,2/2,1,1,1,1112S,1 2 31;0.5918575;3.6265328;a1+:0.000000010083511,a1>:0.00013831035,a2:0.0000000000003929405,Sa2:0.00000000000041299058,a5+:0.000000000000010234129,a5-:0.000000000000041306917,a5>:0.0000011392179,a6-:0.00000000000006073895,a6>:0.00000000000009715095,b2:0.00000000026189206,Sb2:0.0075890995,b6-:0.000012459096,b6<:0.000000000000022318119,b6>:0.00005123446,c3+:0.00000000020642778,c3<:0.000000087309346,c3>:0.00000000090092095,2c3+:0.000000000000024824918,2c3<:0.73675114,2c3<11:0.0000000000013889032,2c3>:0.000000000000037040138,2c3>11:0.000000000000014383463,c4:0.00000000000005605533,Sc4:0.00000000000014227129,d2+:0.000000000000039717653,d2-:0.0000000000001749246,d2>:0.00000000000005519111,d3+:0.000000000000023464165,d3>:0.000000000000016414155,d4-:0.000000000000010125977,d4<:0.000000005153003,d4>:0.0000000000000025876621,d5+:0.00000000000009255651,d5-:0.00000000000081647536,d5>:0.0000000000006508218,e1+:0.0000000014535488,e1<:0.000000009042388,e1>:0.00000000959485,2e1+:0.0000000009451065,2e1+11:0.0000000056262848,2e1<:0.000000022459792,2e1<11:0.00000010889683,2e1>:0.0000000000000029033113,3e1+:0.0000000005224593,3e1+21:0.0000000012222542,3e1+12:0.0000000019497657,3e1+111:0.000000000000012443339,3e1<:0.000000020248434,3e1<21:0.0000000000000011319459,3e1<12:0.000000000000005042738,3e1<111:0.0000024469482,3e1>:0.000000037637616,4e1+:0.00000005688462,4e1+31:0.000000000000002384248,4e1+22:0.000000000000003858239,4e1+211:0.00000000000003348877,4e1+13:0.000000000000034177166,4e1+121:0.000000000000033766104,4e1+112:0.0000000000006837696,4e1<:0.00000000000035581648,4e1<31:0.000000000000020462764,4e1<22:0.00000000000019169834,4e1<211:0.0024097634,4e1<13:0.0000000000011575893,4e1<121:0.0047274427,4e1<112:0.24831665,4e1<1111:0.0000000000030571975,4e1>:0.0000000000007470424,e2+:0.0000000041345327,e2>:0.000000031271018,2e2+:0.000000009813247,2e2+11:0.0000000061623515,2e2>:0.000000000000009031932,e3+:0.000000030906936,e3-:0.000000000000007576026,e3<:0.000000000000004639956,e3>:0.000000000000020873441,e4-:0.000000000000017625948,e4<:0.000000000000020607134,e4>:0.000000000000048611692,e6<:0.000000000000070074575,e6>:0.000000000000058706275,f2+:0.000000000000027215794,f2-:0.0000000000001934725,f2<:0.00000007336593,f5+:0.000000000000017996365,f5-:0.000000000000018026321";

    #[test]
    fn test_parse_target() {
        let _: Target = EXAMPLE_TARGET.parse().unwrap();
    }
}
