use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, BufWriter, Write},
};

const USAGE: &str = "Usage:
    reverse-lines <path/to/input> <path/to/output>
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

    let lines: Vec<String> = input.lines().map(|line| line.unwrap()).collect();

    for line in lines.into_iter().rev() {
        output.write_all(line.as_bytes()).unwrap();
        output.write_all(b"\n").unwrap();
    }
}
