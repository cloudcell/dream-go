// Copyright 2019 Karl Sundequist Blomdahl <karl.sundequist.blomdahl@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use board_fast::BoardFast;
use board::Board;
use color::Color;
use point::Point;
use point_state::Vertex;

use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub enum StoneStatus {
    Alive,
    Dead,
    Seki,
    BlackTerritory,
    WhiteTerritory
}

impl ::std::str::FromStr for StoneStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<StoneStatus, Self::Err> {
        let s = s.to_lowercase();

        if s == "alive" {
            Ok(StoneStatus::Alive)
        } else if s == "dead" {
            Ok(StoneStatus::Dead)
        } else if s == "seki" {
            Ok(StoneStatus::Seki)
        } else if s == "black_territory" {
            Ok(StoneStatus::BlackTerritory)
        } else if s == "white_territory" {
            Ok(StoneStatus::WhiteTerritory)
        } else {
            Err(())
        }
    }
}

pub trait Score {
    /// Returns true if this game is fully scorable, a game is
    /// defined as scorable if the following conditions hold:
    ///
    /// * Both black and white has played at least one stone
    /// * All empty vertices are only reachable from one color
    /// * There are no stones in atari (excluding super-ko)
    ///
    fn is_scorable(&self) -> bool;

    /// Returns all territory that count as a score for either black
    /// or white.
    fn get_scorable_territory(&self) -> Vec<Point>;

    /// Returns the score for each player `(black, white)` of the
    /// current board state according to the Tromp-Taylor rules.
    ///
    /// This method does not take any komi into account, you will
    /// need to add it yourself.
    fn get_score(&self) -> (usize, usize);

    /// Returns the score for each player `(black, white)` of the
    /// current board state after any stones that are not part of
    /// the given _finished_ board state. The Tromp-Taylor rules are
    /// used to determine the score after clean-up.
    ///
    /// This method does not take any komi into account, you will
    /// need to add it yourself.
    ///
    /// # Arguments
    ///
    /// * `finished` - A copy of this board that has been played to
    ///   finish, using some heuristic
    ///
    fn get_guess_score(&self, finished: &Board) -> (usize, usize);

    /// Returns the status of all stones on the board:
    ///
    /// - **alive** if the stone is present on both
    /// - **dead** if the stone is not present in the _finished_ board
    /// - **seki** if the stone is present on both, but not scorable
    ///
    /// # Arguments
    ///
    /// * `finished` - A copy of this board that has been played to
    ///   finish, using some heuristic
    fn get_stone_status(&self, finished: &Board) -> Vec<(Point, Vec<StoneStatus>)>;
}

impl Score for Board {
    fn is_scorable(&self) -> bool {
        let some_black = Point::all().any(|i| self.inner[i].color() == Some(Color::Black));
        let some_white = Point::all().any(|i| self.inner[i].color() == Some(Color::White));

        some_black && some_white && {
            let black_distance = get_territory_distance(&self.inner, Color::Black);
            let white_distance = get_territory_distance(&self.inner, Color::White);

            Point::all().all(|i| black_distance[i] == 0xff || white_distance[i] == 0xff)
        } && {
            Point::all().all(|i| {
                self.inner[i].color() == None || self.inner.has_n_liberty(i, 2)
            })
        }
    }

    fn get_scorable_territory(&self) -> Vec<Point> {
        let black_distance = get_territory_distance(&self.inner, Color::Black);
        let white_distance = get_territory_distance(&self.inner, Color::White);

        Point::all().filter(|&i| {
            black_distance[i] == 0xff || white_distance[i] == 0xff
        }).collect()
    }

    fn get_score(&self) -> (usize, usize) {
        if self.zobrist_hash != 0 {  // at least one stone has been played
            get_tt_score(&self.inner)
        } else {
            (0, 0)
        }
    }

    fn get_guess_score(&self, finished: &Board) -> (usize, usize) {
        // do not score the finished board directly, since there might be dame
        // fillings, etc, that we do not want to take into account.
        let black_distance = get_territory_distance(&finished.inner, Color::Black);
        let white_distance = get_territory_distance(&finished.inner, Color::White);
        let mut other = self.inner.clone();

        for i in Point::all() {
            if other[i].color() == finished.inner[i].color() {
                // pass
            } else if other[i].color() != None {
                if finished.inner[i].color() == None {
                    let is_dead_black = other[i].color() == Some(Color::Black) && white_distance[i] != 0xff;
                    let is_dead_white = other[i].color() == Some(Color::White) && black_distance[i] != 0xff;

                    if is_dead_black || is_dead_white {
                        other[i].set_color(None);
                    }
                } else {
                    other[i].set_color(None); // remove dead stone
                }
            }
        }

        get_tt_score(&other)
    }

    fn get_stone_status(&self, finished: &Board) -> Vec<(Point, Vec<StoneStatus>)> {
        let black_distance = get_territory_distance(&finished.inner, Color::Black);
        let white_distance = get_territory_distance(&finished.inner, Color::White);
        let mut status_list = vec! [];

        for i in Point::all() {
            if self.inner[i].color() == finished.inner[i].color() {
                if self.inner[i].color() != None {
                    let territory_status = match self.inner[i].color() {
                        Some(Color::Black) => StoneStatus::BlackTerritory,
                        Some(Color::White) => StoneStatus::WhiteTerritory,
                        None => unreachable!()
                    };

                    status_list.push((i, vec! [StoneStatus::Alive, territory_status]));
                } else {
                    if black_distance[i] != 0xff && white_distance[i] == 0xff {
                        status_list.push((i, vec! [StoneStatus::BlackTerritory]));
                    } else if black_distance[i] == 0xff && white_distance[i] != 0xff {
                        status_list.push((i, vec! [StoneStatus::WhiteTerritory]));
                    }
                }
            } else if self.inner[i].color() != None {
                if finished.inner[i].color() == None {
                    let is_dead_black = self.inner[i].color() == Some(Color::Black) && white_distance[i] != 0xff;
                    let is_dead_white = self.inner[i].color() == Some(Color::White) && black_distance[i] != 0xff;

                    if is_dead_black {
                        status_list.push((i, vec! [StoneStatus::Dead, StoneStatus::WhiteTerritory]));
                    } else if is_dead_white {
                        status_list.push((i, vec! [StoneStatus::Dead, StoneStatus::BlackTerritory]));
                    }
                } else {
                    let territory_status = match self.inner[i].color() {
                        Some(Color::Black) => StoneStatus::WhiteTerritory,
                        Some(Color::White) => StoneStatus::BlackTerritory,
                        None => unreachable!()
                    };

                    status_list.push((i, vec! [StoneStatus::Dead, territory_status]));
                }
            } else if self.inner[i].color() == None {
                let territory_status = match finished.inner[i].color() {
                    Some(Color::Black) => StoneStatus::BlackTerritory,
                    Some(Color::White) => StoneStatus::WhiteTerritory,
                    None => unreachable!()
                };

                status_list.push((i, vec! [territory_status]));
            }
        }

        status_list
    }
}

/// Returns the score of the given board according to the Tromp-Taylor
/// rules.
///
/// # Arguments
///
/// * `board` - the board to score
///
fn get_tt_score(board: &BoardFast) -> (usize, usize) {
    let mut black = 0;
    let mut white = 0;
    let black_distance = get_territory_distance(&board, Color::Black);
    let white_distance = get_territory_distance(&board, Color::White);

    for i in Point::all() {
        if black_distance[i] == 0 as u8 {
            black += 1; // black has stone at vertex
        } else if white_distance[i] == 0 as u8 {
            white += 1; // white has stone at vertex
        } else if white_distance[i] == 0xff {
            black += 1; // only reachable from black
        } else if black_distance[i] == 0xff {
            white += 1; // only reachable from white
        }
    }

    (black, white)
}

/// Returns an array containing the (manhattan) distance to the closest stone
/// of the given color for each point on the board.
///
/// # Arguments
///
/// * `color` - the color to get the distance from
///
fn get_territory_distance(board: &BoardFast, color: Color) -> [u8; Point::MAX] {
    let current = Some(color);

    // find all of our stones and mark them as starting points
    let mut territory = [0xff; Point::MAX];
    let mut probes = VecDeque::with_capacity(Point::MAX + 1);

    for point in Point::all() {
        if board[point].color() == current {
            territory[point] = 0;
            probes.push_back(point);
        }
    }

    // compute the distance to all neighbours using a dynamic programming
    // approach where we at each iteration try to update the neighbours of
    // each updated vertex, and if the distance we tried to set was smaller
    // than the current distance we try to update that vertex neighbours.
    //
    // This is equivalent to a Bellman–Ford algorithm for the shortest path.
    while !probes.is_empty() {
        let index = probes.pop_front().unwrap();
        let t = territory[index] + 1;

        for other_point in board.adjacent_to(index) {
            if board[other_point].color() == None && territory[other_point] > t {
                probes.push_back(other_point);
                territory[other_point] = t;
            }
        }
    }

    territory
}

#[cfg(test)]
mod tests {
    use board::*;
    use color::*;
    use super::*;

    #[test]
    fn score_black() {
        let mut board = Board::new(7.5);
        board.place(Color::Black, Point::new(0, 0));

        assert!(!board.is_scorable());
        assert_eq!(board.get_score(), (361, 0));
    }

    #[test]
    fn score_white() {
        let mut board = Board::new(7.5);
        board.place(Color::White, Point::new(0, 0));

        assert!(!board.is_scorable());
        assert_eq!(board.get_score(), (0, 361));
    }

    #[test]
    fn score_black_white() {
        let mut board = Board::new(7.5);
        board.place(Color::White, Point::new(1, 0));
        board.place(Color::White, Point::new(0, 1));
        board.place(Color::White, Point::new(1, 1));
        board.place(Color::White, Point::new(1, 2));
        board.place(Color::White, Point::new(0, 3));
        board.place(Color::White, Point::new(1, 3));
        board.place(Color::Black, Point::new(2, 0));
        board.place(Color::Black, Point::new(2, 1));
        board.place(Color::Black, Point::new(2, 2));
        board.place(Color::Black, Point::new(2, 3));
        board.place(Color::Black, Point::new(0, 4));
        board.place(Color::Black, Point::new(1, 4));
        board.place(Color::Black, Point::new(2, 4));

        assert!(board.is_scorable());
        assert_eq!(board.get_score(), (353, 8));
    }
}
