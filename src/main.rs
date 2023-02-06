use clap::Parser;
use ordered_float::NotNan;
use rayon::prelude::*;

use std::{collections::HashMap, fs::File, io::Write};

const CROSSED_OUT: u8 = 0;

type Word = [u8; 5];
type Colors = [u8; 5];

fn to_word(w: impl AsRef<[u8]>) -> Word {
    w.as_ref().try_into().unwrap()
}

struct Tree {
    guess: Word,
    total_guesses: usize,
    max_guesses: usize,
    children: HashMap<Colors, Tree>,
}

impl Tree {
    fn leaf(guess: Word) -> Self {
        Self {
            guess,
            total_guesses: 1,
            max_guesses: 1,
            children: Default::default(),
        }
    }

    fn print(&self, n_answers: usize) {
        println!(
            "{}, total: {}, avg: {}, max: {}",
            std::str::from_utf8(&self.guess).unwrap(),
            self.total_guesses,
            self.total_guesses as f32 / n_answers as f32,
            self.max_guesses,
        )
    }

    fn write(&self, w: &mut impl Write, mut line: Vec<u8>) -> std::io::Result<()> {
        if self.children.is_empty() {
            line.extend(self.guess);
            line.push(b'\n');
            w.write_all(&line)
        } else {
            line.extend(self.guess);
            line.push(b',');
            for child in self.children.values() {
                child.write(w, line.clone())?;
            }
            Ok(())
        }
    }
}

fn score(mut guess: Word, mut answer: Word) -> Word {
    let mut colors = [b'b'; 5];

    for i in 0..5 {
        if guess[i] == answer[i] {
            colors[i] = b'g';
            answer[i] = CROSSED_OUT;
            guess[i] = CROSSED_OUT;
        }
    }

    for i in 0..5 {
        if guess[i] != CROSSED_OUT {
            if let Some(j) = answer.iter().copied().position(|a| a == guess[i]) {
                colors[i] = b'y';
                answer[j] = CROSSED_OUT;
                guess[i] = CROSSED_OUT;
            }
        }
    }

    colors
}

fn solve(params: &Params, depth: usize, guesses: &[Word], answers: &[Word]) -> Option<Tree> {
    assert!(!answers.is_empty());
    if depth >= 7 {
        return None;
    }

    if let &[only_answer] = answers {
        return Some(Tree::leaf(only_answer));
    }

    let top_guesses: Vec<Word> = if depth == 0 && params.starting_word.is_some() {
        vec![to_word(params.starting_word.as_ref().unwrap())]
    } else {
        let mut groups = HashMap::<Word, usize>::new();
        let mut ranked_guesses: Vec<(_, Word)> = guesses
            .iter()
            .map(|&guess| {
                groups.clear();

                for &answer in answers {
                    let colors = score(guess, answer);
                    *groups.entry(colors).or_default() += 1;
                }

                let mut sum: usize = groups.values().copied().sum();
                sum -= groups.get(&[b'g'; 5]).copied().unwrap_or_default();

                let avg = NotNan::new(sum as f64 / groups.len() as f64).unwrap();
                (avg, guess)
            })
            .collect();
        ranked_guesses.sort_unstable();
        ranked_guesses
            .iter()
            .map(|(_score, guess)| *guess)
            .take(params.n_guesses)
            .collect()
    };

    assert!(!top_guesses.is_empty());

    let tree = top_guesses
        .iter()
        .filter_map(|&guess| {
            if depth == 0 {
                print!("{}...\r", std::str::from_utf8(&guess).unwrap());
                std::io::stdout().flush().unwrap();
            }

            let mut tree = Tree {
                total_guesses: answers.len(),
                max_guesses: 0,
                guess,
                children: Default::default(),
            };

            let mut groups = HashMap::<Word, Vec<Word>>::new();
            for &answer in answers {
                let colors = score(guess, answer);
                groups.entry(colors).or_default().push(answer);
            }

            let recurse = |(_score, grouped_answers): (&Colors, &Vec<Word>)| {
                solve(params, depth + 1, guesses, grouped_answers)
            };

            let children: Vec<Option<Tree>> = if depth <= 1 {
                groups.par_iter().map(recurse).collect()
            } else {
                groups.iter().map(recurse).collect()
            };

            for (&score, child) in groups.keys().zip(children) {
                let child = child?;
                if score != [b'g'; 5] {
                    tree.total_guesses += child.total_guesses;
                }
                tree.max_guesses = tree.max_guesses.max(child.max_guesses + 1);
                tree.children.insert(score, child);
            }

            if depth == 0 {
                tree.print(answers.len())
            }

            Some(tree)
        })
        .min_by_key(|tree| tree.total_guesses)?;

    Some(tree)
}

#[derive(Parser)]
struct Params {
    #[clap(short, long, default_value = "20")]
    n_guesses: usize,
    #[clap(long)]
    answers_only: bool,
    #[clap(long)]
    starting_word: Option<String>,
}

fn main() {
    println!("Wordle solver!");

    static ONLY_GUESSES: &[u8] = include_bytes!("../guesses.txt");
    static ANSWERS: &[u8] = include_bytes!("../answers.txt");

    let params = Params::parse();

    let answers: Vec<Word> = ANSWERS.split(|&b| b == b'\n').map(to_word).collect();
    let mut guesses: Vec<Word> = answers.clone();

    if !params.answers_only {
        guesses.extend(ONLY_GUESSES.split(|&b| b == b'\n').map(to_word));
        guesses.sort_unstable();
        guesses.dedup();
    }

    let tree = solve(&params, 0, &guesses, &answers).unwrap();
    println!("\nDone!");
    tree.print(answers.len());

    let mut file = File::create("out.txt").unwrap();
    tree.write(&mut file, vec![]).unwrap();
}

#[test]
fn test_colors() {
    let colors = score(to_word("silly"), to_word("hotel"));
    assert_eq!(colors, to_word("bbybb"));
    let colors = score(to_word("silly"), to_word("daily"));
    assert_eq!(colors, to_word("bybgg"));
}
