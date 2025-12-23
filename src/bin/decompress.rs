use std::{
    fmt::Write,
    fs::OpenOptions,
    io::{BufRead, BufReader},
};

use compress_targets::{LOG_MIN, MIN_PROBABILITY};
use fast_tak::{Board, Colors, Game, Reserves, Stack};
use takparse::{Color, Direction, Move, MoveKind, Pattern, Piece, Square, Tps};

const USAGE: &str = "Usage:
    decompress <path/to/input> <size_of_board>
";

fn main() {
    let mut args = std::env::args();
    let (_, Some(first), Some(second), None) = (args.next(), args.next(), args.next(), args.next())
    else {
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

    let size: usize = match second.parse() {
        Ok(size) => size,
        Err(err) => {
            eprintln!("The specified size is not a number: {err}");
            return;
        }
    };

    match size {
        3 => decompress::<3>(input),
        4 => decompress::<4>(input),
        5 => decompress::<5>(input),
        6 => decompress::<6>(input),
        7 => decompress::<7>(input),
        8 => decompress::<8>(input),
        _ => {
            eprintln!("Unsupported board size {size}");
            return;
        }
    }
    println!("Successfully decompressed targets.");
}

fn decompress<const N: usize>(input: impl BufRead)
where
    Reserves<N>: Default,
{
    let mut bytes = input.bytes().map(Result::unwrap).peekable();
    let mut action_buffer = vec![];

    let mut state: Game<N, 4> = Game::default();

    while bytes.peek().is_some() {
        let action = read_action(&mut bytes);
        if let Some(action) = action {
            state
                .play(action)
                .expect("Relative state encoding should include a valid action");
        } else {
            state = read_state(&mut bytes);
        }
        let value = read_value(&mut bytes);
        let policy = read_policy(&mut bytes);

        // Fill in remaining actions
        state.possible_moves(&mut action_buffer);
        let mut completed_policy: Vec<_> = action_buffer
            .drain(..)
            .map(|a| match policy.iter().find(|(b, _)| *b == a) {
                Some(&x) => x,
                None => (a, MIN_PROBABILITY as f32),
            })
            .collect();
        let sum: f32 = completed_policy.iter().map(|(_, p)| p).sum();
        completed_policy.iter_mut().for_each(|(_, p)| *p /= sum);

        // Output decompressed target
        // EDIT THIS IF YOU WANT A DIFFERENT FORMAT
        let tps: Tps = state.clone().into();
        let mut policy_string =
            completed_policy
                .into_iter()
                .fold(String::new(), |mut s, (a, p)| {
                    write!(s, "{a}:{p},").unwrap();
                    s
                });
        policy_string.pop(); // remove training comma
        println!("{tps};{value};{policy_string}");
    }
}

fn read_action(bytes: &mut impl Iterator<Item = u8>) -> Option<Move> {
    let pattern = bytes.next().expect("action pattern");
    if pattern == 0x00 {
        return None;
    }
    let second = bytes.next().expect("action second");
    let col = second & 0b111;
    let row = (second >> 3) & 0b111;
    let square = Square::new(col, row);
    let last_two_bits = second >> 6;
    if pattern == 0xFF {
        let piece = match last_two_bits {
            0b01 => Piece::Flat,
            0b10 => Piece::Wall,
            0b11 => Piece::Cap,
            _ => unreachable!(),
        };
        Some(Move::new(square, MoveKind::Place(piece)))
    } else {
        let direction = match last_two_bits {
            0b00 => Direction::Up,
            0b01 => Direction::Down,
            0b10 => Direction::Left,
            0b11 => Direction::Right,
            _ => unreachable!(),
        };
        Some(Move::new(
            square,
            MoveKind::Spread(direction, Pattern::from_mask(pattern)),
        ))
    }
}

fn read_state<const N: usize, const HALF_KOMI: i8>(
    bytes: &mut impl Iterator<Item = u8>,
) -> Game<N, HALF_KOMI>
where
    Reserves<N>: Default,
{
    let mut bits = BitIterator::new();

    let to_move = if bits.next(bytes) {
        Color::White
    } else {
        Color::Black
    };

    let mut board = Board::default();
    for i in 0..(N * N) {
        let occupied = bits.next(bytes);
        if !occupied {
            continue;
        }
        let blocking = bits.next(bytes);
        let road = if blocking { bits.next(bytes) } else { true };
        let piece = match (blocking, road) {
            (false, true) => Piece::Flat,
            (true, false) => Piece::Wall,
            (true, true) => Piece::Cap,
            _ => unreachable!(),
        };
        let big_stack = bits.next(bytes);
        let stack = if big_stack {
            let mut size = 0;
            for _ in 0..7 {
                size |= u8::from(bits.next(bytes)) << 7;
                size >>= 1;
            }
            assert!(size < 128);
            let mut colors = Colors::default();
            for color in (0..size)
                .map(|_| {
                    if bits.next(bytes) {
                        Color::White
                    } else {
                        Color::Black
                    }
                })
                .rev()
            {
                colors.push(color);
            }
            Stack::exact(piece, colors)
        } else {
            let white = bits.next(bytes);
            let colors = Colors::of_one(if white { Color::White } else { Color::Black });
            Stack::exact(piece, colors)
        };

        let row = (i / N) as u8;
        let col = (i % N) as u8;
        let board_stack = board.get_mut(Square::new(col, row)).unwrap();
        *board_stack = stack;
    }

    Game::from_board_and_to_move(board, to_move, None)
}

struct BitIterator {
    byte: u8,
    read: u8,
}

impl BitIterator {
    fn new() -> Self {
        Self {
            byte: 0,
            read: u8::MAX,
        }
    }

    fn next(&mut self, bytes: &mut impl Iterator<Item = u8>) -> bool {
        if self.read >= 8 {
            self.byte = bytes.next().unwrap();
            self.read = 0;
        }
        let out = (self.byte >> self.read) & 1 != 0;
        self.read += 1;
        out
    }
}

fn read_value(bytes: &mut impl Iterator<Item = u8>) -> f32 {
    let first = bytes.next().unwrap();
    let second = bytes.next().unwrap();
    let compressed = u16::from_le_bytes([first, second]);
    (f64::from(compressed) / f64::from(0xFFFF) * 2.0 - 1.0) as f32
}

fn read_policy(bytes: &mut impl Iterator<Item = u8>) -> Vec<(Move, f32)> {
    let mut policy = vec![];
    loop {
        let Some(action) = read_action(bytes) else {
            break;
        };
        let first = bytes.next().unwrap();
        let second = bytes.next().unwrap();
        let compressed = u16::from_le_bytes([first, second]);
        let logit = f64::from(compressed) * LOG_MIN / f64::from(0xFFFF);
        let probability = logit.exp();
        policy.push((action, probability as f32))
    }

    policy
}
