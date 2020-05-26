use itertools::Itertools;
use serde_json::Value;
use sgf_parser::{parse, Action, Color, GameNode, GameTree, SgfToken};
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn act_to_str_test() {
    assert_eq!("A19", action_to_string(&Action::Move(1, 1)));
    assert_eq!("PASS", action_to_string(&Action::Pass));
    assert_eq!("D4", action_to_string(&Action::Move(4, 16)));
    assert_eq!("T19", action_to_string(&Action::Move(19, 1)));
    assert_eq!("T1", action_to_string(&Action::Move(19, 19)));
}

fn action_to_string(a_move: &Action) -> String {
    let letters = "ABCDEFGHJKLMNOPQRST";
    let letters: Vec<char> = letters.chars().collect();
    match a_move {
        Action::Move(x, y) => {
            let x = letters[*x as usize - 1];
            let y = 20 - y;
            format!("{}{}", x, y)
        }
        Action::Pass => String::from("PASS"),
    }
}

#[test]
fn str_to_act_test() {
    assert_eq!(string_to_action("A19"), Action::Move(1, 1));
    assert_eq!(string_to_action("PASS"), Action::Pass);
    assert_eq!(string_to_action("D4"), Action::Move(4, 16));
    assert_eq!(string_to_action("T19"), Action::Move(19, 1));
    assert_eq!(string_to_action("T1"), Action::Move(19, 19));
}

fn string_to_action(move_string: &str) -> Action {
    let letters = "ABCDEFGHJKLMNOPQRST";
    let char_to_num = |target| {
        (letters
            .chars()
            .enumerate()
            .find(|(_i, x)| target == *x)
            .unwrap()
            .0
            + 1) as u8
    };

    if move_string == "PASS" {
        Action::Pass
    } else {
        let x = char_to_num(move_string.chars().next().unwrap());
        let y = 20 - move_string[1..].parse::<u8>().unwrap();
        Action::Move(x, y)
    }
}

struct Game {
    pub id: String,
    pub moves: Vec<(Color, Action)>,
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

fn swap_color(color: &Color) -> Color {
    match color {
        Color::Black => Color::White,
        Color::White => Color::Black,
    }
}

impl Response {
    fn get_variations(&self, color: Color) -> Vec<GameTree> {
        let variation_count = 3;
        let mut variations = vec![];

        for alternative in self.move_infos.iter().take(variation_count) {
            let mut current_color = color;
            let info = SgfToken::Comment(format!(
                "Black winrate: {} Visits: {}",
                alternative.winrate, alternative.visits
            ));
            let mut variation = GameTree {
                nodes: vec![],
                variations: vec![],
            };

            for action_string in alternative.primary_variation.iter() {
                variation.nodes.push(GameNode {
                    tokens: vec![
                        SgfToken::Move {
                            color: current_color,
                            action: string_to_action(action_string),
                        },
                        info.clone(),
                    ],
                });
                current_color = swap_color(&current_color);
            }
            variations.push(variation);
        }
        variations
    }
}

impl Game {
    fn to_query(&self) -> String {
        let moves = self
            .moves
            .iter()
            .map(|(color, action)| {
                let turn;
                match color {
                    Color::Black => turn = "\"B\"",
                    Color::White => turn = "\"W\"",
                }
                format!("[{},\"{}\"]", turn, action_to_string(action))
            })
            .join(",");

        format!("{{\"id\":\"{}\",\"moves\":[{}],\"rules\":\"japanese\",\"komi\":6.5,\"boardXSize\":19,\"boardYSize\":19,\"analyzeTurns\":[{}]}}\n", self.id, moves ,(1..self.moves.len()).map(|x| format!("{}",x)).join(","))
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

    let moves: Vec<(Color, Action)> = tree
        .iter()
        .filter_map(|node| {
            node.tokens
                .iter()
                .map(|token| match token {
                    SgfToken::Move {
                        color: a_color,
                        action: a_move,
                    } => Some((*a_color, *a_move)),
                    _ => None,
                })
                .find(|an_action| an_action.is_some())
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
            "9",
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

    let mut responses = json();

    responses.sort_unstable_by(|a, b| a.turn_number.cmp(&b.turn_number));

    let mut new_tree = sgf_parser::GameTree {
        nodes: vec![],
        variations: vec![],
    };

    let change_in_winrate_threshold = 0.1;
    let add_variations: Vec<usize> = responses
        .iter()
        .zip(responses.iter().skip(1))
        .enumerate()
        .filter_map(|(i, (x, y))| {
            if (x.winrate - y.winrate).abs() > change_in_winrate_threshold {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    let mut add_variations = add_variations.into_iter();
    let mut next_bad_move = add_variations.next();

    let mut current_tree = &mut new_tree;

    for ((i, (color, action)), response) in a_game.moves.iter().enumerate().zip(responses.iter()) {
        let winrate_token = SgfToken::Unknown((
            String::from("SBKV"),
            format!("{:2}", response.winrate * 100.0),
        ));
        let sgf_tokens = vec![
            SgfToken::Move {
                color: *color,
                action: *action,
            },
            winrate_token,
        ];

        current_tree.nodes.push(GameNode { tokens: sgf_tokens });

        if next_bad_move.is_some() && next_bad_move.unwrap() == i {
            current_tree.variations.push(GameTree {
                nodes: vec![],
                variations: vec![],
            });
            let next_color = swap_color(color);
            let mut variations = response.get_variations(next_color);
            current_tree.variations.append(&mut variations);
            current_tree = current_tree.variations.get_mut(0).unwrap();
            next_bad_move = add_variations.next();
        }
    }

    let mut new_sgf = File::create("new.sgf").unwrap();
    let sgf_string: String = new_tree.into();
    new_sgf.write_all(sgf_string.as_bytes()).unwrap();

    /*
    for response in responses.iter() {
        println!(
            "move {}: \t winrate: {} \t visits: {}",
            response.turn_number, response.winrate, response.root_visits
        );

        for variation in response.move_infos.iter() {
            println!(
                "rank: {} \t visits: {} \t winrate: {}",
                variation.rank, variation.visits, variation.winrate
            );
            for location in variation.primary_variation.iter() {
                print!("{} ", location);
            }
            println!();
        }
    }
    */
}
