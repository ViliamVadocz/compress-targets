use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, BufWriter, Write},
};

use bitvec::{order::Lsb0, vec::BitVec};
use compress_targets::{Target, LOG_MIN, MIN_PROBABILITY};
use fast_tak::{Game, Reserves};
use takparse::{Color, Direction, Move, MoveKind, Piece};

const USAGE: &str = "Usage:
    compress <path/to/input> <path/to/output> <size_of_board>
";

fn main() {
    let mut args = std::env::args();
    let (_, Some(first), Some(second), Some(third), None) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        println!("{USAGE}");
        return;
    };

    let input = match OpenOptions::new().read(true).open(first) {
        Ok(input) => BufReader::new(input),
        Err(err) => {
            eprintln!("Could not open input file: {err}");
            return;
        }
    };

    let mut output = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(second)
    {
        Ok(output) => BufWriter::new(output),
        Err(err) => {
            eprintln!("Could not open or create the output file: {err}");
            return;
        }
    };

    let size: usize = match third.parse() {
        Ok(size) => size,
        Err(err) => {
            eprintln!("The specified size is not a number: {err}");
            return;
        }
    };

    match size {
        3 => compress::<3>(input, &mut output),
        4 => compress::<4>(input, &mut output),
        5 => compress::<5>(input, &mut output),
        6 => compress::<6>(input, &mut output),
        7 => compress::<7>(input, &mut output),
        8 => compress::<8>(input, &mut output),
        _ => {
            eprintln!("Unsupported board size {size}");
            return;
        }
    }
    println!("Successfully compressed targets.");
}

fn compress<const N: usize>(input: impl BufRead, output: &mut impl Write)
where
    Reserves<N>: Default,
{
    let mut original_size = 0;
    let mut written = 0;

    let mut action_buffer = vec![];
    let mut previous_state: Game<N, 4> = Game::default();
    for (i, maybe_line) in input.lines().enumerate() {
        let line = match maybe_line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("Skipping error line [{i}]: {err}");
                continue;
            }
        };
        let target: Target = match line.parse() {
            Ok(target) => target,
            Err(err) => {
                eprintln!("Could not parse Target: {err}");
                continue;
            }
        };
        let state = Game::<N, 4>::from(target.tps.clone());

        // Check if this state is reachable with one action from the previous one.
        let action = action_buffer.drain(..).find(|&action| {
            let mut next = previous_state.clone();
            next.play(action).expect(
                "The previously generated actions should be valid to play on the previous state.",
            );
            next.reversible_plies = state.reversible_plies; // This is not stored in the TPS
            next.board == state.board
        });

        // Refill action buffer, update last state, and validate target.
        state.possible_moves(&mut action_buffer);
        previous_state = state.clone();
        if !target.actions_match_policy(&action_buffer) {
            eprintln!("Generated actions differ from policy actions.");
            continue;
        }

        // stats
        original_size += line.len();
        let before_written = written;

        // Write the state (relative / full)
        written += write_action(output, action);
        if action.is_none() {
            written += write_state(output, &state);
        }

        written += write_value(output, target.value);
        written += write_policy(output, &target.policy);

        let this_written = written - before_written;
        if i % 10_000 == 0 {
            println!(
                "[{i}] {original_size} -> {written} ({:.1}%)",
                percent(original_size, written)
            );
        }
        if cfg!(false) {
            println!(
                "[{i}] {} -> {this_written} ({:.1}%), total: {original_size} -> {written} ({:.1}%).",
                line.len(),
                percent(line.len(), this_written),
                percent(original_size, written),
            )
        }
    }
}

fn percent(before: usize, after: usize) -> f32 {
    100.0 * (after as f32 / before as f32)
}

fn write_action(output: &mut impl Write, action: Option<Move>) -> usize {
    let Some(action) = action else {
        // zero-byte means state is not relative.
        output.write_all(&[0x00]).unwrap();
        return 1;
    };

    let first = if let MoveKind::Spread(_, pattern) = action.kind() {
        let mask = pattern.mask();
        assert_ne!(mask, 0x00, "picking up 0 is impossible");
        assert_ne!(mask, 0xff, "moving 8 times is impossible");
        mask
    } else {
        0xFF // indicate the action is a placement
    };

    let second = {
        let square = action.square();
        let col = square.column();
        let row = square.row();
        assert!(row < 8);
        assert!(col < 8);
        let square_bits = (row << 3) | col;

        let last_two = match action.kind() {
            MoveKind::Place(Piece::Flat) => 0b01,
            MoveKind::Place(Piece::Wall) => 0b10,
            MoveKind::Place(Piece::Cap) => 0b11,
            MoveKind::Spread(Direction::Up, _) => 0b00,
            MoveKind::Spread(Direction::Down, _) => 0b01,
            MoveKind::Spread(Direction::Left, _) => 0b10,
            MoveKind::Spread(Direction::Right, _) => 0b11,
        };

        (last_two << 6) | square_bits
    };

    output.write_all(&[first, second]).unwrap();
    2
}

fn write_state<const N: usize, const HALF_KOMI: i8>(
    output: &mut impl Write,
    state: &Game<N, HALF_KOMI>,
) -> usize {
    let mut bitvec = BitVec::<u8, Lsb0>::new();
    bitvec.push(state.to_move == Color::White); // to_move
    for stack in state.board.iter().flatten() {
        let Some((piece, top_color)) = stack.top() else {
            bitvec.push(false); // unoccupied
            continue;
        };
        bitvec.push(true); // occupied
        match piece {
            Piece::Flat => bitvec.push(false), // nonblocking (i.e. flat)
            Piece::Cap => {
                bitvec.push(true); // blocking
                bitvec.push(true); // & road (i.e. cap)
            }
            Piece::Wall => {
                bitvec.push(true); // blocking
                bitvec.push(false); // & not road (i.e. wall)
            }
        }
        if stack.size() > 1 {
            bitvec.push(true); // stack is large
            assert!(stack.size() < 128);
            let size_bitvec = BitVec::<u8, Lsb0>::from_element(stack.size() as u8);
            bitvec.extend(size_bitvec.into_iter().take(7)); // size of stack
            bitvec.extend(stack.colors().into_iter().map(|c| c == Color::White));
        } else {
            bitvec.push(false); // stack is small
            bitvec.push(top_color == Color::White); // just the color
        }
    }
    let vec: Vec<u8> = bitvec.into_vec();
    output.write_all(&vec).unwrap();
    vec.len()
}

fn write_value(output: &mut impl Write, value: f32) -> usize {
    assert!(value >= -1.0);
    assert!(value <= 1.0);
    let compressed: u16 = (((f64::from(value) + 1.0) / 2.0) * f64::from(0xFFFF)).round() as u16;
    let bytes = compressed.to_le_bytes();
    output.write_all(&bytes).unwrap();
    bytes.len()
}

fn write_policy(output: &mut impl Write, policy: &[(Move, f32)]) -> usize {
    assert!((MIN_PROBABILITY.ln() - LOG_MIN).abs() < 1e-6);

    let mut written = 0;
    for &(action, probability) in policy {
        let probability = f64::from(probability);
        if probability < MIN_PROBABILITY {
            continue; // skip low probability actions
        }
        let log_prob = probability.ln();
        assert!(log_prob <= 0.0);
        assert!(log_prob >= LOG_MIN);

        let compressed = ((log_prob / LOG_MIN) * f64::from(0xFFFF)).round() as u16;
        let bytes = compressed.to_le_bytes();
        written += write_action(output, Some(action));
        output.write_all(&bytes).unwrap();
        written += bytes.len();
    }
    // empty action to mark end of policy
    written += write_action(output, None);

    written
}
