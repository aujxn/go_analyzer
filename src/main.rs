use itertools::Itertools;
use serde_json::Value;
use sgf_parser::{parse, SgfToken};
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::process::{Command, Stdio};

enum Action {
    Play(usize, usize),
    Pass,
}

impl Action {
    fn gen_string(&self) -> String {
        let letters = "ABCDEFGHJKLMNOPQRST";
        let letters: Vec<char> = letters.chars().collect();
        match self {
            Action::Play(x, y) => {
                let x = letters[x - 1];
                let y = 20 - y;
                format!("\"{}{}\"", x, y)
            }
            Action::Pass => String::from("\"PASS\""),
        }
    }
}

struct Game {
    pub id: String,
    pub moves: Vec<Action>,
    //pub rules: String,
    //pub komi: String,
    //pub x_dim: String,
    //pub y_dim: String,
}

#[derive(Debug)]
struct MoveInfo {
    pub move_location: String,
    pub rank: usize,
    pub primary_variation: Vec<String>,
    pub visits: usize,
    pub winrate: f64,
}

#[derive(Debug)]
struct Response {
    pub move_infos: Vec<MoveInfo>,
    pub root_visits: usize,
    pub winrate: f64,
    pub turn_number: usize,
}

impl Game {
    fn to_query(&self) -> String {
        let moves = self
            .moves
            .iter()
            .enumerate()
            .map(|(move_num, action)| {
                let turn;
                if move_num % 2 == 0 {
                    turn = "\"B\"";
                } else {
                    turn = "\"W\"";
                }
                format!("[{},{}]", turn, action.gen_string())
            })
            .join(",");

        format!("{{\"id\":\"{}\",\"moves\":[{}],\"rules\":\"japanese\",\"komi\":6.5,\"boardXSize\":19,\"boardYSize\":19,\"analyzeTurns\":[{}]}}\n", self.id, moves ,(0..self.moves.len()).map(|x| format!("{}",x)).join(","))
    }
}

fn json() -> Vec<Response> {
    let file = File::open("result.json").unwrap();
    let reader = BufReader::new(file);

    let v: Vec<Value> = serde_json::from_reader(reader).unwrap();

    v.iter()
        .map(|response| {
            let turn_number = response["turnNumber"].as_u64().unwrap() as usize;
            let root_visits = response["rootInfo"]["visits"].as_u64().unwrap() as usize;
            let winrate = response["rootInfo"]["winrate"].as_f64().unwrap();
            let move_infos = response["moveInfos"].as_array().unwrap();

            let move_infos = move_infos
                .iter()
                .map(|move_info| {
                    let move_location = move_info["move"].as_str().unwrap().to_string();
                    let rank = move_info["order"].as_u64().unwrap() as usize;
                    let primary_variation = move_info["pv"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|a_move| a_move.as_str().unwrap().to_string())
                        .collect();
                    let visits = move_info["visits"].as_u64().unwrap() as usize;
                    let winrate = move_info["winrate"].as_f64().unwrap();

                    MoveInfo {
                        move_location,
                        rank,
                        primary_variation,
                        visits,
                        winrate,
                    }
                })
                .collect();

            Response {
                turn_number,
                root_visits,
                winrate,
                move_infos,
            }
        })
        .collect()
}

fn main() {
    let game: String = std::fs::read_to_string("../3.sgf")
        .unwrap()
        .parse()
        .unwrap();
    let tree = parse(&game).unwrap();

    let moves: Vec<Action> = tree
        .iter()
        .filter_map(|node| {
            node.tokens
                .iter()
                .map(|token| match token {
                    SgfToken::Move {
                        color: _,
                        action: a_move,
                    } => match a_move {
                        sgf_parser::Action::Move(x, y) => {
                            Some(Action::Play(*x as usize, *y as usize))
                        }
                        sgf_parser::Action::Pass => Some(Action::Pass),
                    },
                    _ => None,
                })
                .find(|a_move| a_move.is_some())
                .unwrap_or(None)
        })
        .collect();

    let a_game = Game {
        id: String::from("test"),
        moves,
    };

    let query = a_game.to_query();
    println!("{}", query);

    let mut child = Command::new("../KataGo/cpp/katago")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .args(&[
            "analysis",
            "-model",
            "/home/austen/Programs/katago-opencl/g170e-b20c256x2-s3761649408-d809581368.bin.gz",
            "-config",
            "./analysis.config",
            "-analysis-threads",
            "6",
        ])
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin
            .write_all(query.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = child.wait_with_output().expect("Failed to read stdout");
    let output = String::from_utf8_lossy(&output.stdout);
    let json_string = format!("[{}]", &output.trim().split('\n').join(","));

    let mut file = File::create("result.json").unwrap();
    file.write_all(json_string.as_bytes()).unwrap();

    let responses = json();
    for response in responses {
        println!(
            "move {}: \t winrate: {} \t visits: {}",
            response.turn_number, response.winrate, response.root_visits
        );

        for variation in response.move_infos {
            println!(
                "rank: {} \t visits: {} \t winrate: {}",
                variation.rank, variation.visits, variation.winrate
            );
            for location in variation.primary_variation {
                print!("{} ", location);
            }
            println!();
        }
    }
}
